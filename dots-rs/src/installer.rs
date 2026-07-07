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
    pub source:      &'static str,
}

pub static PREMADE_CONFIGS: &[PremadeConfig] = &[
    PremadeConfig {
        app:         "ghostty",
        description: "Ghostty terminal — Catppuccin Mocha theme + sensible defaults",
        dest:        || dirs::home_dir().unwrap_or_default().join(".config/ghostty/config"),
        source:      "ghostty/config",
    },
    PremadeConfig {
        app:         "neovim",
        description: "Minimal Neovim config (lazy.nvim bootstrap)",
        dest:        || dirs::home_dir().unwrap_or_default().join(".config/nvim/init.lua"),
        source:      "neovim/init.lua",
    },
    PremadeConfig {
        app:         "opencode",
        description: "Opencode AI terminal config",
        dest:        || dirs::home_dir().unwrap_or_default().join(".config/opencode/config.json"),
        source:      "opencode/config.json",
    },
];

pub fn apply_premade(dots_dir: &Path, entry: &PremadeConfig) -> Result<()> {
    let source = dots_dir.join("src/premade").join(entry.source);
    let dest   = (entry.dest)();
    apply_premade_to(&source, &dest)
}

pub fn apply_premade_to(source: &Path, dest: &Path) -> Result<()> {
    if !source.exists() {
        bail!("premade source not found: {}", source.display());
    }
    if dest.is_dir() {
        bail!("destination is a directory: {}", dest.display());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    if dest.exists() {
        let bak = PathBuf::from(format!("{}.bak", dest.display()));
        std::fs::copy(dest, &bak)
            .with_context(|| format!("backing up {}", dest.display()))?;
    }
    let tmp = PathBuf::from(format!("{}.tmp", dest.display()));
    std::fs::copy(source, &tmp).context("copying premade config")?;
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
        bail!("{} is not available via {} on this system", dep.bin, format!("{pm:?}"));
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
        let src    = tmp.path().join("premade.txt");
        let dest   = tmp.path().join("config");
        fs::write(&src,  "new content").unwrap();
        fs::write(&dest, "original").unwrap();
        apply_premade_to(&src, &dest).unwrap();
        let bak = PathBuf::from(format!("{}.bak", dest.display()));
        assert!(bak.exists(), "backup not found");
        assert_eq!(fs::read_to_string(&bak).unwrap(), "original");
        assert_eq!(fs::read_to_string(&dest).unwrap(), "new content");
    }

    #[test]
    fn premade_creates_parent_dir() {
        let tmp  = tempdir().unwrap();
        let src  = tmp.path().join("src.txt");
        let dest = tmp.path().join("a/b/c/config");
        fs::write(&src, "data").unwrap();
        apply_premade_to(&src, &dest).unwrap();
        assert!(dest.exists());
    }

    #[test]
    fn premade_dest_is_dir_returns_error() {
        let tmp  = tempdir().unwrap();
        let src  = tmp.path().join("src.txt");
        let dest = tmp.path().join("destdir");
        fs::write(&src, "x").unwrap();
        fs::create_dir(&dest).unwrap();
        assert!(apply_premade_to(&src, &dest).is_err());
    }
}
