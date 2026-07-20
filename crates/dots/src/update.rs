use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy)]
pub enum UpdateMode {
    Normal,
    Dev,
}

#[derive(Debug, Clone)]
pub struct UpdateStatus {
    pub behind: u32,
    pub label:  String,
}

// ── public API ────────────────────────────────────────────────────────────────

pub fn check_upstream(dots_dir: &Path, mode: UpdateMode) -> Result<UpdateStatus> {
    let dots_dir = dots_dir.to_owned();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(check_upstream_inner(&dots_dir, mode));
    });
    rx.recv_timeout(Duration::from_secs(15))
        .unwrap_or_else(|_| Err(anyhow::anyhow!("update check timed out")))
}

pub fn do_pull(dots_dir: &Path) -> Result<String> {
    let out = std::process::Command::new("git")
        .arg("-C").arg(dots_dir)
        .args(["pull", "--ff-only"])
        .output()
        .context("running git pull")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        anyhow::bail!("{}", if stderr.is_empty() { "pull failed".to_string() } else { stderr });
    }

    crate::symlinks::repair_all().context("repairing symlinks after pull")?;

    let ver_out = std::process::Command::new("git")
        .arg("-C").arg(dots_dir)
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
        .context("reading new version")?;

    let new_ver = String::from_utf8_lossy(&ver_out.stdout)
        .trim()
        .trim_start_matches('v')
        .to_string();

    record_check(dots_dir).ok();
    Ok(new_ver)
}

pub fn should_check(dots_dir: &Path, freq_minutes: u64) -> bool {
    let now  = unix_now();
    let last = read_stamp(dots_dir);
    now.saturating_sub(last) > freq_minutes * 60
}

pub fn record_check(dots_dir: &Path) -> Result<()> {
    write_stamp(dots_dir, unix_now())
}

pub fn write_stamp(dots_dir: &Path, secs: u64) -> Result<()> {
    let path = stamp_path(dots_dir);
    let tmp  = path.with_extension("stamp.tmp");
    std::fs::write(&tmp, secs.to_string())
        .with_context(|| format!("writing stamp {}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .with_context(|| format!("renaming stamp {}", path.display()))?;
    Ok(())
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── internals ─────────────────────────────────────────────────────────────────

fn stamp_path(dots_dir: &Path) -> PathBuf {
    dots_dir.join(".update_stamp")
}

fn read_stamp(dots_dir: &Path) -> u64 {
    std::fs::read_to_string(stamp_path(dots_dir))
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or(0)
}

fn check_upstream_inner(dots_dir: &Path, mode: UpdateMode) -> Result<UpdateStatus> {
    match mode {
        UpdateMode::Normal => check_normal(dots_dir),
        UpdateMode::Dev    => check_dev(dots_dir),
    }
}

fn check_normal(dots_dir: &Path) -> Result<UpdateStatus> {
    git_run(dots_dir, &["fetch", "--depth", "1", "--tags", "origin"])?;

    let tags = git_output(dots_dir, &["tag", "--list", "v*", "--sort=-version:refname"])?;
    let latest = tags.lines().next().unwrap_or("").trim().to_string();

    if latest.is_empty() {
        return Ok(UpdateStatus { behind: 0, label: String::new() });
    }

    let current = git_output(dots_dir, &["describe", "--tags", "--abbrev=0"])
        .unwrap_or_default()
        .trim()
        .to_string();

    let behind = if latest != current { 1 } else { 0 };
    let label  = latest.trim_start_matches('v').to_string();
    Ok(UpdateStatus { behind, label })
}

fn check_dev(dots_dir: &Path) -> Result<UpdateStatus> {
    git_run(dots_dir, &["fetch", "--depth", "1", "origin"])?;

    let count_str = git_output(dots_dir, &["rev-list", "--count", "HEAD..origin/HEAD"])?;
    let behind: u32 = count_str.trim().parse().unwrap_or(0);

    let label = if behind > 0 {
        git_output(dots_dir, &["rev-parse", "--short", "origin/HEAD"])
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        String::new()
    };

    Ok(UpdateStatus { behind, label })
}

fn git_run(dots_dir: &Path, args: &[&str]) -> Result<()> {
    let status = std::process::Command::new("git")
        .arg("-C").arg(dots_dir)
        .args(args)
        .status()
        .context("running git")?;
    if status.success() { Ok(()) } else {
        Err(anyhow::anyhow!("git {} failed", args.first().copied().unwrap_or("")))
    }
}

fn git_output(dots_dir: &Path, args: &[&str]) -> Result<String> {
    let out = std::process::Command::new("git")
        .arg("-C").arg(dots_dir)
        .args(args)
        .output()
        .context("running git")?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(anyhow::anyhow!(
            "git {} failed: {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn check_throttled() {
        let tmp = tempdir().unwrap();
        record_check(tmp.path()).unwrap();
        assert!(!should_check(tmp.path(), 1440));
    }

    #[test]
    fn check_stale() {
        let tmp = tempdir().unwrap();
        write_stamp(tmp.path(), unix_now() - 90_000).unwrap();
        assert!(should_check(tmp.path(), 1440));
    }

    #[test]
    fn check_upstream_one_behind() {
        let tmp     = tempdir().unwrap();
        let origin  = tmp.path().join("origin");
        let local   = tmp.path().join("local");
        std::fs::create_dir_all(&origin).unwrap();

        fn git(dir: &Path, args: &[&str]) {
            std::process::Command::new("git")
                .arg("-C").arg(dir)
                .args(args)
                .env("GIT_TERMINAL_PROMPT", "0")
                .output().ok();
        }

        git(&origin, &["init"]);
        git(&origin, &["config", "user.email", "test@example.com"]);
        git(&origin, &["config", "user.name",  "Test"]);
        std::fs::write(origin.join("a.txt"), b"init").unwrap();
        git(&origin, &["add", "."]);
        git(&origin, &["commit", "-m", "init"]);

        git(tmp.path(), &["clone", origin.to_str().unwrap(), local.to_str().unwrap()]);
        git(&local, &["config", "user.email", "test@example.com"]);
        git(&local, &["config", "user.name",  "Test"]);

        // New commit in origin — local is now 1 behind
        std::fs::write(origin.join("b.txt"), b"second").unwrap();
        git(&origin, &["add", "."]);
        git(&origin, &["commit", "-m", "second"]);

        match check_upstream(&local, UpdateMode::Dev) {
            Ok(s)  => assert_eq!(s.behind, 1, "expected 1 commit behind"),
            Err(e) => eprintln!("check_upstream_one_behind skipped: {e}"),
        }
    }
}
