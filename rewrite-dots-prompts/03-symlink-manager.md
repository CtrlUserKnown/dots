# Prompt 03 — Symlink Manager

## Before writing any code

1. Read `~/development/dots/_python_backup/dots.py` — focus on `get_symlinks()` (line ~98), `check_symlink()`, `repair_symlink()`, `repair_all()`, and `_backup_path()`.
2. Run `find ~/.config -maxdepth 2 -type l` and print each symlink with its target. Note which point to `~/.dots` (correct) vs anywhere else (suspect).
3. Read `~/development/dots/dots-rs/src/symlinks.rs` (should be an empty stub from prompt 01).
4. State your plan: the `Symlink` struct, all status variants, the backup naming scheme, and which symlinks are "core" vs "conditional."
5. **Wait for the user to confirm before writing any code.**

---

## Objective

Port the symlink management logic from Python to Rust. This is the core of `dots health --fix` and the setup repair flow.

---

## Status enum

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum SymlinkStatus {
    Ok,
    Missing,       // neither symlink nor real file exists at link path
    Broken,        // is a symlink but target does not exist
    NotALink,      // path exists but is a real file or directory
    WrongTarget,   // is a symlink but points somewhere else
}
```

---

## Core logic

```rust
pub struct Symlink {
    pub link:   PathBuf,   // e.g. ~/.config/zsh
    pub target: PathBuf,   // e.g. ~/.dots/src/zsh/zsh
}

pub fn get_symlinks() -> Vec<Symlink>;
// Returns the canonical list. Conditional entries:
// - ghostty: only if `which ghostty` succeeds
// - herdr:   only if `which herdr` succeeds
// Core (always present):
// - ~/.config/bat        → ~/.dots/src/bat
// - ~/.config/fastfetch  → ~/.dots/src/fastfetch
// - ~/.config/zsh        → ~/.dots/src/zsh/zsh
// - ~/.zshrc             → ~/.dots/src/zsh/.zshrc

pub fn check(s: &Symlink) -> SymlinkStatus;

pub fn repair(s: &Symlink) -> anyhow::Result<()>;
// If NotALink:
//   - directory: move to backup path (see below)
//   - regular file: check version header match (see below); if same version, return Ok; else move to backup
// If Broken or WrongTarget: unlink, then symlink
// If Missing: symlink directly

pub fn repair_all() -> anyhow::Result<RepairReport>;

pub struct RepairReport {
    pub ok:       usize,
    pub repaired: usize,
    pub skipped:  usize,  // file in the way that could not be moved
}
```

---

## Version header check

Before backing up a regular file that is in the way of a symlink, read its first 10 lines and look for a line matching `# dotfiles vX.Y.Z`. If the version string matches the target file's version header, the file is already up-to-date — return `Ok` without creating a backup or symlink (user has a real file that is identical to what the symlink would point to).

```rust
pub fn read_version_header(path: &Path) -> Option<String>;
// Returns the version string (e.g. "1.5.5") or None
```

---

## Backup naming

```
original path:        ~/.zshrc
backup (no conflict): ~/.zshrc.bak.20260707
backup (conflict):    ~/.zshrc.bak.20260707.1
                      ~/.zshrc.bak.20260707.2
                      ...
```

---

## `src/main.rs` — wire in `repair_all`

Add to the `Health` subcommand stub:
```rust
Some(Command::Health) => {
    let report = dots::symlinks::repair_all()?;
    println!("  {} OK, {} repaired, {} skipped",
             report.ok, report.repaired, report.skipped);
}
```

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| `fs::rename` fails moving real file to backup (cross-device) | Fall back to `fs::copy` + `fs::remove_file`; if that also fails, add to `skipped` |
| `symlink` syscall fails (permissions) | Add to `skipped` with error context, continue loop |
| Target path (`~/.dots/src/...`) does not exist | `SymlinkStatus::Broken` — do not panic |
| `get_symlinks()` called on a system where `~/.dots` does not exist | Return empty `Vec` with a warning, not an error |

---

## Testing — three passes

**Pass 1 — `check()` unit test:**
```rust
#[test]
fn check_missing() {
    let tmp = tempdir().unwrap();
    let s = Symlink {
        link: tmp.path().join("link"),
        target: tmp.path().join("target"),
    };
    assert_eq!(check(&s), SymlinkStatus::Missing);
}
// Also test: Ok, Broken, NotALink, WrongTarget
```

**Pass 2 — `repair()` unit test:**
Create a temp dir with a real file in the link position. Call `repair()`. Assert:
- The original file was moved to a `.bak.DATE` path
- A symlink now exists at `link` pointing to `target`

**Pass 3 — version header skip:**
Create a temp dir. Put a file at `link` that contains `# dotfiles v1.5.5`. Put the same file at `target`. Call `repair()`. Assert:
- No `.bak` file was created
- The original file is still at `link` (not replaced with a symlink)

Run `cargo test symlinks` — all must pass.

---

## Completion criteria

- [ ] All five `SymlinkStatus` variants implemented and tested
- [ ] `repair_all()` returns a correct `RepairReport`
- [ ] `cargo test symlinks` — all pass
- [ ] `cargo run -- health` no longer prints "not yet implemented" — it runs `repair_all` and prints a report
