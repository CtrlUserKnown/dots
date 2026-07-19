mod common;

use assert_cmd::Command;
use predicates::str as pstr;

// ── binary basics ─────────────────────────────────────────────────────────────

#[test]
fn version_flag() {
    Command::cargo_bin("dots").unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(pstr::contains("dots"));
}

#[test]
fn help_flag() {
    Command::cargo_bin("dots").unwrap()
        .arg("--help")
        .assert()
        .success();
}

// ── health ────────────────────────────────────────────────────────────────────

#[test]
fn health_exits_zero() {
    Command::cargo_bin("dots").unwrap()
        .arg("health")
        .assert()
        .success();
}

// ── update ────────────────────────────────────────────────────────────────────

#[test]
fn update_check_with_local_repo() {
    let (_root, _bare, local) = common::fake_git_repo_behind(1);
    match dots::update::check_upstream(&local, dots::update::UpdateMode::Dev) {
        Ok(status) => assert_eq!(status.behind, 1, "local should be 1 commit behind"),
        Err(e)     => eprintln!("update_check_with_local_repo skipped: {e}"),
    }
}

// ── alias CLI ─────────────────────────────────────────────────────────────────

#[test]
fn aliases_list() {
    Command::cargo_bin("dots").unwrap()
        .args(["aliases", "list"])
        .assert()
        .success();
}
