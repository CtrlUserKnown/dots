//! User-declared config symlinks (`dots link`).
//!
//! Lets any user manage their own dotfiles, GNU Stow style, via a manifest at
//! `~/.dots/links.toml`. Two forms are supported:
//!
//! ```toml
//! # Explicit one-off links: real file at `source`, symlink created at `target`.
//! [[link]]
//! source = "~/dotfiles/starship.toml"
//! target = "~/.config/starship.toml"
//!
//! # Stow-style folding: mirror each package subdir of `dir` into `target`.
//! [stow]
//! dir      = "~/dotfiles"
//! target   = "~"                                  # optional, defaults to $HOME
//! packages = ["nvim", "git"]
//! ignore   = [".DS_Store", "*.bak", "README.md"]  # optional
//! ```
//!
//! Both forms are lowered to the shared [`crate::symlinks::Symlink`] engine, so
//! `dots health` repairs them alongside the built-in links.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use toml_edit::{value, ArrayOfTables, DocumentMut, Item, Table};

use crate::config::settings::dots_dir;
use crate::symlinks::Symlink;

// ── manifest schema ─────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
pub struct LinksManifest {
    #[serde(default, rename = "link")]
    pub links: Vec<LinkEntry>,
    #[serde(default)]
    pub stow: Option<StowConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkEntry {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StowConfig {
    pub dir: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
}

pub fn manifest_path() -> PathBuf {
    dots_dir().join("links.toml")
}

// ── loading & lowering ──────────────────────────────────────────────────────

pub fn load_manifest() -> Result<LinksManifest> {
    let path = manifest_path();
    if !path.exists() {
        return Ok(LinksManifest::default());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// All symlinks the manifest declares (explicit `[[link]]` + `[stow]` packages).
/// Errors are reported to stderr and produce an empty list, so callers such as
/// `get_symlinks()` never fail solely because of a bad manifest.
pub fn planned_symlinks() -> Vec<Symlink> {
    match load_manifest() {
        Ok(m) => manifest_to_symlinks(&m),
        Err(e) => {
            eprintln!("warning: {e:#}");
            Vec::new()
        }
    }
}

pub fn manifest_to_symlinks(manifest: &LinksManifest) -> Vec<Symlink> {
    let mut out = Vec::new();
    for entry in &manifest.links {
        out.push(Symlink {
            link: expand_path(&entry.target),
            target: expand_path(&entry.source),
        });
    }
    if let Some(stow) = &manifest.stow {
        stow_symlinks(stow, &mut out);
    }
    out
}

fn stow_symlinks(stow: &StowConfig, out: &mut Vec<Symlink>) {
    let dir = expand_path(&stow.dir);
    let target_root = stow.target.as_deref().map(expand_path).unwrap_or_else(home);
    for pkg in &stow.packages {
        let pkg_root = dir.join(pkg);
        if !pkg_root.is_dir() {
            eprintln!(
                "warning: stow package '{}' not found at {}",
                pkg,
                pkg_root.display()
            );
            continue;
        }
        plan_stow_dir(&pkg_root, &target_root, &stow.ignore, out);
    }
}

/// Walk a package directory, mirroring it into the target. A subdirectory is
/// "folded" into a single symlink when its destination doesn't already exist as
/// a real directory; otherwise we "unfold" and recurse so we never clobber a
/// real directory that holds files we don't manage.
fn plan_stow_dir(src_dir: &Path, dst_dir: &Path, ignore: &[String], out: &mut Vec<Symlink>) {
    let entries = match std::fs::read_dir(src_dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if is_ignored(&name.to_string_lossy(), ignore) {
            continue;
        }
        let src = entry.path();
        let dst = dst_dir.join(&name);
        if src.is_dir() {
            let dst_is_real_dir = dst
                .symlink_metadata()
                .map(|m| m.file_type().is_dir())
                .unwrap_or(false);
            if dst_is_real_dir {
                plan_stow_dir(&src, &dst, ignore, out); // unfold
            } else {
                out.push(Symlink { link: dst, target: src }); // fold
            }
        } else {
            out.push(Symlink { link: dst, target: src });
        }
    }
}

// ── mutations (`dots link add` / `remove`) ──────────────────────────────────

/// Record a link and create it. If `source` doesn't exist but a real file/dir
/// sits at `target`, it is adopted (moved into `source`) before linking.
pub fn add_link(source: &Path, target: &Path) -> Result<()> {
    let source_exists = source.symlink_metadata().is_ok();
    if !source_exists {
        match target.symlink_metadata() {
            Ok(m) if !m.file_type().is_symlink() => {
                if let Some(parent) = source.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("creating {}", parent.display()))?;
                }
                std::fs::rename(target, source).with_context(|| {
                    format!("adopting {} -> {}", target.display(), source.display())
                })?;
            }
            _ => bail!(
                "source {} does not exist, and there is no real file at {} to adopt",
                source.display(),
                target.display()
            ),
        }
    }

    upsert_manifest_link(source, target)?;

    let s = Symlink {
        link: target.to_path_buf(),
        target: source.to_path_buf(),
    };
    crate::symlinks::repair(&s)
        .with_context(|| format!("creating symlink {}", target.display()))?;
    Ok(())
}

/// Remove an explicit `[[link]]` declaration for `target` and delete the
/// symlink (never a real file). Stow-managed links are removed by editing the
/// `[stow]` `packages` list, not here.
pub fn remove_link(target: &Path) -> Result<()> {
    if !remove_manifest_link(target)? {
        bail!(
            "no [[link]] declaration found for target {} in links.toml",
            target.display()
        );
    }
    if let Ok(m) = target.symlink_metadata() {
        if m.file_type().is_symlink() {
            std::fs::remove_file(target)
                .with_context(|| format!("removing symlink {}", target.display()))?;
        }
    }
    Ok(())
}

fn upsert_manifest_link(source: &Path, target: &Path) -> Result<()> {
    let path = manifest_path();
    let mut doc = if path.exists() {
        std::fs::read_to_string(&path)?
            .parse::<DocumentMut>()
            .context("parsing links.toml")?
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        DocumentMut::new()
    };

    let src_s = to_manifest_string(source);
    let tgt_s = to_manifest_string(target);

    let arr = doc
        .entry("link")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .ok_or_else(|| anyhow::anyhow!("'link' in links.toml is not an array of tables"))?;

    if arr
        .iter()
        .any(|t| t.get("target").and_then(|v| v.as_str()) == Some(tgt_s.as_str()))
    {
        bail!("a link for target {} already exists in links.toml", tgt_s);
    }

    let mut table = Table::new();
    table["source"] = value(src_s);
    table["target"] = value(tgt_s);
    arr.push(table);

    write_atomic(&path, &doc.to_string())
}

fn remove_manifest_link(target: &Path) -> Result<bool> {
    let path = manifest_path();
    if !path.exists() {
        return Ok(false);
    }
    let mut doc = std::fs::read_to_string(&path)?
        .parse::<DocumentMut>()
        .context("parsing links.toml")?;

    let mut removed = false;
    if let Some(arr) = doc.get_mut("link").and_then(|i| i.as_array_of_tables_mut()) {
        let before = arr.len();
        arr.retain(|t| match t.get("target").and_then(|v| v.as_str()) {
            Some(s) => expand_path(s) != target,
            None => true,
        });
        removed = arr.len() != before;
    }
    if removed {
        write_atomic(&path, &doc.to_string())?;
    }
    Ok(removed)
}

// ── path helpers ────────────────────────────────────────────────────────────

fn home() -> PathBuf {
    dirs::home_dir().expect("home dir must exist")
}

/// Expand `~`, `~/…`, and `$VAR` / `${VAR}` environment references.
pub fn expand_path(input: &str) -> PathBuf {
    let s = expand_env(input);
    if s == "~" {
        return home();
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return home().join(rest);
    }
    PathBuf::from(s)
}

fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        let rest = &s[i + 1..];
        let (name, consumed) = if let Some(inner) = rest.strip_prefix('{') {
            match inner.find('}') {
                Some(end) => (&inner[..end], end + 2), // {NAME}
                None => ("", 0),
            }
        } else {
            let n = rest
                .char_indices()
                .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_')
                .count();
            (&rest[..n], n)
        };
        if name.is_empty() {
            out.push('$');
            continue;
        }
        if let Ok(val) = std::env::var(name) {
            out.push_str(&val);
        }
        for _ in 0..consumed {
            chars.next();
        }
    }
    out
}

/// Collapse a leading home path back to `~/…` so manifests stay portable.
fn to_manifest_string(p: &Path) -> String {
    let home = home();
    match p.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

fn is_ignored(name: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| glob_match(p, name))
}

/// Minimal glob: `*` (all), `*.ext` (suffix), `prefix*` (prefix), or exact.
fn glob_match(pattern: &str, name: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix('*') {
        name.ends_with(suffix)
    } else if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else {
        pattern == name
    }
}

fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, content)
        .with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("renaming {}", path.display()))?;
    Ok(())
}

// ── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn glob_matching() {
        assert!(glob_match("*.bak", "notes.bak"));
        assert!(!glob_match("*.bak", "notes.txt"));
        assert!(glob_match("README*", "README.md"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match(".DS_Store", ".DS_Store"));
        assert!(!glob_match(".DS_Store", "DS_Store"));
    }

    #[test]
    fn env_expansion() {
        std::env::set_var("DOTS_TEST_VAR", "/opt/x");
        assert_eq!(expand_env("$DOTS_TEST_VAR/y"), "/opt/x/y");
        assert_eq!(expand_env("${DOTS_TEST_VAR}/y"), "/opt/x/y");
        assert_eq!(expand_env("no vars here"), "no vars here");
        assert_eq!(expand_env("$"), "$");
    }

    #[test]
    fn explicit_links_lowered() {
        let manifest = LinksManifest {
            links: vec![LinkEntry {
                source: "~/dotfiles/starship.toml".into(),
                target: "~/.config/starship.toml".into(),
            }],
            stow: None,
        };
        let syms = manifest_to_symlinks(&manifest);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].link, home().join(".config/starship.toml"));
        assert_eq!(syms[0].target, home().join("dotfiles/starship.toml"));
    }

    #[test]
    fn stow_folds_when_target_absent() {
        let tmp = tempdir().unwrap();
        let dotfiles = tmp.path().join("dotfiles");
        let target = tmp.path().join("home");
        fs::create_dir_all(dotfiles.join("nvim/.config/nvim")).unwrap();
        fs::write(dotfiles.join("nvim/.config/nvim/init.lua"), b"-- cfg").unwrap();
        fs::create_dir_all(&target).unwrap();

        let stow = StowConfig {
            dir: dotfiles.to_string_lossy().into(),
            target: Some(target.to_string_lossy().into()),
            packages: vec!["nvim".into()],
            ignore: vec![],
        };
        let mut out = Vec::new();
        stow_symlinks(&stow, &mut out);

        // `.config` doesn't exist in target → the whole subtree folds to one link.
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].link, target.join(".config"));
        assert_eq!(out[0].target, dotfiles.join("nvim/.config"));
    }

    #[test]
    fn stow_unfolds_into_existing_dir() {
        let tmp = tempdir().unwrap();
        let dotfiles = tmp.path().join("dotfiles");
        let target = tmp.path().join("home");
        fs::create_dir_all(dotfiles.join("nvim/.config/nvim")).unwrap();
        fs::write(dotfiles.join("nvim/.config/nvim/init.lua"), b"-- cfg").unwrap();
        // Target already has a real .config dir → must unfold down to it.
        fs::create_dir_all(target.join(".config")).unwrap();

        let stow = StowConfig {
            dir: dotfiles.to_string_lossy().into(),
            target: Some(target.to_string_lossy().into()),
            packages: vec!["nvim".into()],
            ignore: vec![],
        };
        let mut out = Vec::new();
        stow_symlinks(&stow, &mut out);

        // `.config` exists → recurse; `.config/nvim` doesn't → fold there.
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].link, target.join(".config/nvim"));
        assert_eq!(out[0].target, dotfiles.join("nvim/.config/nvim"));
    }

    #[test]
    fn stow_respects_ignore() {
        let tmp = tempdir().unwrap();
        let dotfiles = tmp.path().join("dotfiles");
        let target = tmp.path().join("home");
        fs::create_dir_all(dotfiles.join("git")).unwrap();
        fs::write(dotfiles.join("git/.gitconfig"), b"x").unwrap();
        fs::write(dotfiles.join("git/README.md"), b"x").unwrap();
        fs::create_dir_all(&target).unwrap();

        let stow = StowConfig {
            dir: dotfiles.to_string_lossy().into(),
            target: Some(target.to_string_lossy().into()),
            packages: vec!["git".into()],
            ignore: vec!["README.md".into()],
        };
        let mut out = Vec::new();
        stow_symlinks(&stow, &mut out);

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].link, target.join(".gitconfig"));
    }
}
