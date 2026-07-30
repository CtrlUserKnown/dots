use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

// ── package manager ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PackageManager { Homebrew, Apt, Dnf, Unknown }

pub fn detect_pm() -> PackageManager {
    if which_exists("brew")                       { return PackageManager::Homebrew; }
    if Path::new("/usr/bin/apt").exists()          { return PackageManager::Apt; }
    if Path::new("/usr/bin/dnf").exists()          { return PackageManager::Dnf; }
    PackageManager::Unknown
}

pub fn install_package(pm: &PackageManager, name: &str) -> Result<()> {
    let status = match pm {
        PackageManager::Homebrew => {
            if name.starts_with("--cask ") {
                let cask = name.strip_prefix("--cask ").unwrap_or(name);
                Command::new("brew")
                    .args(["install", "--cask", cask])
                    .status()
                    .context("brew install --cask")?
            } else {
                Command::new("brew")
                    .args(["install", name])
                    .status()
                    .context("brew install")?
            }
        }
        PackageManager::Apt => {
            Command::new("sudo")
                .args(["apt", "install", "-y", name])
                .status()
                .context("apt install")?
        }
        PackageManager::Dnf => {
            Command::new("sudo")
                .args(["dnf", "install", "-y", name])
                .status()
                .context("dnf install")?
        }
        PackageManager::Unknown => {
            bail!("no supported package manager found; install manually");
        }
    };
    if !status.success() {
        bail!("install {} failed (exit {})", name, status);
    }
    Ok(())
}

pub fn is_installed(name: &str) -> bool {
    which_exists(name)
}

fn which_exists(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ── premade configs ───────────────────────────────────────────────────────────

pub struct PremadeConfig {
    pub app:         &'static str,
    pub description: &'static str,
    pub dest:        fn() -> PathBuf,
    /// Config file contents, embedded in the binary at build time.
    pub content:     &'static str,
}

pub static PREMADE_CONFIGS: &[PremadeConfig] = &[
    PremadeConfig {
        app:         "ghostty",
        description: "Ghostty terminal — Catppuccin Mocha theme + sensible defaults",
        dest:        || dirs::home_dir().expect("home dir must exist").join(".config/ghostty/config"),
        content:     include_str!("../assets/premade/ghostty/config"),
    },
    PremadeConfig {
        app:         "neovim",
        description: "Minimal Neovim config (lazy.nvim bootstrap)",
        dest:        || dirs::home_dir().expect("home dir must exist").join(".config/nvim/init.lua"),
        content:     include_str!("../assets/premade/neovim/init.lua"),
    },
    PremadeConfig {
        app:         "opencode",
        description: "Opencode AI terminal config",
        dest:        || dirs::home_dir().expect("home dir must exist").join(".config/opencode/config.json"),
        content:     include_str!("../assets/premade/opencode/config.json"),
    },
];

pub fn apply_premade(entry: &PremadeConfig) -> Result<()> {
    write_premade(entry.content, &(entry.dest)())
}

/// Whether a premade config is currently "on": the file exists at its
/// destination *and* its contents match the bundled premade config. Comparing
/// content (rather than mere existence) lets a user's own file at the same path
/// read as "off", and lets [`remove_premade`] flip the state back cleanly.
pub fn is_applied(entry: &PremadeConfig) -> bool {
    std::fs::read_to_string((entry.dest)())
        .map(|c| c == entry.content)
        .unwrap_or(false)
}

/// Turn a premade config "off". If a `.bak` from a previous [`apply_premade`]
/// exists, restore it over the destination (undoing the apply); otherwise just
/// remove the applied file. A no-op if nothing is there.
pub fn remove_premade(entry: &PremadeConfig) -> Result<()> {
    let dest = (entry.dest)();
    let bak  = PathBuf::from(format!("{}.bak", dest.display()));
    if bak.exists() {
        std::fs::rename(&bak, &dest)
            .with_context(|| format!("restoring backup {}", bak.display()))?;
    } else if dest.exists() {
        std::fs::remove_file(&dest)
            .with_context(|| format!("removing {}", dest.display()))?;
    }
    Ok(())
}

pub fn write_premade(content: &str, dest: &Path) -> Result<()> {
    if dest.is_dir() {
        bail!("destination is a directory: {}", dest.display());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    if dest.exists() {
        // Re-applying an already-applied premade (or hitting `apply` twice) must
        // not clobber the user's original with premade content: bail out before
        // touching the backup once `dest` already matches what we'd write.
        if std::fs::read_to_string(dest).map(|c| c == content).unwrap_or(false) {
            return Ok(());
        }
        // Keep only the *first* backup — it holds the user's real original.
        // A later apply (e.g. onto a premade the user has since hand-edited)
        // must not overwrite that with the in-between state.
        let bak = PathBuf::from(format!("{}.bak", dest.display()));
        if !bak.exists() {
            std::fs::copy(dest, &bak)
                .with_context(|| format!("backing up {}", dest.display()))?;
        }
    }
    let tmp = PathBuf::from(format!("{}.tmp", dest.display()));
    std::fs::write(&tmp, content).context("writing premade config")?;
    std::fs::rename(&tmp, dest).context("renaming tmp to dest")?;
    Ok(())
}

// ── install for a dep ─────────────────────────────────────────────────────────

pub fn install_dep(dep: &crate::packages::Dep) -> Result<()> {
    let pm = detect_pm();
    let name = match pm {
        PackageManager::Homebrew => {
            if dep.cask { format!("--cask {}", dep.brew) } else { dep.brew.to_string() }
        }
        PackageManager::Apt => dep.apt.to_string(),
        PackageManager::Dnf => dep.dnf.to_string(),
        PackageManager::Unknown => bail!("no supported package manager found"),
    };
    if name.is_empty() {
        bail!("{} is not available via {pm:?} on this system", dep.bin);
    }
    install_package(&pm, &name)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn detect_pm_returns_something() {
        let pm = detect_pm();
        let _ = pm; // just ensure it doesn't panic
    }

    #[test]
    fn detect_pm_homebrew_if_brew_in_path() {
        if which_exists("brew") {
            assert!(matches!(detect_pm(), PackageManager::Homebrew));
        }
    }

    #[test]
    fn detect_pm_dnf_on_fedora() {
        if Path::new("/usr/bin/dnf").exists() && !which_exists("brew") {
            assert!(matches!(detect_pm(), PackageManager::Dnf));
        }
    }

    #[test]
    fn premade_creates_backup() {
        let tmp    = tempdir().unwrap();
        let dest   = tmp.path().join("config");
        fs::write(&dest, "original").unwrap();
        write_premade("new content", &dest).unwrap();
        let bak = PathBuf::from(format!("{}.bak", dest.display()));
        assert!(bak.exists(), "backup not found");
        assert_eq!(fs::read_to_string(&bak).unwrap(), "original");
        assert_eq!(fs::read_to_string(&dest).unwrap(), "new content");
    }

    #[test]
    fn premade_apply_twice_preserves_original_backup() {
        let tmp  = tempdir().unwrap();
        let dest = tmp.path().join("config");
        fs::write(&dest, "the user's real config").unwrap();

        write_premade("premade content", &dest).unwrap();
        write_premade("premade content", &dest).unwrap(); // re-apply, e.g. run twice

        let bak = PathBuf::from(format!("{}.bak", dest.display()));
        assert_eq!(
            fs::read_to_string(&bak).unwrap(),
            "the user's real config",
            "second apply must not overwrite the backup with premade content"
        );
        assert_eq!(fs::read_to_string(&dest).unwrap(), "premade content");
    }

    #[test]
    fn premade_creates_parent_dir() {
        let tmp  = tempdir().unwrap();
        let dest = tmp.path().join("a/b/c/config");
        write_premade("data", &dest).unwrap();
        assert!(dest.exists());
    }

    #[test]
    fn premade_dest_is_dir_returns_error() {
        let tmp  = tempdir().unwrap();
        let dest = tmp.path().join("destdir");
        fs::create_dir(&dest).unwrap();
        assert!(write_premade("x", &dest).is_err());
    }
}
