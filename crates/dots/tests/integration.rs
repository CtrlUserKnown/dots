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
fn install_source_is_detectable_offline() {
    // Self-update gating must resolve without any network. A package-manager
    // install yields a defer message; a self-managed one does not.
    use dots::update::{install_source, InstallSource};
    match install_source() {
        InstallSource::SelfManaged => {}
        InstallSource::PackageManager(_) => {
            assert!(install_source().defer_message().is_some());
        }
    }
    // The baked-in version is always present.
    assert!(!dots::update::CURRENT.is_empty());
}

// ── alias CLI ─────────────────────────────────────────────────────────────────

#[test]
fn aliases_list() {
    Command::cargo_bin("dots").unwrap()
        .args(["aliases", "list"])
        .assert()
        .success();
}
