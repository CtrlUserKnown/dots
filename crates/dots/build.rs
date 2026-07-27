//! Bakes the real version into the binary at build time, the way lazygit
//! (GoReleaser ldflags) and herdr do — so `dots` never reports a hard-coded
//! version. Resolution order, first match wins:
//!
//! 1. `DOTS_VERSION` env var — CI injects the exact tag here.
//! 2. `git describe --tags` — dev/source builds get the tag (plus a commit
//!    suffix when ahead of it).
//! 3. `CARGO_PKG_VERSION` — last-resort fallback (e.g. a tarball build, no git).
//!
//! A leading `v` is stripped so display code can add its own (`v{VERSION}`).
//!
//! Three vars are emitted, all read through `crate::version`:
//! `DOTS_VERSION`, `DOTS_COMMIT` (short hash, empty when unknown), and
//! `DOTS_VERSION_SOURCE` (which rule above won — shown in `--version`'s long
//! form so a surprising version can be traced back to how it was resolved).

use std::path::Path;
use std::process::Command;

fn main() {
    let cargo = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();

    let (version, source) = resolve(&cargo);
    let version = version.trim().trim_start_matches('v');

    println!("cargo:rustc-env=DOTS_VERSION={version}");
    println!("cargo:rustc-env=DOTS_VERSION_SOURCE={source}");
    println!("cargo:rustc-env=DOTS_COMMIT={}", commit().unwrap_or_default());

    // Rebuild when the checked-out commit/tag or the override changes.
    for p in ["../../.git/HEAD", "../../.git/packed-refs"] {
        if Path::new(p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }
    println!("cargo:rerun-if-env-changed=DOTS_VERSION");
}

/// Pick the version string and record which rule produced it.
fn resolve(cargo: &str) -> (String, &'static str) {
    if let Some(v) = std::env::var("DOTS_VERSION").ok().filter(|v| !v.trim().is_empty()) {
        return (v, "env");
    }

    // `git describe` only yields a usable version when it actually found a tag.
    // With `--always` and no tag in history it degrades to a bare commit hash,
    // which must never become the version: it would compare as older than every
    // release and put self-update into a permanent "update available" state.
    // In that case keep the manifest version and append the hash as build
    // metadata, which semver-ish comparison ignores.
    match describe() {
        Some(d) if d.starts_with('v') => (d, "git-tag"),
        Some(hash) => (format!("{cargo}+g{hash}"), "git-untagged"),
        None => (cargo.to_string(), "cargo"),
    }
}

fn describe() -> Option<String> {
    git(&["describe", "--tags", "--always", "--match", "v*", "--dirty"])
}

fn commit() -> Option<String> {
    git(&["rev-parse", "--short", "HEAD"])
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}
