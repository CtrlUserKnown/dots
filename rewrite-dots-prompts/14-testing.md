# Prompt 14 — Test Suite

## Before writing any code

1. Read `~/development/dots/tests/test_dots.py` — review the 45 existing tests. Note which categories exist: symlink, settings, update, health, SSM.
2. Read `~/development/dots/tests/test_setup.sh` — review the bash test suite.
3. Read all `*.rs` source files in `~/development/dots/dots-rs/src/` — catalog which modules have inline unit tests and which do not.
4. Read `~/development/dots/dots-rs/Cargo.toml` — confirm `[dev-dependencies]` includes `tempfile` and any other test helpers.
5. State your plan: which test categories you will add, the structure of the integration test harness, and the CI workflow file.
6. **Wait for the user to confirm before writing any code.**

---

## Objective

Build a complete, three-layer test suite for the Rust rewrite:
1. **Unit tests** — per-module, inline, `#[cfg(test)]`
2. **Integration tests** — `tests/` directory, drive the binary as a subprocess
3. **Manual scenarios** — documented checklist run by a human before each release

Plus a GitHub Actions CI workflow that runs layers 1 and 2 on every push and PR.

---

## Layer 1 — Unit tests

Each module must have a `#[cfg(test)]` block. The table below lists the minimum tests required per module. Tests already written in earlier prompts count — add what is missing.

| Module | Tests required |
|--------|---------------|
| `config::settings` | load defaults, load from file, save roundtrip, invalid TOML returns Err |
| `config::personal` | generate roundtrip, version 1 migration, missing keys fill defaults |
| `symlinks` | link points to correct target, broken link detected, repair creates correct link |
| `health` | all deps present → all green, one dep absent → one red |
| `update` | `should_check` timing (fresh stamp = false, stale stamp = true), `check_upstream` with local bare repo |
| `tui::theme` | `set_ghostty_theme` read/write, no existing theme line → appends |
| `ssm::storage` | password not in JSON after save, version 1 migration |
| `ssm::keychain` | `keychain_available` returns bool (either value acceptable), store/load/delete roundtrip |
| `packages` | `detect_pm` returns Homebrew on macOS if brew present, premade backup created |
| `aliases` | parse roundtrip, add/remove user alias |

### Test helpers

Add `tests/common/mod.rs` with shared helpers:

```rust
// Write content to a tempfile and return the path
pub fn write_tmp(content: &str) -> PathBuf;

// Create a temp dir with a fake dots layout (~/.dots stub)
pub fn fake_dots_dir() -> TempDir;

// Create a local bare repo + clone that is N commits behind
pub fn fake_git_repo_behind(n: u32) -> (TempDir, TempDir);
```

---

## Layer 2 — Integration tests

In `tests/integration/`. Each test spawns `cargo run --` (or the compiled binary) as a subprocess and checks stdout/stderr/exit code. Use `assert_cmd` crate for ergonomics.

Add to `Cargo.toml` dev-dependencies:
```toml
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

### Integration tests to write

**Binary basics:**
```rust
#[test]
fn version_flag() {
    Command::cargo_bin("dots").unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains("dots"));
}

#[test]
fn help_flag() {
    Command::cargo_bin("dots").unwrap()
        .arg("--help")
        .assert()
        .success();
}
```

**Health command:**
```rust
#[test]
fn health_exits_zero() {
    Command::cargo_bin("dots").unwrap()
        .arg("health")
        .assert()
        .success();
}
```

**Update check (no network required):**
```rust
#[test]
fn update_check_with_local_repo() {
    let (_bare, local) = fake_git_repo_behind(1);
    let status = dots_update::check_upstream(local.path(), UpdateMode::Dev).unwrap();
    assert_eq!(status.behind, 1);
}
```

**SSM CLI:**
```rust
#[test]
fn ssm_list_empty() {
    let tmp = tempdir().unwrap();
    Command::cargo_bin("dots").unwrap()
        .env("DOTS_SSM_DIR", tmp.path())
        .args(["ssm", "--list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("No sessions"));
}
```

**Alias CLI:**
```rust
#[test]
fn aliases_list() {
    Command::cargo_bin("dots").unwrap()
        .args(["aliases", "list"])
        .assert()
        .success();
}
```

**Setup script (idempotency):**
```bash
# tests/integration/test_setup.sh
bash setup.sh
bash setup.sh   # second run
COUNT=$(grep -c 'dots/bin' ~/.zshrc)
[ "$COUNT" -eq 1 ] || { echo "FAIL: duplicate PATH entry"; exit 1; }
echo "PASS"
```

---

## Layer 3 — Manual scenario checklist

This file lives at `docs/manual-test-checklist.md` and is run by a human before every release.

```markdown
# Manual Test Checklist — vX.Y.Z

Date: __________   Tester: __________

## Setup
- [ ] Fresh `setup.sh` on macOS arm64
- [ ] Fresh `setup.sh` on macOS x86_64
- [ ] Fresh `setup.sh` on Linux (Ubuntu or Fedora)

## TUI golden paths
- [ ] `dots` opens TUI without error
- [ ] Health screen shows all core deps as ✓
- [ ] Settings: toggle greeting, restart, confirm persisted
- [ ] Theme picker: apply a theme, confirm ghostty config updated
- [ ] Aliases: add alias, restart shell, confirm active
- [ ] Profile: generate personal.json, inspect file
- [ ] Update screen: check for updates without network (should show error, not crash)

## SSM golden paths
- [ ] `dots ssm` opens TUI
- [ ] Add a session, confirm in list and sessions.json
- [ ] Connect to a real SSH host (or loopback if available)
- [ ] Duplicate a session, confirm name is unique
- [ ] Delete a session, confirm removed from list and keychain

## Edge cases
- [ ] Resize terminal mid-TUI — no panic
- [ ] Ctrl-C during SSM connection — TUI re-enters cleanly
- [ ] `dots --help` prints all subcommands
- [ ] `dots --version` prints correct version

## Regression: Python parity
For each item, verify the Rust version matches the Python version's behavior:
- [ ] Symlinks created correctly by `dots health --fix`
- [ ] `personal.json` v1 (Python format) imports correctly
- [ ] SSM sessions.json with plaintext passwords (old format) migrates to keychain
```

---

## CI workflow

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: dots-rs
      - name: Unit + integration tests
        run: cargo test --manifest-path dots-rs/Cargo.toml --all
      - name: Clippy
        run: cargo clippy --manifest-path dots-rs/Cargo.toml -- -D warnings
      - name: Format check
        run: cargo fmt --manifest-path dots-rs/Cargo.toml -- --check

  shell-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run bash test suite
        run: bash tests/test_setup.sh
```

---

## Error handling in tests

| Scenario | Expected test behavior |
|----------|----------------------|
| `DOTS_SSM_DIR` env not respected | Test sets the env var and verifies the binary uses it |
| Keychain unavailable in CI | `ssm` tests skip keychain tests with `#[cfg(not(ci))]` or check `keychain_available()` first |
| Network unavailable in CI | Update tests use a local bare git repo, not real network |
| Binary not yet built | `assert_cmd` handles this — prints "binary not found" and fails cleanly |

---

## Testing — three passes (meta: testing the tests)

**Pass 1 — unit tests:**
```sh
cargo test --manifest-path dots-rs/Cargo.toml
# All tests pass. Zero ignored (unless CI-only guard). Zero failures.
```

**Pass 2 — integration tests:**
```sh
cargo test --manifest-path dots-rs/Cargo.toml --test integration
# version_flag, help_flag, health_exits_zero, ssm_list_empty, aliases_list all pass.
```

**Pass 3 — coverage check:**
```sh
cargo install cargo-tarpaulin
cargo tarpaulin --manifest-path dots-rs/Cargo.toml --out Stdout
# Target: ≥ 70% line coverage. Report which modules are under 70% and create issues for them.
```

---

## Completion criteria

- [ ] All unit tests pass with `cargo test`
- [ ] All integration tests pass with `cargo test --test integration`
- [ ] CI workflow runs on push and passes on both `ubuntu-latest` and `macos-latest`
- [ ] Manual checklist document exists in `docs/`
- [ ] Coverage report shows ≥ 70% across the codebase
- [ ] No test calls the network (except if explicitly gated behind a `#[ignore]` flag)
