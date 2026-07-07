# Prompt 06 — Update System

## Before writing any code

1. Read `~/development/dots/_python_backup/shared.py` — `check_upstream()`.
2. Read `~/development/dots/_python_backup/dots.py` — `do_pull()` and `run_check_updates_view()`.
3. Read `~/development/dots/src/zsh/zsh/update-check.zsh` to understand the shell-side update logic and how it compares to the Python logic. Note the frequency comparison (`_dots_freq * 60` — already fixed).
4. State your plan: how the update check determines "behind" in normal vs dev mode, how the pull+relink is done atomically, and how the stamp file is managed.
5. **Wait for the user to confirm before writing any code.**

---

## Objective

Port the update system to Rust. Key improvements over the Python version:

1. The user does **not** need to be in `~/.dots` to trigger an update — all git operations use an explicit `-C <path>` equivalent (path passed to `Command`).
2. Normal mode tracks GitHub releases (git tags). Dev mode tracks commits on `origin/HEAD`.
3. The update check is non-blocking — it runs in a background thread and the TUI opens immediately. The result is posted back when ready.
4. A stamp file throttles the check per `update_frequency` setting.

---

## Core types

```rust
pub enum UpdateMode { Normal, Dev }

pub struct UpdateStatus {
    pub behind: u32,         // 0 = up to date
    pub label: String,       // release tag (normal) or short SHA (dev)
}

pub fn check_upstream(dots_dir: &Path, mode: UpdateMode) -> anyhow::Result<UpdateStatus>;
pub fn do_pull(dots_dir: &Path) -> anyhow::Result<String>; // returns new version string
pub fn should_check(dots_dir: &Path, freq_minutes: u64) -> bool;
pub fn record_check(dots_dir: &Path) -> anyhow::Result<()>;
```

---

## `check_upstream` implementation

Use `std::process::Command` (not `git2`) — keeps the binary small and avoids a native dep.

```rust
// Normal mode: tags
// git -C <dots_dir> fetch --depth 1 --tags origin
// git -C <dots_dir> tag --list 'v*' --sort=-version:refname | head -1

// Dev mode: commits
// git -C <dots_dir> fetch --depth 1 origin
// git -C <dots_dir> rev-list --count HEAD..origin/HEAD
// git -C <dots_dir> rev-parse --short origin/HEAD
```

All `Command` calls get a 15-second timeout via a thread + `recv_timeout`. If the timeout fires, return `Err("update check timed out")`.

---

## `do_pull` implementation

```
1. git -C <dots_dir> pull --ff-only
2. If exit 0: run symlink repair (call symlinks::repair_all)
3. Read new version from: git -C <dots_dir> describe --tags --abbrev=0
4. Write new version to stamp file
5. Return new version string
```

If `pull --ff-only` fails (e.g. local changes), return `Err` with the stderr output from git as the error message — do not silently swallow it.

---

## Stamp file

Location: `~/.dots/.update_stamp` (seconds since epoch as a decimal string).

`should_check()`:
```rust
let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
let last = read_stamp(dots_dir).unwrap_or(0);
now.saturating_sub(last) > freq_minutes * 60
```

`record_check()` writes `now` to the stamp file atomically (tmp → rename).

---

## Background check in the TUI

```rust
// In app.rs, after opening the TUI:
let (tx, rx) = std::sync::mpsc::channel();
let dots_dir_clone = dots_dir.clone();
let mode = if settings.dots.developer_mode { UpdateMode::Dev } else { UpdateMode::Normal };
std::thread::spawn(move || {
    if should_check(&dots_dir_clone, settings.dots.update_frequency) {
        let _ = record_check(&dots_dir_clone);
        let result = check_upstream(&dots_dir_clone, mode);
        let _ = tx.send(result);
    }
});

// In the event loop, after polling for key events:
if let Ok(result) = rx.try_recv() {
    app.update_status = Some(result);
    // If behind > 0: set a flash: "📦 Update available — open Settings to apply"
}
```

This must never block the TUI render loop. `try_recv` is non-blocking.

---

## TUI Update screen

Implement `src/tui/update.rs` as a screen the user navigates to from Settings:

```
─────────── check for updates ───────────
  Checking for updates…          (initial state)

  ─ or ─

  ✓  No updates — you're on v1.5.5

  ─ or ─

  📦 Update available: v1.5.5 → v1.6.0
     Press y to update, any other key to skip.
──────────── y update  q back ───────────
```

States: `Checking`, `UpToDate`, `Available { behind, label }`, `Pulling`, `Done { new_ver }`, `Error(String)`.

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| No internet / git fetch fails | Return `Err`; TUI shows `"✗ Could not reach upstream"` |
| `pull --ff-only` fails (local changes) | Show git's stderr in the error state; do not force-pull |
| Stamp file unreadable / corrupt | Treat as `last_check = 0` (check will fire) |
| Stamp file write fails | Log warning; update check still proceeds |
| Background thread panics | The `tx.send()` never fires; `rx.try_recv()` keeps returning `Err(Empty)` — this is safe |

---

## Testing — three passes

**Pass 1 — `should_check` timing:**
```rust
#[test]
fn check_throttled() {
    let tmp = tempdir().unwrap();
    // Write stamp as "now"
    record_check(tmp.path()).unwrap();
    // Immediately: should_check must return false for freq=1440
    assert!(!should_check(tmp.path(), 1440));
}
#[test]
fn check_stale() {
    let tmp = tempdir().unwrap();
    // Write stamp as "now - 90000 seconds"
    write_stamp(tmp.path(), unix_now() - 90_000);
    assert!(should_check(tmp.path(), 1440));
}
```

**Pass 2 — `check_upstream` with a real local git repo:**
Create a tmp bare repo + local clone that is 1 commit behind (mirrors the Python test in `test_dots.py`). Call `check_upstream` in normal mode — assert `behind == 1`.

**Pass 3 — manual background check:**
Run `cargo run`. Confirm the TUI opens without delay. After a few seconds, if an update is available, a flash message appears. If up to date, no flash.

---

## Completion criteria

- [ ] `cargo run -- update` opens the update screen and shows real status
- [ ] `should_check` tests pass
- [ ] Background check does not block TUI startup
- [ ] `do_pull` runs symlink repair after pulling
- [ ] No panics when `~/.dots/.git` is absent
