# dots — Rust Rewrite: Overview & Ground Rules

## What this is

A full rewrite of the `dots` dotfiles manager. The current implementation is Python (`dots.py`, `ssm.py`, `shared.py`) with a curses TUI. The new implementation is Rust with a `ratatui` TUI, a unified `dots` binary, and a cleaner configuration model.

The rewrite lives in `~/development/dots` (dev repo). The running system at `~/.dots` is **not touched** until the rewrite is complete and the user explicitly migrates.

---

## Prompts in this series

Run these in order. Each one is a self-contained brief for an AI model session.

| # | File | What it builds |
|---|------|----------------|
| 01 | `01-project-setup.md` | Rust workspace, Cargo.toml, directory scaffold, back up Python |
| 02 | `02-config-system.md` | Settings (TOML), `.personal/` layout, app config JSON |
| 03 | `03-symlink-manager.md` | Symlink detection, repair, backup |
| 04 | `04-tui-framework.md` | ratatui skeleton, event loop, terminal-native colors, shared widgets |
| 05 | `05-health-view.md` | Health check screen — symlinks, tools, plugins |
| 06 | `06-update-system.md` | Release tracking (normal) + commit tracking (dev), no-cd-required pull |
| 07 | `07-settings-view.md` | Settings TUI screen, save/load |
| 08 | `08-ssm-core.md` | SSM session storage, OS keychain, herdr/ssh connection |
| 09 | `09-ssm-tui.md` | SSM ratatui screen — list, form, search, help |
| 10 | `10-app-config-manager.md` | Import/export configs, `.personal/` per-app files, non-intrusive apply |
| 11 | `11-alias-system.md` | Default aliases, user-extensible via `.personal/aliases.zsh` |
| 12 | `12-package-installer.md` | brew/dnf/apt abstraction, premade configs (ghostty, neovim, herdr, opencode) |
| 13 | `13-setup-script.md` | New `setup.sh` that builds/downloads and installs the Rust binary |
| 14 | `14-testing.md` | Full test suite — unit, integration, manual scenarios |

---

## Non-negotiable rules for every prompt

These apply to every session, every step:

1. **Clarify before writing code.** Read the relevant existing files. State what you plan to implement. Wait for the user to confirm before writing any code.

2. **Test three ways.** For every feature: (a) happy path, (b) at least one error/edge case, (c) the thing most likely to break in production (missing file, no network, wrong OS, etc.).

3. **Never touch `~/.dots`.** All work happens in `~/development/dots`. The running system is untouched.

4. **No regressions on the running shell.** The `.zshrc` / `rc.zsh` / `.aliases` / `.functions` files in `~/development/dots/src/zsh/` can be modified but must remain valid zsh. If you change the shell entry points for `dots` or `ssm`, test with `zsh -n` before finishing.

5. **Handle errors explicitly.** Every file I/O, network call, subprocess, and keychain access gets an error path. Use `anyhow::Result` and propagate with context strings, not panics.

6. **Keep it snappy.** No unnecessary allocations, no startup work that isn't needed for the current command. The update check runs in a background thread or deferred; it must never block the TUI from opening.

---

## Architecture summary

```
~/.dots/                        ← installed/running version (DO NOT TOUCH)
~/development/dots/             ← this repo (all new code goes here)
  ├── dots-rs/                  ← new Rust project root
  │   ├── Cargo.toml
  │   └── src/
  ├── src/zsh/                  ← existing zsh config (modified to call new binary)
  ├── rewrite-dots-prompts/     ← this folder
  └── _python_backup/           ← backed-up Python files (created in prompt 01)

~/.personal/                    ← user-specific configs (auto-created by dots)
  ├── config.toml               ← user's settings override
  ├── aliases.zsh               ← user's custom aliases
  └── apps/                     ← per-app config files
      ├── ghostty.json
      ├── neovim.json
      └── ...
```

## Key technology choices

| Concern | Library | Docs |
|---------|---------|------|
| TUI | `ratatui` 0.29 | https://ratatui.rs |
| Terminal backend | `crossterm` | https://docs.rs/crossterm |
| CLI args | `clap` 4 (derive) | https://docs.rs/clap |
| Config parsing | `serde` + `toml` | https://serde.rs, https://docs.rs/toml |
| JSON (sessions, app configs) | `serde_json` | https://docs.rs/serde_json |
| OS keychain | `keyring` 3 | https://docs.rs/keyring |
| HTTP (GitHub releases) | `ureq` | https://docs.rs/ureq |
| Error handling | `anyhow` | https://docs.rs/anyhow |
| Path/dirs | `dirs` | https://docs.rs/dirs |

**Avoid:** `tokio`/async for the main binary — this is a local TUI tool, blocking I/O is fine and keeps the binary small.
