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

use anyhow::{bail, Context, Result};

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
/// 1. The managed source repo at `~/.dots/src` (what `dots gc` pulls into) — the
///    single repo the user manages, so it wins.
/// 2. `links.toml` `[stow].dir` (with its target/ignore) — legacy manifests.
/// 3. A conventional dotfiles directory that exists on disk.
fn resolve_source() -> Option<Source> {
    let home = home();

    let src = dots_dir().join("src");
    if src.is_dir() {
        return Some(Source { root: src, target_root: home.clone(), ignore: default_ignore() });
    }

    if let Ok(manifest) = links::load_manifest() {
        if let Some(stow) = manifest.stow {
            let root = links::expand_path(&stow.dir);
            if root.is_dir() {
                let target_root = stow.target.as_deref().map(links::expand_path).unwrap_or_else(|| home.clone());
                let ignore = if stow.ignore.is_empty() { default_ignore() } else { stow.ignore };
                return Some(Source { root, target_root, ignore });
            }
        }
    }

    for cand in [home.join("dotfiles"), home.join(".dotfiles")] {
        if cand.is_dir() {
            return Some(Source { root: cand, target_root: home.clone(), ignore: default_ignore() });
        }
    }
    None
}

// ── sync the managed source repo (`dots config sync`) ─────────────────────────

/// Pull the latest source repo and re-apply the configs that are currently
/// installed, so a `git`-side change (new files, edits) lands on the system.
/// Configs the user hasn't installed are left alone.
pub fn sync() -> Result<String> {
    let src = dots_dir().join("src");
    if !src.join(".git").is_dir() {
        bail!(
            "no source repo at {} — run 'dots gc -u=<user/repo>' first",
            src.display()
        );
    }
    crate::import::pull(&src)?;

    let mut configs = 0;
    let mut links = 0;
    for cfg in discover() {
        if matches!(cfg.status, ConfigStatus::Installed | ConfigStatus::Partial) {
            let r = install(&cfg)?;
            links += r.ok + r.repaired;
            configs += 1;
        }
    }
    Ok(format!("pulled latest; re-applied {configs} config(s), {links} link(s)"))
}

// ── add a config into the managed source repo (`dots config add`) ─────────────

/// Where a file will land in the source repo, and the symlink that replaces it.
#[derive(Debug, PartialEq, Eq)]
pub struct AddPlan {
    /// App/config name (the top-level subdir under the source repo).
    pub app:  String,
    /// The file's path relative to `$HOME` (its stow-style layout).
    pub rel:  PathBuf,
    /// Absolute destination inside the source repo.
    pub dest: PathBuf,
    /// The original absolute path, which becomes a symlink to `dest`.
    pub link: PathBuf,
}

/// Add an existing dotfile into the managed source repo and link it back, like
/// `chezmoi add`: the real file is moved under `~/.dots/src/<app>/…`, a symlink
/// takes its place, and the config is recorded in `dots.toml`'s `[stow]`. dots
/// handles the file organization so the user never edits a manifest by hand.
pub fn add(path: &Path, app_override: Option<&str>) -> Result<String> {
    let home = dirs::home_dir().context("no home directory")?;
    let src  = ensure_source_repo()?;
    let abs  = absolutize(&home, path);
    if !abs.exists() {
        bail!("{} does not exist", abs.display());
    }
    let plan = plan_add(&home, &src, &abs, app_override)?;
    apply_add(&plan)?;
    ensure_stow_package(&src, &plan.app)?;
    Ok(format!(
        "✓ added '{}' as config '{}'\n  moved  {} → {}\n  linked {} → the repo copy",
        plan.rel.display(),
        plan.app,
        abs.display(),
        plan.dest.display(),
        plan.link.display(),
    ))
}

/// Compute the [`AddPlan`] for moving `abs` (already absolute, under `home`) into
/// `src`. Pure over paths so it is unit-testable without touching the disk.
pub fn plan_add(home: &Path, src: &Path, abs: &Path, app_override: Option<&str>) -> Result<AddPlan> {
    let rel = abs
        .strip_prefix(home)
        .map_err(|_| anyhow::anyhow!("{} is not under your home directory", abs.display()))?
        .to_path_buf();
    let app = match app_override {
        Some(a) => a.to_string(),
        None => infer_app(&rel),
    };
    let dest = src.join(&app).join(&rel);
    Ok(AddPlan { app, rel, dest, link: abs.to_path_buf() })
}

/// Guess the config name from a home-relative path: `.config/<app>/…` → `<app>`,
/// otherwise the first path component with any leading dot stripped.
fn infer_app(rel: &Path) -> String {
    let comps: Vec<&str> = rel
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    if comps.first() == Some(&".config") {
        if let Some(app) = comps.get(1) {
            return (*app).to_string();
        }
    }
    comps
        .first()
        .map(|c| c.trim_start_matches('.').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "misc".to_string())
}

fn apply_add(plan: &AddPlan) -> Result<()> {
    if plan.dest.exists() {
        bail!("{} already exists in the source repo", plan.dest.display());
    }
    if let Some(parent) = plan.dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    move_path(&plan.link, &plan.dest)?;
    // Replace the original with a symlink into the repo (adopt).
    symlinks::repair(&Symlink { link: plan.link.clone(), target: plan.dest.clone() })
        .with_context(|| format!("linking {}", plan.link.display()))?;
    Ok(())
}

/// Move `from` to `to`, falling back to copy+remove for a single file when a
/// plain rename fails (e.g. across filesystems).
fn move_path(from: &Path, to: &Path) -> Result<()> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if from.is_file() {
        fs::copy(from, to).with_context(|| format!("copying {}", from.display()))?;
        fs::remove_file(from).with_context(|| format!("removing {}", from.display()))?;
        return Ok(());
    }
    bail!("could not move {} into the repo (cross-filesystem directory move)", from.display())
}

/// Create `~/.dots/src` (a fresh git repo) if it doesn't exist yet, so a user can
/// start managing configs before ever pulling a repo.
fn ensure_source_repo() -> Result<PathBuf> {
    let src = dots_dir().join("src");
    if !src.is_dir() {
        fs::create_dir_all(&src).with_context(|| format!("creating {}", src.display()))?;
        // Best-effort: make it a real repo so it can be pushed later.
        let _ = std::process::Command::new("git").arg("init").arg("-q").arg(&src).status();
    }
    Ok(src)
}

/// Resolve `path` to an absolute path: expand `~`/`$VAR`, then anchor any still
/// relative path under `home`.
fn absolutize(home: &Path, path: &Path) -> PathBuf {
    let expanded = links::expand_path(&path.to_string_lossy());
    if expanded.is_absolute() {
        expanded
    } else {
        home.join(expanded)
    }
}

/// Record `app` in the source repo's `dots.toml` `[stow].packages`, creating the
/// section (pointed at `~/.dots/src`) on first use. Idempotent.
fn ensure_stow_package(src: &Path, app: &str) -> Result<()> {
    use toml_edit::{value, Array, DocumentMut, Item, Table};

    let path = src.join("dots.toml");
    let mut doc = if path.exists() {
        fs::read_to_string(&path)?
            .parse::<DocumentMut>()
            .context("parsing dots.toml")?
    } else {
        DocumentMut::new()
    };

    let stow = doc
        .entry("stow")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("[stow] in dots.toml is not a table"))?;
    if !stow.contains_key("dir") {
        stow["dir"] = value("~/.dots/src");
    }
    let arr = stow
        .entry("packages")
        .or_insert(value(Array::new()))
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("stow.packages in dots.toml is not an array"))?;
    if !arr.iter().any(|v| v.as_str() == Some(app)) {
        arr.push(app);
    }

    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, doc.to_string()).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, &path).with_context(|| format!("renaming {}", path.display()))?;
    Ok(())
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

    #[test]
    fn infer_app_from_paths() {
        assert_eq!(infer_app(Path::new(".config/nvim/init.lua")), "nvim");
        assert_eq!(infer_app(Path::new(".config/ghostty/config")), "ghostty");
        assert_eq!(infer_app(Path::new(".gitconfig")), "gitconfig");
        assert_eq!(infer_app(Path::new(".zshrc")), "zshrc");
    }

    #[test]
    fn plan_add_builds_stow_layout_dest() {
        let home = Path::new("/home/u");
        let src  = Path::new("/home/u/.dots/src");
        let p = plan_add(home, src, Path::new("/home/u/.config/nvim/init.lua"), None).unwrap();
        assert_eq!(p.app, "nvim");
        assert_eq!(p.rel, Path::new(".config/nvim/init.lua"));
        assert_eq!(p.dest, Path::new("/home/u/.dots/src/nvim/.config/nvim/init.lua"));
    }

    #[test]
    fn plan_add_honors_app_override_and_rejects_outside_home() {
        let home = Path::new("/home/u");
        let src  = Path::new("/home/u/.dots/src");
        let p = plan_add(home, src, Path::new("/home/u/.gitconfig"), Some("git")).unwrap();
        assert_eq!(p.app, "git");
        assert_eq!(p.dest, Path::new("/home/u/.dots/src/git/.gitconfig"));
        assert!(plan_add(home, src, Path::new("/etc/hosts"), None).is_err());
    }

    #[test]
    fn apply_add_moves_file_and_leaves_symlink() {
        let tmp  = tempdir().unwrap();
        let home = tmp.path().join("home");
        let src  = tmp.path().join("src");
        fs::create_dir_all(home.join(".config/nvim")).unwrap();
        fs::create_dir_all(&src).unwrap();
        let orig = home.join(".config/nvim/init.lua");
        fs::write(&orig, b"-- cfg").unwrap();

        let plan = plan_add(&home, &src, &orig, None).unwrap();
        apply_add(&plan).unwrap();

        // real file now lives in the repo, laid out for stow
        let repo_copy = src.join("nvim/.config/nvim/init.lua");
        assert!(repo_copy.is_file());
        assert_eq!(fs::read_to_string(&repo_copy).unwrap(), "-- cfg");
        // original path is now a symlink pointing at the repo copy
        let meta = orig.symlink_metadata().unwrap();
        assert!(meta.file_type().is_symlink());
        assert_eq!(fs::read_link(&orig).unwrap(), plan.dest);
    }
}
