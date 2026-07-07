use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::settings::dots_dir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymlinkStatus {
    Ok,
    Missing,
    Broken,
    NotALink,
    WrongTarget,
}

pub struct Symlink {
    pub link: PathBuf,
    pub target: PathBuf,
}

pub struct RepairReport {
    pub ok: usize,
    pub repaired: usize,
    pub skipped: usize,
}

pub fn get_symlinks() -> Vec<Symlink> {
    let home = dirs::home_dir().expect("home dir must exist");
    let dots = dots_dir();

    if !dots.exists() {
        eprintln!("warning: ~/.dots does not exist; no symlinks to manage");
        return Vec::new();
    }

    let mut links = vec![
        Symlink {
            link: home.join(".config/bat"),
            target: dots.join("src/bat"),
        },
        Symlink {
            link: home.join(".config/fastfetch"),
            target: dots.join("src/fastfetch"),
        },
        Symlink {
            link: home.join(".config/zsh"),
            target: dots.join("src/zsh/zsh"),
        },
        Symlink {
            link: home.join(".zshrc"),
            target: dots.join("src/zsh/.zshrc"),
        },
    ];

    if which_exists("ghostty") {
        links.push(Symlink {
            link: home.join(".config/ghostty"),
            target: dots.join("src/ghostty"),
        });
    }
    if which_exists("herdr") {
        links.push(Symlink {
            link: home.join(".config/herdr"),
            target: dots.join("src/herdr"),
        });
    }

    links
}

fn which_exists(bin: &str) -> bool {
    std::process::Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn check(s: &Symlink) -> SymlinkStatus {
    let meta = match s.link.symlink_metadata() {
        Ok(m) => m,
        Err(_) => return SymlinkStatus::Missing,
    };

    if !meta.file_type().is_symlink() {
        return SymlinkStatus::NotALink;
    }

    let actual = match fs::read_link(&s.link) {
        Ok(p) => p,
        Err(_) => return SymlinkStatus::Broken,
    };

    if actual == s.target {
        if s.target.exists() {
            SymlinkStatus::Ok
        } else {
            SymlinkStatus::Broken
        }
    } else {
        if s.target.exists() {
            SymlinkStatus::WrongTarget
        } else {
            SymlinkStatus::Broken
        }
    }
}

pub fn repair(s: &Symlink) -> Result<()> {
    match check(s) {
        SymlinkStatus::Ok => return Ok(()),
        SymlinkStatus::Missing => {
            create_symlink(&s.link, &s.target)?;
        }
        SymlinkStatus::Broken | SymlinkStatus::WrongTarget => {
            fs::remove_file(&s.link)
                .with_context(|| format!("unlinking {}", s.link.display()))?;
            create_symlink(&s.link, &s.target)?;
        }
        SymlinkStatus::NotALink => {
            let meta = s.link.metadata()?;
            if meta.is_dir() {
                let bak = backup_path(&s.link);
                move_to_backup(&s.link, &bak)?;
                create_symlink(&s.link, &s.target)?;
            } else {
                // Regular file: check version header
                let link_ver = read_version_header(&s.link);
                let target_ver = read_version_header(&s.target);
                if link_ver.is_some() && link_ver == target_ver {
                    // Already identical version — leave real file in place
                    return Ok(());
                }
                let bak = backup_path(&s.link);
                move_to_backup(&s.link, &bak)?;
                create_symlink(&s.link, &s.target)?;
            }
        }
    }
    Ok(())
}

pub fn repair_all() -> Result<RepairReport> {
    let symlinks = get_symlinks();
    let mut report = RepairReport {
        ok: 0,
        repaired: 0,
        skipped: 0,
    };

    for s in &symlinks {
        let status = check(s);
        if status == SymlinkStatus::Ok {
            report.ok += 1;
            continue;
        }
        match repair(s) {
            Ok(()) => report.repaired += 1,
            Err(e) => {
                eprintln!("skipping {}: {:#}", s.link.display(), e);
                report.skipped += 1;
            }
        }
    }

    Ok(report)
}

pub fn read_version_header(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines().take(10) {
        if let Some(rest) = line.strip_prefix("# dotfiles v") {
            let ver = rest.split_whitespace().next()?;
            return Some(ver.to_string());
        }
    }
    None
}

fn backup_path(path: &Path) -> PathBuf {
    use std::fmt::Write as _;

    let date = {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Convert seconds since epoch to YYYYMMDD without chrono
        let days = now / 86400;
        let (y, m, d) = epoch_days_to_ymd(days);
        let mut s = String::new();
        let _ = write!(s, "{:04}{:02}{:02}", y, m, d);
        s
    };

    let base_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let parent = path.parent().unwrap_or(Path::new("."));

    let candidate = parent.join(format!("{}.bak.{}", base_name, date));
    if !candidate.exists() {
        return candidate;
    }
    for n in 1u32.. {
        let candidate = parent.join(format!("{}.bak.{}.{}", base_name, date, n));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn epoch_days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Gregorian calendar from Julian Day Number
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn move_to_backup(src: &Path, dst: &Path) -> Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Cross-device: fall back to copy + remove
            fs::copy(src, dst)
                .with_context(|| format!("copying {} to {}", src.display(), dst.display()))?;
            fs::remove_file(src)
                .with_context(|| format!("removing original {}", src.display()))?;
            Ok(())
        }
    }
}

#[cfg(unix)]
fn create_symlink(link: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating parent dir {}", parent.display()))?;
    }
    std::os::unix::fs::symlink(target, link)
        .with_context(|| format!("symlinking {} -> {}", link.display(), target.display()))
}

#[cfg(not(unix))]
fn create_symlink(link: &Path, target: &Path) -> Result<()> {
    anyhow::bail!("symlinks not supported on this platform")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn make_symlink(dir: &Path, name: &str, target: &str) -> Symlink {
        Symlink {
            link: dir.join(name),
            target: dir.join(target),
        }
    }

    // ── check() tests ─────────────────────────────────────────────────────────

    #[test]
    fn check_missing() {
        let tmp = tempdir().unwrap();
        let s = make_symlink(tmp.path(), "link", "target");
        assert_eq!(check(&s), SymlinkStatus::Missing);
    }

    #[test]
    fn check_ok() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, b"hello").unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let s = Symlink { link, target };
        assert_eq!(check(&s), SymlinkStatus::Ok);
    }

    #[test]
    fn check_broken() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        // target does not exist
        let s = Symlink { link, target };
        assert_eq!(check(&s), SymlinkStatus::Broken);
    }

    #[test]
    fn check_not_a_link() {
        let tmp = tempdir().unwrap();
        let link = tmp.path().join("link");
        fs::write(&link, b"real file").unwrap();
        let s = make_symlink(tmp.path(), "link", "target");
        assert_eq!(check(&s), SymlinkStatus::NotALink);
    }

    #[test]
    fn check_wrong_target() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, b"target content").unwrap();
        let wrong = tmp.path().join("wrong");
        fs::write(&wrong, b"wrong content").unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&wrong, &link).unwrap();
        let s = Symlink { link, target };
        assert_eq!(check(&s), SymlinkStatus::WrongTarget);
    }

    // ── repair() tests ────────────────────────────────────────────────────────

    #[test]
    fn repair_moves_real_file_to_backup() {
        let tmp = tempdir().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, b"target content").unwrap();
        let link = tmp.path().join("link");
        fs::write(&link, b"real file content").unwrap();

        let s = Symlink {
            link: link.clone(),
            target: target.clone(),
        };
        repair(&s).unwrap();

        // Backup exists
        let bak: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("link.bak.")
            })
            .collect();
        assert!(!bak.is_empty(), "backup file should exist");

        // Symlink now exists pointing to target
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(fs::read_link(&link).unwrap(), target);
    }

    // ── version header skip ───────────────────────────────────────────────────

    #[test]
    fn version_header_skip_leaves_real_file() {
        let tmp = tempdir().unwrap();
        let content = b"# dotfiles v1.5.5\nsome config\n";

        let target = tmp.path().join("target");
        fs::write(&target, content).unwrap();

        let link = tmp.path().join("link");
        fs::write(&link, content).unwrap(); // same version header

        let s = Symlink {
            link: link.clone(),
            target,
        };
        repair(&s).unwrap();

        // No backup created
        let bak_count = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("link.bak.")
            })
            .count();
        assert_eq!(bak_count, 0, "no backup should be created for same-version file");

        // Original real file still in place (not a symlink)
        assert!(
            !link.symlink_metadata().unwrap().file_type().is_symlink(),
            "original file should remain, not be replaced by symlink"
        );
    }
}
