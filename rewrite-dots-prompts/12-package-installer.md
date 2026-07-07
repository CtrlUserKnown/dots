# Prompt 12 — Package Installer & Premade Configs

## Before writing any code

1. Read `~/development/dots/_python_backup/dots.py` — `install_deps()`, `DEPS`, `_check_dep()`, the `brew` and `brew_cask` helper calls, and the health check loop.
2. Read `~/development/dots/src/zsh/zsh/brew-sync.zsh` — the Brewfile sync logic for go/cargo entries.
3. Read `~/development/dots/dots-rs/src/health/mod.rs` (prompt 05) — confirm what `check_dep` returns.
4. Read `~/development/dots/src/zsh/zsh/dots.py` lines containing `DEPS` list for the full dependency list.
5. State your plan: the package manager abstraction, how premade configs are defined, what "install app" does per tool, and how install integrates with the health screen.
6. **Wait for the user to confirm before writing any code.**

---

## Objective

Implement a cross-platform package installer that:
- Detects the active package manager (Homebrew on macOS/Linux, apt/dnf on Linux)
- Installs `dots` dependencies and optional packages
- Bundles premade configs for specific apps and applies them with user consent
- Integrates with the health screen (prompt 05) — health items become installable from within the TUI

---

## Package manager abstraction

```rust
pub enum PackageManager {
    Homebrew,   // macOS always, Linux if brew in PATH
    Apt,        // Linux, /usr/bin/apt
    Dnf,        // Linux, /usr/bin/dnf
    Unknown,
}

pub fn detect_pm() -> PackageManager;
// Check in order: brew (which brew), apt (/usr/bin/apt), dnf (/usr/bin/dnf)
// On macOS: always Homebrew if brew exists, Unknown otherwise

pub fn install_package(pm: &PackageManager, name: &str) -> anyhow::Result<()>;
// Runs the appropriate command, e.g.:
//   brew install <name>
//   sudo apt install -y <name>
//   sudo dnf install -y <name>
// Streams output to a log buffer; returns Ok(()) on exit 0, Err on non-zero

pub fn is_installed(name: &str) -> bool;
// which(name).is_some()
```

On `Unknown`: do not attempt any install. Return `Err("no supported package manager found; install manually")`.

---

## DEPS list

The full dependency list (map from dependency name to install name per package manager):

| Name | brew | apt | dnf |
|------|------|-----|-----|
| `git` | `git` | `git` | `git` |
| `zsh` | `zsh` | `zsh` | `zsh` |
| `bat` | `bat` | `bat` | `bat` |
| `fzf` | `fzf` | `fzf` | `fzf` |
| `eza` | `eza` | `eza` | `eza` |
| `gh` | `gh` | `gh` | `gh` |
| `fastfetch` | `fastfetch` | `fastfetch` | `fastfetch` |

Optional packages (installed on request):

| Name | brew | apt | dnf | Notes |
|------|------|-----|-----|-------|
| `herdr` | `herdr` | — | — | macOS/brew only |
| `btop` | `btop` | `btop` | `btop` | |
| `lazygit` | `lazygit` | `lazygit` | `lazygit` | |
| `tmux` | `tmux` | `tmux` | `tmux` | |
| `ghostty` | `--cask ghostty` | — | — | macOS brew cask only |
| `neovim` | `neovim` | `neovim` | `neovim` | |

---

## Premade configs

Bundle three premade configs. Each is a static set of files in `~/development/dots/src/premade/`:

```
src/premade/
  ghostty/
    config        ← ghostty config with Catppuccin Mocha theme and sensible defaults
  neovim/
    init.lua      ← minimal neovim config
  opencode/
    config.json   ← opencode config
```

These are **templates** — not applied automatically. The user opts in from the TUI.

```rust
pub struct PremadeConfig {
    pub app:         &'static str,
    pub description: &'static str,
    pub dest:        fn() -> PathBuf,   // e.g. ~/.config/ghostty/config
    pub source:      &'static str,      // relative path inside dots/src/premade/
}

pub fn apply_premade(dots_dir: &Path, entry: &PremadeConfig) -> anyhow::Result<()>;
// If dest exists: create a backup at dest.bak before overwriting
// If dest parent doesn't exist: create it
// Copy source → dest atomically (tmp → rename)
// Flash: "✓ Ghostty config applied (backup at ~/.config/ghostty/config.bak)"
```

---

## TUI Install screen

Accessible from the health screen: when a health item is "Missing", pressing `Enter` or `i` on it opens an inline confirmation prompt:

```
Install bat? [y/N]
```

If the user presses `y`: call `install_package`. Stream output to a scrollable log area in the TUI. On completion, re-run the health check for that item only and update its status.

A separate "Premade Configs" section in the health screen:

```
  premade configs
  ghostty config   [ apply ]
  neovim config    [ apply ]
  opencode config  [ apply ]
```

Pressing `Enter` on a `[ apply ]` item shows:
```
Apply ghostty premade config? Existing config will be backed up. [y/N]
```

---

## CLI interface

```
dots install <name>           install a single dependency
dots install --all            install all missing core deps
dots install --optional       install all optional deps
dots premade apply ghostty    apply ghostty premade config
dots premade list             list available premade configs
```

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| Package manager not found | `Err("no supported package manager found")` |
| Install command exits non-zero | `Err("brew install bat failed (exit 1)")` with stderr |
| `herdr` requested on Linux | `Err("herdr is only available via Homebrew on macOS")` |
| Premade dest is a directory | `Err("destination is a directory: {path}")` |
| Backup write fails | `Err` with context; do not overwrite original |
| `which` returns None | `is_installed` returns false; no crash |

---

## Testing — three passes

**Pass 1 — package manager detection:**
```rust
#[test]
fn detect_pm_homebrew() {
    // If `brew` is in PATH on this test machine:
    if std::process::Command::new("which").arg("brew").status().map(|s| s.success()).unwrap_or(false) {
        assert!(matches!(detect_pm(), PackageManager::Homebrew));
    }
}
```

**Pass 2 — premade config apply with backup:**
```rust
#[test]
fn premade_creates_backup() {
    let tmp = tempdir().unwrap();
    let dest = tmp.path().join("config");
    fs::write(&dest, "original").unwrap();
    apply_premade_to(&src, &dest).unwrap();
    let bak = tmp.path().join("config.bak");
    assert!(bak.exists());
    assert_eq!(fs::read_to_string(&bak).unwrap(), "original");
}
```

**Pass 3 — manual smoke test (TUI):**
Run `cargo run` → Health screen. If `bat` is missing:
- Press `Enter` on the bat row.
- Confirm the install prompt appears.
- Confirm that after install, the health row shows `✓`.

Then navigate to a premade config row:
- Press `Enter` on ghostty.
- Confirm the apply prompt appears.
- If you press `y`, confirm `~/.config/ghostty/config.bak` was created and the new config is in place.

---

## Completion criteria

- [ ] `detect_pm()` returns correct result on this machine
- [ ] Health screen items with status Missing show an install prompt
- [ ] Premade config apply creates a backup of the existing config
- [ ] `herdr` on Linux returns a clean error, not a crash
- [ ] All three tests pass
