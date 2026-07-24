//! The unified dots manifest.
//!
//! One `dots.toml` describes both **symlinks** (`[[link]]`, `[stow]`) and
//! **package installs** (`[[group]]`, `[[script]]`, `[[git]]`) — a single,
//! shareable artifact that lives at the root of the user's dotfiles repo.
//!
//! It is read from the managed source repo (`~/.dots/src/dots.toml`) when one
//! has been pulled (see [`crate::import`]), otherwise from `~/.dots/dots.toml`.
//! For backward compatibility the legacy split files — `~/.dots/links.toml`
//! (shipped in v2.2.0) and `~/.dots/packages.toml` — are still read and merged
//! on top, so existing setups keep working with nothing to migrate.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::config::settings::dots_dir;
use crate::links::{LinkEntry, LinksManifest, StowConfig};
use crate::user_packages::{GitInstall, Group, PackagesManifest, Script};

#[derive(Debug, Default, Deserialize)]
pub struct DotsManifest {
    #[serde(default, rename = "link")]
    pub links: Vec<LinkEntry>,
    #[serde(default)]
    pub stow: Option<StowConfig>,
    #[serde(default, rename = "group")]
    pub groups: Vec<Group>,
    #[serde(default, rename = "script")]
    pub scripts: Vec<Script>,
    #[serde(default, rename = "git")]
    pub gits: Vec<GitInstall>,
}

impl DotsManifest {
    /// Fold the legacy split manifests in on top of `self`. Link entries are
    /// de-duplicated by target so the same link declared in both files isn't
    /// planned twice; a legacy `[stow]` is used only if the unified file has none.
    fn merge(&mut self, links: LinksManifest, packages: PackagesManifest) {
        for e in links.links {
            if !self.links.iter().any(|x| x.target == e.target) {
                self.links.push(e);
            }
        }
        if self.stow.is_none() {
            self.stow = links.stow;
        }
        self.groups.extend(packages.groups);
        self.scripts.extend(packages.scripts);
        self.gits.extend(packages.gits);
    }

    /// View of just the symlink sections, for the [`crate::links`] planner.
    pub fn to_links_manifest(&self) -> LinksManifest {
        LinksManifest {
            links: self.links.clone(),
            stow: self.stow.clone(),
        }
    }

    /// View of just the package sections, for the [`crate::user_packages`] planner.
    pub fn to_packages_manifest(&self) -> PackagesManifest {
        PackagesManifest {
            groups: self.groups.clone(),
            scripts: self.scripts.clone(),
            gits: self.gits.clone(),
        }
    }
}

/// Where the effective `dots.toml` lives: the pulled source repo if present,
/// else the `~/.dots` root.
pub fn manifest_path() -> PathBuf {
    let src = dots_dir().join("src").join("dots.toml");
    if src.is_file() {
        src
    } else {
        dots_dir().join("dots.toml")
    }
}

/// Parse a `dots.toml` at an explicit path (used when previewing a cloned repo).
pub fn load_from(path: &Path) -> Result<DotsManifest> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// The effective manifest for this machine: the unified `dots.toml` (if any)
/// with the legacy `links.toml` / `packages.toml` merged on top. A malformed
/// legacy file is skipped rather than failing the whole load.
pub fn load() -> Result<DotsManifest> {
    let mut manifest = if manifest_path().is_file() {
        load_from(&manifest_path())?
    } else {
        DotsManifest::default()
    };
    manifest.merge(
        crate::links::load_manifest().unwrap_or_default(),
        crate::user_packages::load_manifest().unwrap_or_default(),
    );
    Ok(manifest)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_sections_from_one_file() {
        let m = load_from_str(
            r#"
            [[link]]
            source = "~/dots/starship.toml"
            target = "~/.config/starship.toml"

            [stow]
            dir      = "~/.dots/src"
            packages = ["nvim"]

            [[group]]
            os = ["fedora"]
            packages = ["ripgrep"]

            [[script]]
            name = "herdr"
            run  = "curl x | sh"

            [[git]]
            name = "fzf-tab"
            repo = "https://github.com/Aloxaf/fzf-tab"
            dest = "~/.config/zsh/plugins/fzf-tab"
            "#,
        );
        assert_eq!(m.links.len(), 1);
        assert!(m.stow.is_some());
        assert_eq!(m.groups.len(), 1);
        assert_eq!(m.scripts.len(), 1);
        assert_eq!(m.gits.len(), 1);

        // the split views carry the right sections through
        assert_eq!(m.to_links_manifest().links.len(), 1);
        assert_eq!(m.to_packages_manifest().groups.len(), 1);
    }

    #[test]
    fn merge_dedupes_links_and_keeps_existing_stow() {
        let mut base = load_from_str(
            r#"
            [[link]]
            source = "~/a"
            target = "~/.config/a"
            [stow]
            dir = "~/.dots/src"
            "#,
        );
        let legacy: LinksManifest = toml::from_str(
            r#"
            [[link]]
            source = "~/a-dup"
            target = "~/.config/a"   # same target → deduped away
            [[link]]
            source = "~/b"
            target = "~/.config/b"   # new → kept
            [stow]
            dir = "~/legacy"          # ignored, base already has a stow
            "#,
        )
        .unwrap();
        base.merge(legacy, PackagesManifest::default());

        let targets: Vec<_> = base.links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(targets, vec!["~/.config/a", "~/.config/b"]);
        assert_eq!(base.stow.unwrap().dir, "~/.dots/src");
    }

    #[test]
    fn merge_appends_legacy_packages() {
        let mut base = DotsManifest::default();
        let pkgs: PackagesManifest = toml::from_str(
            r#"
            [[group]]
            packages = ["zsh"]
            "#,
        )
        .unwrap();
        base.merge(LinksManifest::default(), pkgs);
        assert_eq!(base.groups.len(), 1);
    }

    fn load_from_str(s: &str) -> DotsManifest {
        toml::from_str(s).unwrap()
    }
}
