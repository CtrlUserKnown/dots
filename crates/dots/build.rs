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

use std::path::Path;
use std::process::Command;

fn main() {
    let version = std::env::var("DOTS_VERSION")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(git_describe)
        .unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap_or_default());

    let version = version.trim().trim_start_matches('v');
    println!("cargo:rustc-env=DOTS_VERSION={version}");

    // Rebuild when the checked-out commit/tag or the override changes.
    for p in ["../../.git/HEAD", "../../.git/packed-refs"] {
        if Path::new(p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }
    println!("cargo:rerun-if-env-changed=DOTS_VERSION");
}

fn git_describe() -> Option<String> {
    let out = Command::new("git")
        .args(["describe", "--tags", "--always", "--match", "v*", "--dirty"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}
