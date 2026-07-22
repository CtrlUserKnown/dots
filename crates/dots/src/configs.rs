//! App configs discovered from directories in the dotfiles repo.
//!
//! Every app config lives as its own directory (e.g. `nvim/`, `git/`, `bat/`)
//! under a configs root — the `[stow].dir` from `links.toml`, or a conventional
//! dotfiles location. A config is "installed" when the symlinks that directory
//! would create in `$HOME` are all present and healthy; that link planning and
//! checking is the *same* machinery `dots health` uses (see [`crate::links`]
//! and [`crate::symlinks`]), so the two never disagree.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::settings::dots_dir;
use crate::links;
use crate::symlinks::{self, Symlink, SymlinkStatus};

/// Files a config directory may contain that aren't themselves config to link.
const DEFAULT_IGNORE: &[&str] = &[".DS_Store", "*.bak", "README.md", ".git", ".gitignore"];

// ── model ─────────────────────────────────────────────────────────────────────

/// Where a config's symlinks stand relative to `$HOME`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigStatus {
    /// Every planned symlink is present and points at the config.
    Installed,
    /// Some links are in place, some missing or broken.
    Partial,
    /// None of the config's links exist.
    NotInstalled,
    /// The directory holds nothing linkable.
    Empty,
}

impl ConfigStatus {
    /// Right-aligned badge text, matching the requested list look.
    pub fn badge(self) -> &'static str {
        match self {
            ConfigStatus::Installed    => "[ installed ]",
            ConfigStatus::Partial      => "[ partial ]",
            ConfigStatus::NotInstalled => "[ not installed ]",
            ConfigStatus::Empty        => "[ empty ]",
        }
    }
}

/// One app config: a directory plus the symlinks it maps into `$HOME`.
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory name (e.g. `nvim`).
    pub name:   String,
    /// Absolute path to the config directory in the dotfiles repo.
    pub dir:    PathBuf,
    /// Planned symlinks (link in `$HOME` → target inside `dir`).
    pub links:  Vec<Symlink>,
    pub status: ConfigStatus,
}

impl Config {
    /// (ok, total) planned links that are currently healthy.
    pub fn link_counts(&self) -> (usize, usize) {
        let ok = self.links.iter()
            .filter(|s| symlinks::check(s) == SymlinkStatus::Ok)
            .count();
        (ok, self.links.len())
    }
}

// ── discovery ─────────────────────────────────────────────────────────────────

/// Discover configs from the resolved dotfiles source. Empty when no configs
/// root can be located (no `[stow]` dir and no conventional directory exists).
pub fn discover() -> Vec<Config> {
    match resolve_source() {
        Some(src) => configs_in(&src.root, &src.target_root, &src.ignore),
        None => Vec::new(),
    }
}

/// The resolved configs root, if any — useful for the UI to explain where it's
/// looking (or why the list is empty).
pub fn configs_root() -> Option<PathBuf> {
    resolve_source().map(|s| s.root)
}

/// Build the config list for an explicit root. Pure over the filesystem so it's
/// unit-testable with temp dirs. Immediate subdirectories become configs, sorted
/// by name; plain files at the root are ignored.
pub fn configs_in(root: &Path, target_root: &Path, ignore: &[String]) -> Vec<Config> {
    let Ok(entries) = fs::read_dir(root) else { return Vec::new() };

    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| !is_hidden_meta(p))
        .collect();
    dirs.sort();

    dirs.into_iter()
        .map(|dir| {
            let name  = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let links = links::plan_package(&dir, target_root, ignore);
            let status = status_of(&links);
            Config { name, dir, links, status }
        })
        .collect()
}

fn status_of(links: &[Symlink]) -> ConfigStatus {
    if links.is_empty() {
        return ConfigStatus::Empty;
    }
    let ok = links.iter()
        .filter(|s| symlinks::check(s) == SymlinkStatus::Ok)
        .count();
    if ok == 0 {
        ConfigStatus::NotInstalled
    } else if ok == links.len() {
        ConfigStatus::Installed
    } else {
        ConfigStatus::Partial
    }
}

/// Skip version-control and obvious non-config dirs at the configs root.
fn is_hidden_meta(p: &Path) -> bool {
    matches!(
        p.file_name().and_then(|n| n.to_str()),
        Some(".git") | Some(".github") | Some("node_modules")
    )
}

// ── install / remove ──────────────────────────────────────────────────────────

/// Install a config: create/repair all its symlinks (adopting & backing up any
/// real files in the way, exactly as `dots health --fix` does).
pub fn install(cfg: &Config) -> Result<symlinks::RepairReport> {
    Ok(symlinks::repair_list(&cfg.links))
}

/// Remove a config: delete only the symlinks we manage (those that actually
/// point at this config's targets). Real files and unrelated links are left
/// untouched. Returns how many links were removed.
pub fn remove(cfg: &Config) -> Result<usize> {
    let mut removed = 0;
    for s in &cfg.links {
        let Ok(meta) = s.link.symlink_metadata() else { continue };
        if !meta.file_type().is_symlink() {
            continue;
        }
        // Only our own link — one whose readlink resolves to this config's target.
        if fs::read_link(&s.link).map(|t| t == s.target).unwrap_or(false) {
            fs::remove_file(&s.link)
                .with_context(|| format!("removing {}", s.link.display()))?;
            removed += 1;
        }
    }
    Ok(removed)
}

// ── source resolution ─────────────────────────────────────────────────────────

struct Source {
    root:        PathBuf,
    target_root: PathBuf,
    ignore:      Vec<String>,
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

fn default_ignore() -> Vec<String> {
    DEFAULT_IGNORE.iter().map(|s| s.to_string()).collect()
}

/// Resolve where configs live, in priority order:
/// 1. `links.toml` `[stow].dir` (with its target/ignore) — the manifest already
///    describes the dotfiles repo, so honour it first.
/// 2. A conventional dotfiles directory that exists on disk.
fn resolve_source() -> Option<Source> {
    if let Ok(manifest) = links::load_manifest() {
        if let Some(stow) = manifest.stow {
            let root = links::expand_path(&stow.dir);
            if root.is_dir() {
                let target_root = stow.target.as_deref().map(links::expand_path).unwrap_or_else(home);
                let ignore = if stow.ignore.is_empty() { default_ignore() } else { stow.ignore };
                return Some(Source { root, target_root, ignore });
            }
        }
    }

    let home = home();
    for cand in [dots_dir().join("src"), home.join("dotfiles"), home.join(".dotfiles")] {
        if cand.is_dir() {
            return Some(Source { root: cand, target_root: home.clone(), ignore: default_ignore() });
        }
    }
    None
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// A configs root with `nvim/.config/nvim/init.lua` and `git/.gitconfig`.
    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp    = tempdir().unwrap();
        let root   = tmp.path().join("dotfiles");
        let target = tmp.path().join("home");
        fs::create_dir_all(root.join("nvim/.config/nvim")).unwrap();
        fs::write(root.join("nvim/.config/nvim/init.lua"), b"-- nvim").unwrap();
        fs::create_dir_all(root.join("git")).unwrap();
        fs::write(root.join("git/.gitconfig"), b"[user]").unwrap();
        fs::write(root.join("README.md"), b"docs").unwrap(); // a stray file, ignored
        fs::create_dir_all(&target).unwrap();
        (tmp, root, target)
    }

    #[test]
    fn discovers_one_config_per_subdir() {
        let (_tmp, root, target) = fixture();
        let cfgs = configs_in(&root, &target, &default_ignore());
        let names: Vec<_> = cfgs.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["git", "nvim"]); // sorted, README.md not a config
    }

    #[test]
    fn fresh_configs_are_not_installed() {
        let (_tmp, root, target) = fixture();
        let cfgs = configs_in(&root, &target, &default_ignore());
        for c in &cfgs {
            assert_eq!(c.status, ConfigStatus::NotInstalled, "{} should be absent", c.name);
        }
    }

    #[test]
    fn install_then_status_is_installed() {
        let (_tmp, root, target) = fixture();
        let cfgs = configs_in(&root, &target, &default_ignore());
        let git = cfgs.iter().find(|c| c.name == "git").unwrap();

        install(git).unwrap();

        // Re-scan: git is now fully linked, nvim still absent.
        let after = configs_in(&root, &target, &default_ignore());
        assert_eq!(after.iter().find(|c| c.name == "git").unwrap().status, ConfigStatus::Installed);
        assert_eq!(after.iter().find(|c| c.name == "nvim").unwrap().status, ConfigStatus::NotInstalled);
    }

    #[test]
    fn remove_deletes_only_our_symlinks() {
        let (_tmp, root, target) = fixture();
        let git = configs_in(&root, &target, &default_ignore())
            .into_iter().find(|c| c.name == "git").unwrap();
        install(&git).unwrap();

        let removed = remove(&git).unwrap();
        assert_eq!(removed, 1);
        assert!(!target.join(".gitconfig").exists(), "symlink should be gone");
        // The real source file is untouched.
        assert!(root.join("git/.gitconfig").exists());
    }

    #[test]
    fn partial_when_some_links_missing() {
        let (_tmp, root, target) = fixture();
        // Two files in one config; link only one by hand.
        fs::write(root.join("git/.gitignore_global"), b"*.log").unwrap();
        let git = configs_in(&root, &target, &default_ignore())
            .into_iter().find(|c| c.name == "git").unwrap();
        // Create just the first planned link.
        let first = &git.links[0];
        std::os::unix::fs::symlink(&first.target, &first.link).unwrap();

        let status = configs_in(&root, &target, &default_ignore())
            .into_iter().find(|c| c.name == "git").unwrap().status;
        assert_eq!(status, ConfigStatus::Partial);
    }

    #[test]
    fn empty_dir_reports_empty() {
        let tmp    = tempdir().unwrap();
        let root   = tmp.path().join("dotfiles");
        let target = tmp.path().join("home");
        fs::create_dir_all(root.join("blank")).unwrap();
        fs::create_dir_all(&target).unwrap();
        let cfgs = configs_in(&root, &target, &default_ignore());
        assert_eq!(cfgs.len(), 1);
        assert_eq!(cfgs[0].status, ConfigStatus::Empty);
    }
}
