//! User-declared package installs (`~/.dots/packages.toml`, `dots packages install`).
//!
//! A hand-authored manifest describing what to install on this machine, via three
//! install *methods* — each its own section, filtered by OS and/or package
//! manager. This is distinct from the built-in [`crate::packages::DEPS`] catalog
//! (dots' own curated deps) and from `links.toml` (symlinks).
//!
//! ```toml
//! # ~/.dots/packages.toml
//!
//! # 1. package manager (batched). `os`/`managers` are optional filters;
//! #    dots supplies the correct assume-yes flag per manager itself.
//! [[group]]
//! os       = ["fedora", "macos"]
//! managers = ["dnf", "brew"]
//! flags    = ["--no-install-recommends"]   # optional extra flags
//! packages = ["ghostty", "zsh", "bash"]
//!
//! # 2. install script (curl | sh, or any shell command). `check` is a binary
//! #    or path; if it already exists the script is skipped (idempotent).
//! [[script]]
//! name  = "herdr"
//! run   = "curl -fsSL https://herdr.dev/install.sh | sh"
//! check = "herdr"
//!
//! # 3. git clone (+ optional post-clone build step in `run`). Skipped if
//! #    `dest` already exists.
//! [[git]]
//! name = "zsh-autosuggestions"
//! repo = "https://github.com/zsh-users/zsh-autosuggestions"
//! dest = "~/.config/zsh/plugins/zsh-autosuggestions"
//! ```
//!
//! A group/script/git entry applies when **both** filter axes pass: `os` is empty
//! or intersects the machine's [`crate::system::os_tags`], and `managers` is empty
//! or contains the active PM. Both empty ⇒ applies everywhere.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::links::expand_path;
use crate::system::{self, Pm};

// ── manifest schema ───────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
pub struct PackagesManifest {
    #[serde(default, rename = "group")]
    pub groups: Vec<Group>,
    #[serde(default, rename = "script")]
    pub scripts: Vec<Script>,
    #[serde(default, rename = "git")]
    pub gits: Vec<GitInstall>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Group {
    #[serde(default)]
    pub os: Vec<String>,
    #[serde(default)]
    pub managers: Vec<String>,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Script {
    pub name: String,
    #[serde(default)]
    pub os: Vec<String>,
    #[serde(default)]
    pub managers: Vec<String>,
    pub run: String,
    #[serde(default)]
    pub check: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitInstall {
    pub name: String,
    #[serde(default)]
    pub os: Vec<String>,
    #[serde(default)]
    pub managers: Vec<String>,
    pub repo: String,
    pub dest: String,
    #[serde(default)]
    pub run: String,
}

pub fn manifest_path() -> PathBuf {
    crate::config::settings::dots_dir().join("packages.toml")
}

pub fn load_manifest() -> Result<PackagesManifest> {
    let path = manifest_path();
    if !path.exists() {
        return Ok(PackagesManifest::default());
    }
    load_manifest_from(&path)
}

/// Parse a `packages.toml` at an arbitrary path (used when importing a manifest
/// from a cloned repo — see [`crate::import`]).
pub fn load_manifest_from(path: &Path) -> Result<PackagesManifest> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

// ── planning ──────────────────────────────────────────────────────────────────

/// A concrete step the manifest resolves to for this machine. Keeping the plan
/// separate from execution is what makes `--dry-run` honest and the resolver
/// unit-testable without touching the system.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Install {
        pm: &'static str,
        program: String,
        argv: Vec<String>,
        packages: Vec<String>,
    },
    Script {
        name: String,
        run: String,
        check: String,
    },
    Git {
        name: String,
        repo: String,
        dest: String,
        run: String,
    },
}

/// An entry that matched but can't run, with why (shown to the user).
#[derive(Debug, PartialEq, Eq)]
pub struct Skip {
    pub what: String,
    pub reason: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub actions: Vec<Action>,
    pub skips: Vec<Skip>,
}

/// Does an entry with these `os`/`managers` filters apply to this machine?
fn applies(os: &[String], managers: &[String], tags: &BTreeSet<String>, active: Option<Pm>) -> bool {
    let os_ok = os.is_empty() || os.iter().any(|t| tags.contains(&t.to_lowercase()));
    let mgr_ok = managers.is_empty()
        || active.is_some_and(|pm| managers.iter().filter_map(|k| Pm::from_key(k)).any(|m| m == pm));
    os_ok && mgr_ok
}

/// Resolve `manifest` against a machine described by `tags` + `active` PM. Pure:
/// no filesystem or process access, so it can be exercised directly in tests.
pub fn plan(manifest: &PackagesManifest, tags: &BTreeSet<String>, active: Option<Pm>) -> Plan {
    let mut plan = Plan::default();

    for g in &manifest.groups {
        if !applies(&g.os, &g.managers, tags, active) {
            continue;
        }
        if g.packages.is_empty() {
            continue;
        }
        match active {
            Some(pm) => {
                let (program, argv) = pm.install_command(&g.flags, &g.packages);
                plan.actions.push(Action::Install {
                    pm: pm.key(),
                    program,
                    argv,
                    packages: g.packages.clone(),
                });
            }
            None => plan.skips.push(Skip {
                what: format!("packages [{}]", g.packages.join(", ")),
                reason: "no supported package manager detected".into(),
            }),
        }
    }

    for s in &manifest.scripts {
        if !applies(&s.os, &s.managers, tags, active) {
            continue;
        }
        if s.run.trim().is_empty() {
            plan.skips.push(Skip {
                what: format!("script '{}'", s.name),
                reason: "empty `run`".into(),
            });
            continue;
        }
        plan.actions.push(Action::Script {
            name: s.name.clone(),
            run: s.run.clone(),
            check: s.check.clone(),
        });
    }

    for gi in &manifest.gits {
        if !applies(&gi.os, &gi.managers, tags, active) {
            continue;
        }
        if gi.repo.trim().is_empty() || gi.dest.trim().is_empty() {
            plan.skips.push(Skip {
                what: format!("git '{}'", gi.name),
                reason: "missing `repo` or `dest`".into(),
            });
            continue;
        }
        plan.actions.push(Action::Git {
            name: gi.name.clone(),
            repo: gi.repo.clone(),
            dest: gi.dest.clone(),
            run: gi.run.clone(),
        });
    }

    plan
}

/// Resolve the manifest for the current machine.
pub fn plan_current(manifest: &PackagesManifest) -> Plan {
    plan(manifest, &system::os_tags(), system::active_pm())
}

// ── execution ─────────────────────────────────────────────────────────────────

/// Run (or, when `dry_run`, merely print) every action in `plan`. Idempotency is
/// evaluated here, at run time: a script whose `check` binary already exists and
/// a git clone whose `dest` already exists are skipped.
pub fn execute(plan: &Plan, dry_run: bool) -> Result<()> {
    for skip in &plan.skips {
        println!("  skip {} — {}", skip.what, skip.reason);
    }

    for action in &plan.actions {
        match action {
            Action::Install {
                pm,
                program,
                argv,
                packages,
            } => {
                println!("  [{pm}] install: {}", packages.join(" "));
                if dry_run {
                    println!("      $ {} {}", program, argv.join(" "));
                    continue;
                }
                run_install(pm, program, argv)?;
            }
            Action::Script { name, run, check } => {
                if !check.is_empty() && system::which_exists(check) {
                    println!("  skip script '{name}' — '{check}' already present");
                    continue;
                }
                println!("  [script] {name}");
                if dry_run {
                    println!("      $ {run}");
                    continue;
                }
                run_shell(run, None).with_context(|| format!("script '{name}'"))?;
            }
            Action::Git {
                name,
                repo,
                dest,
                run,
            } => {
                let dest_path = expand_path(dest);
                if dest_path.exists() {
                    println!("  skip git '{name}' — {} already exists", dest_path.display());
                    continue;
                }
                println!("  [git] {name}: clone {repo} -> {dest}");
                if dry_run {
                    println!("      $ git clone {repo} {}", dest_path.display());
                    if !run.trim().is_empty() {
                        println!("      $ (in {}) {run}", dest_path.display());
                    }
                    continue;
                }
                run_argv(
                    "git",
                    &[
                        "clone".to_string(),
                        repo.clone(),
                        dest_path.display().to_string(),
                    ],
                    None,
                )
                .with_context(|| format!("cloning {repo}"))?;
                if !run.trim().is_empty() {
                    run_shell(run, Some(&dest_path))
                        .with_context(|| format!("post-clone step for '{name}'"))?;
                }
            }
        }
    }
    Ok(())
}

fn run_install(pm: &str, program: &str, argv: &[String]) -> Result<()> {
    // apt reads debconf prompts from a tty; force it non-interactive so an
    // unattended install can't hang waiting on input.
    let env = (pm == "apt").then_some(("DEBIAN_FRONTEND", "noninteractive"));
    let mut cmd = Command::new(program);
    cmd.args(argv);
    if let Some((k, v)) = env {
        cmd.env(k, v);
    }
    let status = cmd
        .status()
        .with_context(|| format!("running {program}"))?;
    if !status.success() {
        bail!("{program} {} failed ({status})", argv.join(" "));
    }
    Ok(())
}

fn run_argv(program: &str, argv: &[String], cwd: Option<&std::path::Path>) -> Result<()> {
    let mut cmd = Command::new(program);
    cmd.args(argv);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let status = cmd
        .status()
        .with_context(|| format!("running {program}"))?;
    if !status.success() {
        bail!("{program} failed ({status})");
    }
    Ok(())
}

fn run_shell(cmd: &str, cwd: Option<&std::path::Path>) -> Result<()> {
    run_argv("sh", &["-c".to_string(), cmd.to_string()], cwd)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn applies_empty_filters_match_everything() {
        assert!(applies(&[], &[], &tags(&["linux"]), Some(Pm::Dnf)));
        assert!(applies(&[], &[], &BTreeSet::new(), None));
    }

    #[test]
    fn applies_os_filter() {
        let t = tags(&["linux", "fedora"]);
        assert!(applies(&["fedora".into()], &[], &t, None));
        assert!(applies(&["macos".into(), "fedora".into()], &[], &t, None));
        assert!(!applies(&["macos".into()], &[], &t, None));
    }

    #[test]
    fn applies_manager_filter() {
        let t = tags(&["linux"]);
        assert!(applies(&[], &["dnf".into()], &t, Some(Pm::Dnf)));
        assert!(!applies(&[], &["dnf".into()], &t, Some(Pm::Apt)));
        assert!(!applies(&[], &["dnf".into()], &t, None));
    }

    #[test]
    fn applies_both_axes_must_pass() {
        let t = tags(&["linux", "fedora"]);
        assert!(applies(&["fedora".into()], &["dnf".into()], &t, Some(Pm::Dnf)));
        // OS matches but manager doesn't:
        assert!(!applies(&["fedora".into()], &["brew".into()], &t, Some(Pm::Dnf)));
    }

    fn sample_manifest() -> PackagesManifest {
        toml::from_str(
            r#"
            [[group]]
            os       = ["fedora"]
            managers = ["dnf", "brew"]
            packages = ["ghostty", "zsh"]

            [[group]]
            os       = ["macos"]
            packages = ["mac-only"]

            [[script]]
            name  = "herdr"
            run   = "curl -fsSL https://herdr.dev/install.sh | sh"
            check = "herdr"

            [[git]]
            name = "zsh-autosuggestions"
            repo = "https://github.com/zsh-users/zsh-autosuggestions"
            dest = "~/.config/zsh/plugins/zsh-autosuggestions"
            "#,
        )
        .unwrap()
    }

    #[test]
    fn plan_selects_matching_entries_on_fedora_dnf() {
        let m = sample_manifest();
        let p = plan(&m, &tags(&["linux", "fedora"]), Some(Pm::Dnf));

        // fedora group installs via dnf; macos group is filtered out; script + git apply.
        assert_eq!(p.skips, vec![]);
        assert_eq!(p.actions.len(), 3);
        assert_eq!(
            p.actions[0],
            Action::Install {
                pm: "dnf",
                program: "sudo".into(),
                argv: vec![
                    "dnf".into(),
                    "install".into(),
                    "-y".into(),
                    "ghostty".into(),
                    "zsh".into()
                ],
                packages: vec!["ghostty".into(), "zsh".into()],
            }
        );
        assert!(matches!(&p.actions[1], Action::Script { name, .. } if name == "herdr"));
        assert!(matches!(&p.actions[2], Action::Git { name, .. } if name == "zsh-autosuggestions"));
    }

    #[test]
    fn plan_group_without_pm_is_skipped_not_installed() {
        let m = sample_manifest();
        // macOS tags, but no package manager present.
        let p = plan(&m, &tags(&["macos"]), None);
        assert!(p.actions.iter().all(|a| !matches!(a, Action::Install { .. })));
        assert_eq!(p.skips.len(), 1);
        assert!(p.skips[0].reason.contains("no supported package manager"));
    }

    #[test]
    fn plan_empty_manifest_is_empty() {
        let p = plan(&PackagesManifest::default(), &tags(&["linux"]), Some(Pm::Dnf));
        assert_eq!(p, Plan::default());
    }

    #[test]
    fn invalid_run_and_repo_produce_skips() {
        let m: PackagesManifest = toml::from_str(
            r#"
            [[script]]
            name = "empty"
            run  = "   "

            [[git]]
            name = "broken"
            repo = ""
            dest = ""
            "#,
        )
        .unwrap();
        let p = plan(&m, &tags(&["linux"]), Some(Pm::Dnf));
        assert_eq!(p.actions, vec![]);
        assert_eq!(p.skips.len(), 2);
    }
}
