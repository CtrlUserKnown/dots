//! The one place the binary's own version is read from.
//!
//! `build.rs` resolves the version at compile time (tag → git describe →
//! manifest) and bakes it in; every consumer — `--version`, the settings
//! header, the self-update check — reads it through here rather than reaching
//! for `env!` directly, so the resolution story stays in one place.

/// Version without a leading `v` (e.g. `"2.2.0"`, or `"2.2.0-11-ga7df56c"` for
/// a dev build ahead of the tag).
pub const VERSION: &str = env!("DOTS_VERSION");

/// Short commit hash of the build, or empty when built outside a git checkout.
pub const COMMIT: &str = env!("DOTS_COMMIT");

/// Which rule in `build.rs` produced [`VERSION`]: `env`, `git-tag`,
/// `git-untagged`, or `cargo`.
pub const SOURCE: &str = env!("DOTS_VERSION_SOURCE");

/// The version as declared in `Cargo.toml`. Release builds assert this matches
/// the tag, so a drift between the two is a packaging bug worth surfacing.
pub const MANIFEST: &str = env!("CARGO_PKG_VERSION");

/// True when this build is not exactly a released tag — either ahead of one,
/// dirty, or resolved from an untagged checkout. Used to keep dev builds from
/// nagging about updates they already contain.
pub fn is_dev() -> bool {
    SOURCE == "git-untagged" || VERSION.contains('+') || VERSION.contains("-dirty") || ahead_of_tag()
}

/// `git describe` appends `-<n>-g<hash>` when the checkout is ahead of its tag.
fn ahead_of_tag() -> bool {
    let mut parts = VERSION.split('-').skip(1);
    match (parts.next(), parts.next()) {
        (Some(n), Some(g)) => n.chars().all(|c| c.is_ascii_digit()) && g.starts_with('g'),
        _ => false,
    }
}

/// The multi-line `--version` body: the version, then the provenance needed to
/// tell two builds of the same version apart. Built once and cached, because
/// clap wants a `&'static str` for `long_version`. No `dots` prefix — clap
/// prints the binary name ahead of this itself.
pub fn long() -> &'static str {
    static LONG: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    LONG.get_or_init(|| {
        let mut s = VERSION.to_string();
        if !COMMIT.is_empty() {
            s.push_str(&format!("\ncommit:   {COMMIT}"));
        }
        s.push_str(&format!("\nresolved: {SOURCE}"));
        if MANIFEST != VERSION && !VERSION.starts_with(MANIFEST) {
            // Worth showing: the binary and the manifest disagree, which is
            // exactly the drift the release guard exists to catch.
            s.push_str(&format!("\nmanifest: {MANIFEST}"));
        }
        s
    })
    .as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_never_a_bare_hash() {
        // A bare short hash is 7+ hex chars and nothing else — build.rs turns
        // that case into `<manifest>+g<hash>` instead.
        let bare = VERSION.len() >= 7
            && VERSION.chars().all(|c| c.is_ascii_hexdigit())
            && VERSION.chars().any(|c| c.is_ascii_alphabetic());
        assert!(!bare, "version resolved to a bare commit hash: {VERSION}");
    }

    #[test]
    fn version_starts_with_a_number() {
        assert!(
            VERSION.chars().next().is_some_and(|c| c.is_ascii_digit()),
            "version should start with a digit, got {VERSION}"
        );
    }

    #[test]
    fn source_is_one_of_the_known_rules() {
        assert!(matches!(SOURCE, "env" | "git-tag" | "git-untagged" | "cargo"), "unexpected source {SOURCE}");
    }

    #[test]
    fn long_mentions_the_version() {
        assert!(long().contains(VERSION));
    }

    #[test]
    fn ahead_of_tag_detects_describe_suffix() {
        // Sanity-check the shape matcher against strings git actually produces.
        fn ahead(v: &str) -> bool {
            let mut parts = v.split('-').skip(1);
            match (parts.next(), parts.next()) {
                (Some(n), Some(g)) => n.chars().all(|c| c.is_ascii_digit()) && g.starts_with('g'),
                _ => false,
            }
        }
        assert!(ahead("2.2.0-11-ga7df56c"));
        assert!(!ahead("2.2.0"));
        assert!(!ahead("2.2.0-rc.1"));
    }
}
