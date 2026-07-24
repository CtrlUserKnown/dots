//! Import a dots config from a GitHub repo (`dots get-config` / `dots gc`).
//!
//! Resolves a `user/repo` shorthand (or a full git URL) into a clone URL — the
//! same spec model Neovim's `vim.pack` uses — clones it under
//! `~/.dots/imports/<name>`, then discovers and applies any `packages.toml` it
//! finds (see [`crate::user_packages`]).
//!
//! ```text
//! dots gc -u='user/repo'            # clone github.com/user/repo, preview, apply
//! dots gc -u='user/repo@main'       # pin a branch, tag, or commit
//! dots gc -u='https://…' --dry-run  # full URL; clone + preview, run nothing
//! ```
//!
//! **Security:** an imported `packages.toml` can contain `[[script]]` entries
//! that run arbitrary shell (`curl | sh`). The CLI therefore previews every
//! action and requires confirmation before executing — see `cli_get_config`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::config::settings::dots_dir;

/// A parsed import spec: the clone URL, an optional ref (branch/tag/commit) to
/// check out, and the local name (last path segment, `.git` stripped) that names
/// the clone directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub url: String,
    pub reference: Option<String>,
    pub name: String,
}

/// Resolve a user-supplied spec into a [`Source`].
///
/// * `user/repo`        → `https://github.com/user/repo.git`
/// * `user/repo@ref`    → …with `ref` checked out after clone
/// * `https://…`, `git@host:…`, `ssh://…`, `file://…` → passed through unchanged
pub fn resolve(spec: &str) -> Result<Source> {
    let spec = spec.trim();
    if spec.is_empty() {
        bail!("empty import spec");
    }

    let (url, reference) = if is_full_url(spec) {
        // A full URL owns its own ref handling; don't try to split `@`, which
        // would misfire on the scp-style `git@host:…` and `ssh://git@…` forms.
        (spec.to_string(), None)
    } else {
        let (repo_part, reference) = match spec.rsplit_once('@') {
            Some((r, rf)) => (r, Some(rf.to_string())),
            None => (spec, None),
        };
        let segs: Vec<&str> = repo_part.split('/').filter(|s| !s.is_empty()).collect();
        if segs.len() != 2 {
            bail!("expected 'user/repo', 'user/repo@ref', or a full git URL — got '{spec}'");
        }
        (
            format!("https://github.com/{}/{}.git", segs[0], segs[1]),
            reference,
        )
    };

    let name = infer_name(&url);
    if name.is_empty() {
        bail!("could not infer a repo name from '{spec}'");
    }
    Ok(Source { url, reference, name })
}

/// Whether `spec` is already a full clone URL rather than a `user/repo`
/// shorthand: any `scheme://…`, or the scp-style `git@host:path` form.
fn is_full_url(spec: &str) -> bool {
    spec.contains("://") || (spec.contains('@') && spec.contains(':'))
}

/// Repo name from a clone URL: strip a trailing `.git`, then take the final path
/// segment (splitting on `/` or `:` to also handle `git@host:user/repo`).
fn infer_name(url: &str) -> String {
    let u = url.strip_suffix(".git").unwrap_or(url);
    let u = u.trim_end_matches('/');
    u.rsplit(['/', ':']).next().unwrap_or("").to_string()
}

/// The managed source repo location — the single dotfiles repo `dots gc` pulls.
pub fn source_dir() -> PathBuf {
    dots_dir().join("src")
}

/// Clone `source` into the managed source repo (`~/.dots/src`).
pub fn clone_source(source: &Source, force: bool) -> Result<PathBuf> {
    clone_at(source, &source_dir(), force)
}

/// Clone `source` into `<base>/<name>` (used in tests). See [`clone_at`].
pub fn clone_into(source: &Source, base: &Path, force: bool) -> Result<PathBuf> {
    clone_at(source, &base.join(&source.name), force)
}

/// Clone `source` into exactly `dest`, checking out `source.reference` if set.
///
/// A full (non-shallow) clone is used so an arbitrary commit ref can be checked
/// out. If `dest` already exists it is an error unless `force`, which removes and
/// re-clones it.
pub fn clone_at(source: &Source, dest: &Path, force: bool) -> Result<PathBuf> {
    if dest.exists() {
        if !force {
            bail!(
                "{} already exists — pass --force to replace it, or remove it first",
                dest.display()
            );
        }
        std::fs::remove_dir_all(dest)
            .with_context(|| format!("removing {}", dest.display()))?;
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }

    run_git(
        &[
            "clone".to_string(),
            source.url.clone(),
            dest.display().to_string(),
        ],
        None,
    )
    .with_context(|| format!("cloning {}", source.url))?;

    if let Some(reference) = &source.reference {
        run_git(&["checkout".to_string(), reference.clone()], Some(dest))
            .with_context(|| format!("checking out '{reference}'"))?;
    }
    Ok(dest.to_path_buf())
}

/// Fast-forward pull the git repo at `dir` (the managed source). Errors if it
/// isn't a repo or the pull can't fast-forward.
pub fn pull(dir: &Path) -> Result<()> {
    run_git(&["pull".to_string(), "--ff-only".to_string()], Some(dir))
        .with_context(|| format!("pulling {}", dir.display()))
}

/// Config manifests discovered at the root of a cloned repo.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Discovered {
    pub packages: Option<PathBuf>,
    pub links: Option<PathBuf>,
}

pub fn discover(repo: &Path) -> Discovered {
    let packages = repo.join("packages.toml");
    let links = repo.join("links.toml");
    Discovered {
        packages: packages.is_file().then_some(packages),
        links: links.is_file().then_some(links),
    }
}

fn run_git(argv: &[String], cwd: Option<&Path>) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.args(argv);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let status = cmd.status().context("running git (is it installed?)")?;
    if !status.success() {
        bail!("git {} failed ({status})", argv.join(" "));
    }
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolve_shorthand() {
        let s = resolve("user/repo").unwrap();
        assert_eq!(s.url, "https://github.com/user/repo.git");
        assert_eq!(s.reference, None);
        assert_eq!(s.name, "repo");
    }

    #[test]
    fn resolve_shorthand_with_ref() {
        let s = resolve("user/my-dots@main").unwrap();
        assert_eq!(s.url, "https://github.com/user/my-dots.git");
        assert_eq!(s.reference.as_deref(), Some("main"));
        assert_eq!(s.name, "my-dots");
    }

    #[test]
    fn resolve_ref_with_slash() {
        let s = resolve("user/repo@feature/x").unwrap();
        assert_eq!(s.reference.as_deref(), Some("feature/x"));
    }

    #[test]
    fn resolve_full_https_url() {
        let s = resolve("https://github.com/user/repo").unwrap();
        assert_eq!(s.url, "https://github.com/user/repo");
        assert_eq!(s.reference, None);
        assert_eq!(s.name, "repo");
    }

    #[test]
    fn resolve_scp_style_url_is_passthrough() {
        // The '@' here is auth, not a ref — must not be split.
        let s = resolve("git@github.com:user/repo.git").unwrap();
        assert_eq!(s.url, "git@github.com:user/repo.git");
        assert_eq!(s.reference, None);
        assert_eq!(s.name, "repo");
    }

    #[test]
    fn resolve_rejects_bad_specs() {
        assert!(resolve("").is_err());
        assert!(resolve("justoneword").is_err());
        assert!(resolve("too/many/segments").is_err());
    }

    fn git(argv: &[&str], cwd: &Path) {
        let owned: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
        run_git(&owned, Some(cwd)).unwrap();
    }

    #[test]
    fn clone_into_local_repo_offline() {
        // Build a tiny source repo with a manifest, no network involved.
        let src = tempdir().unwrap();
        git(&["init", "-q", "-b", "main"], src.path());
        std::fs::write(src.path().join("packages.toml"), "# imported\n").unwrap();
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "add", "."], src.path());
        git(
            &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "init"],
            src.path(),
        );

        let source = Source {
            url: format!("file://{}", src.path().display()),
            reference: None,
            name: "imported-cfg".into(),
        };
        let base = tempdir().unwrap();

        let dest = clone_into(&source, base.path(), false).unwrap();
        assert!(dest.join("packages.toml").is_file());
        assert_eq!(discover(&dest).packages, Some(dest.join("packages.toml")));

        // Re-clone without force fails; with force it succeeds.
        assert!(clone_into(&source, base.path(), false).is_err());
        assert!(clone_into(&source, base.path(), true).is_ok());
    }
}
