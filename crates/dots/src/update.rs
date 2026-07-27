//! Self-update: version awareness, release checks, and in-place binary swap.
//!
//! `dots` ships as a standalone binary, so updating means replacing the binary
//! itself — not `git pull`-ing a checkout. We ask the GitHub Releases API whether
//! a newer tag exists and, if so, download the tarball built for this exact
//! OS/arch and rename it over the running executable.
//!
//! The network work never happens on the render thread. [`spawn_check`] and
//! [`spawn_apply`] run on plain std threads and hand their results back over an
//! [`mpsc`] channel, mirroring the pattern the network probe already uses.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};

/// `owner/repo` the releases are published under.
const REPO: &str = "CtrlUserKnown/dots";
/// Version baked in at build time by `build.rs` (git tag → git describe → Cargo).
pub use crate::version::VERSION as CURRENT;
/// GitHub requires a User-Agent on every API request.
const UA: &str = "dots-updater";

/// A newer release than what's running, with everything needed to fetch it.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// The newer version, without a leading `v` (e.g. `"2.1.0"`).
    pub latest: String,
    /// The version we're running now.
    pub current: String,
    /// Direct download URL for this platform's `.tar.gz`.
    pub asset_url: String,
    /// Download URL for the `.sha256` sidecar, verified before we trust the blob.
    pub sha_url: String,
}

/// How this binary was installed — decides whether self-update is even allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallSource {
    /// A plain downloaded binary we may replace ourselves.
    SelfManaged,
    /// Managed by a package manager (Homebrew, etc.); defer to it.
    PackageManager(&'static str),
}

impl InstallSource {
    /// A user-facing sentence for the disabled case, or `None` when self-update
    /// is allowed.
    pub fn defer_message(&self) -> Option<String> {
        match self {
            InstallSource::SelfManaged => None,
            InstallSource::PackageManager(m) => Some(format!("update via `{m}`")),
        }
    }
}

/// Detect how dots was installed by inspecting the running exe's path. Homebrew
/// lives under `/opt/homebrew` (Apple Silicon), `/usr/local/Cellar` (Intel), or
/// `/home/linuxbrew`. Anything else we treat as self-managed.
pub fn install_source() -> InstallSource {
    let exe = std::env::current_exe().unwrap_or_default();
    let p = exe.to_string_lossy();
    if p.contains("/Cellar/")
        || p.contains("/homebrew/")
        || p.contains("/linuxbrew/")
        || p.starts_with("/opt/homebrew")
    {
        return InstallSource::PackageManager("brew upgrade dots");
    }
    InstallSource::SelfManaged
}

// ── release asset naming ──────────────────────────────────────────────────────

/// The `<os>` token in the release asset name. The release workflow uses
/// `darwin`/`linux` (from `uname -s`), which differs from Rust's `macos`.
fn asset_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    }
}

/// The `<arch>` token in the release asset name (`x86_64` / `aarch64`), matching
/// what `install.sh` derives from `uname -m`.
fn asset_arch() -> &'static str {
    std::env::consts::ARCH
}

// ── check ───────────────────────────────────────────────────────────────────

/// Query the Releases API. Returns `Some(info)` when a newer version exists,
/// `None` when we're already current.
pub fn check() -> Result<Option<UpdateInfo>> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp: serde_json::Value = ureq::get(&url)
        .set("User-Agent", UA)
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(10))
        .call()
        .context("querying GitHub releases")?
        .into_json()
        .context("parsing releases JSON")?;

    let tag = resp["tag_name"].as_str().unwrap_or_default();
    let latest = tag.trim_start_matches('v').to_string();
    if latest.is_empty() || !is_newer(&latest, CURRENT) {
        return Ok(None);
    }

    // Asset names are fixed by release.yml: dots-<tag>-<os>-<arch>.tar.gz (+ .sha256).
    let base = format!(
        "https://github.com/{REPO}/releases/download/{tag}/dots-{tag}-{}-{}.tar.gz",
        asset_os(),
        asset_arch(),
    );
    Ok(Some(UpdateInfo {
        latest,
        current: CURRENT.to_string(),
        asset_url: base.clone(),
        sha_url: format!("{base}.sha256"),
    }))
}

/// Spawn [`check`] on a background thread; drain the result off the channel from
/// the render loop. The check is skipped (thread still sends `Ok(None)`) when the
/// throttle stamp says we looked recently.
pub fn spawn_check(frequency_minutes: u64) -> Receiver<Result<Option<UpdateInfo>>> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("dots-update-check".into())
        .spawn(move || {
            if !should_check(frequency_minutes) {
                let _ = tx.send(Ok(None));
                return;
            }
            let result = check();
            if result.is_ok() {
                record_check();
            }
            let _ = tx.send(result);
        })
        .expect("spawn update-check thread");
    rx
}

/// Semver-ish compare: split on `.` and `-`, compare the numeric components.
/// Non-numeric/pre-release suffixes are dropped, so `1.10.0 > 1.9.0` holds and a
/// `-dirty`/`-<n>-g<sha>` dev suffix never reads as newer than a clean tag.
fn is_newer(latest: &str, current: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.split(['.', '-']).filter_map(|p| p.parse().ok()).collect()
    }
    parts(latest) > parts(current)
}

// ── apply ────────────────────────────────────────────────────────────────────

/// Download the release tarball, verify its checksum, extract the `dots` binary,
/// and atomically replace the running executable. On Unix a running binary can
/// be `rename`d over (the open inode stays valid), so this is safe live.
pub fn apply(info: &UpdateInfo) -> Result<()> {
    // 1. Download the tarball into memory.
    let mut buf = Vec::new();
    ureq::get(&info.asset_url)
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(60))
        .call()
        .context("downloading release")?
        .into_reader()
        .read_to_end(&mut buf)
        .context("reading release bytes")?;

    // 2. Verify the .sha256 sidecar before trusting a single byte of it.
    verify_sha256(&buf, &info.sha_url).context("verifying download checksum")?;

    // 3. Extract dots-<...>/dots from the tarball.
    let new_bin = extract_dots_binary(&buf).context("extracting dots binary from tarball")?;

    // 4. Write next to the current exe, chmod, de-quarantine, then rename over it.
    let exe = std::env::current_exe().context("locating current executable")?;
    let dir = exe.parent().context("executable has no parent dir")?;
    let tmp = dir.join(".dots.update.tmp");
    std::fs::write(&tmp, &new_bin).with_context(|| format!("writing {}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
            .context("setting exec permissions")?;
    }
    strip_quarantine(&tmp);
    std::fs::rename(&tmp, &exe).with_context(|| {
        // Clean up the temp file if the swap fails (e.g. cross-device, read-only).
        let _ = std::fs::remove_file(&tmp);
        format!("replacing {}", exe.display())
    })?;
    Ok(())
}

/// Spawn [`apply`] on a background thread so the download doesn't block the UI.
/// The single-shot channel yields the outcome once the swap finishes.
pub fn spawn_apply(info: UpdateInfo) -> Receiver<Result<()>> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("dots-update-apply".into())
        .spawn(move || {
            let _ = tx.send(apply(&info));
        })
        .expect("spawn update-apply thread");
    rx
}

/// Fetch the `.sha256` sidecar and confirm it matches the downloaded bytes.
fn verify_sha256(bytes: &[u8], sha_url: &str) -> Result<()> {
    use sha2::{Digest, Sha256};

    let sidecar = ureq::get(sha_url)
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(15))
        .call()
        .context("downloading checksum")?
        .into_string()
        .context("reading checksum")?;

    // Sidecar format is `<hex>  <filename>`; take the first whitespace token.
    let expected = sidecar
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();
    if expected.len() != 64 {
        bail!("malformed checksum sidecar");
    }

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let actual = hex_lower(&hasher.finalize());

    if actual != expected {
        bail!("checksum mismatch (expected {expected}, got {actual})");
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Pull `dots-<...>/dots` (or any entry whose file name is `dots`) out of the
/// gzip'd tarball and return its bytes.
fn extract_dots_binary(tar_gz: &[u8]) -> Result<Vec<u8>> {
    let decoder = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries().context("reading tar entries")? {
        let mut entry = entry.context("reading tar entry")?;
        let path = entry.path().context("entry path")?.into_owned();
        if path.file_name().and_then(|n| n.to_str()) == Some("dots") {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).context("reading dots bytes")?;
            return Ok(bytes);
        }
    }
    bail!("tarball did not contain a `dots` binary");
}

/// Clear the macOS quarantine attribute so Gatekeeper doesn't block the swapped
/// binary. No-op everywhere else.
fn strip_quarantine(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("xattr")
            .args(["-d", "com.apple.quarantine"])
            .arg(path)
            .status();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
    }
}

// ── throttle stamp ───────────────────────────────────────────────────────────

/// `~/.dots/.update_stamp` — the unix timestamp of the last check, so we don't
/// hit the API on every launch.
fn stamp_path() -> PathBuf {
    crate::config::settings::dots_dir().join(".update_stamp")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// True when at least `frequency_minutes` have elapsed since the last recorded
/// check (or when there's no stamp yet).
fn should_check(frequency_minutes: u64) -> bool {
    let last = std::fs::read_to_string(stamp_path())
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok());
    match last {
        Some(t) => now_secs().saturating_sub(t) >= frequency_minutes.saturating_mul(60),
        None => true,
    }
}

/// Record "checked just now" so the next launch respects the throttle.
fn record_check() {
    let path = stamp_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, now_secs().to_string());
}

// ── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    #[test]
    fn newer_versions_compare_numerically() {
        assert!(is_newer("1.1.0", "1.0.0"));
        assert!(is_newer("1.10.0", "1.9.0")); // not lexicographic
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "1.0.0"));
    }

    #[test]
    fn dev_suffix_is_not_newer_than_clean_tag() {
        // A dev build like 2.0.0-3-gabcdef-dirty must not out-rank 2.0.0.
        assert!(!is_newer("2.0.0", "2.0.0-3-gabcdef"));
    }

    #[test]
    fn asset_tokens_are_release_shaped() {
        // os is darwin/linux (matching uname -s), never Rust's "macos"; arch is
        // a bare token like x86_64/aarch64.
        assert_ne!(asset_os(), "macos");
        assert!(!asset_arch().is_empty());
    }

    #[test]
    fn sha256_verification_roundtrips() {
        use sha2::{Digest, Sha256};
        let blob = b"hello dots";
        let mut h = Sha256::new();
        h.update(blob);
        let digest = hex_lower(&h.finalize());
        assert_eq!(digest.len(), 64);
        let mut h2 = Sha256::new();
        h2.update(b"hello dott");
        assert_ne!(digest, hex_lower(&h2.finalize()));
    }

    #[test]
    fn extract_finds_the_dots_binary() {
        // Build a tiny in-memory tarball: dots-.../dots + a stray README.
        let mut tar_buf = Vec::new();
        {
            let enc = GzEncoder::new(&mut tar_buf, Compression::default());
            let mut builder = tar::Builder::new(enc);

            let payload = b"#!fake-dots-binary";
            let mut header = tar::Header::new_gnu();
            header.set_size(payload.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(&mut header, "dots-x86_64/dots", &payload[..])
                .unwrap();

            let readme = b"docs";
            let mut h2 = tar::Header::new_gnu();
            h2.set_size(readme.len() as u64);
            h2.set_cksum();
            builder
                .append_data(&mut h2, "dots-x86_64/README.md", &readme[..])
                .unwrap();

            builder.into_inner().unwrap().finish().unwrap();
        }

        let extracted = extract_dots_binary(&tar_buf).unwrap();
        assert_eq!(extracted, b"#!fake-dots-binary");
    }

    #[test]
    fn extract_errors_without_dots_entry() {
        let mut tar_buf = Vec::new();
        {
            let enc = GzEncoder::new(&mut tar_buf, Compression::default());
            let mut builder = tar::Builder::new(enc);
            let data = b"nothing here";
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, "other/file.txt", &data[..])
                .unwrap();
            builder.into_inner().unwrap().finish().unwrap();
        }
        assert!(extract_dots_binary(&tar_buf).is_err());
    }

    #[test]
    fn defer_message_only_for_package_manager() {
        assert!(InstallSource::SelfManaged.defer_message().is_none());
        assert!(InstallSource::PackageManager("brew upgrade dots")
            .defer_message()
            .is_some());
    }
}
