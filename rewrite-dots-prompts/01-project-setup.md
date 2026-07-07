# Prompt 01 — Project Setup & Python Backup

## Before writing any code

1. Read `~/development/dots/src/zsh/zsh/dots.py`, `ssm.py`, and `shared.py` to understand what the Python implementation does.
2. Run `ls ~/development/dots/` and `ls ~/.dots/` to confirm the two directories are separate (different inodes).
3. Read `~/development/dots/src/zsh/zsh/.functions` to see how `dots` and `ssm` are invoked from the shell.
4. State your plan: what directories you will create, what you will back up, and what the final `Cargo.toml` will contain.
5. **Wait for the user to confirm before writing any files.**

---

## Objective

Set up the Rust workspace inside `~/development/dots/dots-rs/` and back up the Python files. Nothing on the running system (`~/.dots`) changes.

---

## Step 1 — Back up Python files

Create `~/development/dots/_python_backup/` and copy (do not move) the following:

```
src/zsh/zsh/dots.py     → _python_backup/dots.py
src/zsh/zsh/ssm.py      → _python_backup/ssm.py
src/zsh/zsh/shared.py   → _python_backup/shared.py
```

Verify each copy with a byte-count diff (`wc -c`). If any copy does not match, abort and report.

---

## Step 2 — Create `dots-rs/` workspace

Create `~/development/dots/dots-rs/` with the following layout:

```
dots-rs/
├── Cargo.toml
└── src/
    ├── main.rs         ← entry point, CLI dispatch
    ├── config/
    │   ├── mod.rs
    │   ├── settings.rs ← dots settings (TOML)
    │   └── personal.rs ← ~/.personal layout
    ├── symlinks.rs     ← symlink management
    ├── update.rs       ← update check + pull logic
    ├── packages.rs     ← brew/dnf/apt abstraction
    ├── tui/
    │   ├── mod.rs
    │   ├── app.rs      ← main TUI loop
    │   ├── health.rs
    │   ├── theme.rs
    │   ├── settings.rs
    │   └── update.rs
    └── ssm/
        ├── mod.rs
        ├── storage.rs  ← sessions + keychain
        ├── connect.rs  ← ssh / herdr connection
        └── tui.rs      ← SSM ratatui screen
```

---

## Step 3 — `Cargo.toml`

```toml
[package]
name = "dots"
version = "0.1.0"
edition = "2021"
description = "dots dotfiles manager"

[[bin]]
name = "dots"
path = "src/main.rs"

[dependencies]
# TUI
ratatui     = "0.29"
crossterm   = "0.28"

# CLI
clap        = { version = "4", features = ["derive"] }

# Serialization
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
toml        = "0.8"

# OS keychain (macOS: Security framework, Linux: secret-service)
keyring     = { version = "3", features = ["apple-native", "sync-secret-service"] }

# HTTP — GitHub release checks
ureq        = { version = "2", features = ["json"] }

# Error handling
anyhow      = "1"

# Platform dirs (~/.config, ~/ etc.)
dirs        = "5"

[profile.release]
opt-level   = "z"    # minimize binary size
lto         = true
strip       = true
```

**Important:** `keyring` with `sync-secret-service` requires `libdbus-1-dev` on Linux. Document this in the compile error note below.

---

## Step 4 — `src/main.rs` skeleton

Write a minimal `main.rs` that:
1. Parses CLI args with `clap` — subcommands: `health`, `update`, `ssm`, and a bare invocation that opens the TUI.
2. Prints `"dots vX.Y.Z — TUI not yet implemented"` for every subcommand (stubs).
3. Compiles and runs cleanly: `cargo run -- --help` and `cargo run -- health` must both work.

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "dots", version, about = "dots dotfiles manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Check symlinks, tools, and plugins
    Health,
    /// Check for and apply updates
    Update,
    /// SSH session manager
    Ssm {
        /// Connect directly: user@host[:port]
        #[arg(short = 'c')]
        connect: Option<String>,
        /// List saved sessions
        #[arg(short = 'l')]
        list: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None              => println!("TUI — not yet implemented"),
        Some(Command::Health) => println!("health — not yet implemented"),
        Some(Command::Update) => println!("update — not yet implemented"),
        Some(Command::Ssm { .. }) => println!("ssm — not yet implemented"),
    }
    Ok(())
}
```

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| `cargo build` fails on `keyring` (missing libdbus on Linux) | Print clear note: "on Linux, install `libdbus-1-dev` before building" |
| `_python_backup/` already exists | Do not overwrite; print a warning and skip |
| `dots-rs/` already exists | Do not re-init; print a warning and exit cleanly |

---

## Testing — run all three before finishing

1. **Build test:** `cd dots-rs && cargo build` — must compile with zero errors and zero warnings (fix all warnings before moving on).
2. **Help test:** `cargo run -- --help` — must list all subcommands.
3. **Subcommand test:** Run `cargo run -- health`, `cargo run -- update`, `cargo run -- ssm -l` — each must print its stub message and exit 0.

If any test fails, fix it before marking this step complete.

---

## Completion criteria

- [ ] `_python_backup/` contains correct copies of all three Python files
- [ ] `dots-rs/Cargo.toml` exists with all listed dependencies
- [ ] Full directory scaffold exists (all `mod.rs` stubs, even if empty `mod` blocks)
- [ ] `cargo build` clean
- [ ] All three manual tests pass
