# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [2.3.1] - 2026-07-29

### Added
- **Dashboard blocks editor** — Settings → **Dashboard blocks** opens an in-TUI editor over `layout.toml`: swap, add, remove, and reorder the widgets in each zone (built-ins and live plugin panes alike), applied to disk as you go. Zone geometry (columns/span/weight) still requires hand-editing `layout.toml`
- `install.sh` now verifies a downloaded release tarball's SHA-256 checksum before unpacking it

### Changed
- `install.sh` no longer clones this tool's own repo into `~/.dots` — it only ever downloads (or, as a last resort, source-builds in a scratch dir) the `dots` binary itself. The only git repo an install now needs is the user's own personal dotfiles repo. Upgraders with a pre-existing full-repo clone at `~/.dots` get it cleaned up automatically (tool-repo files only — `settings.toml`/`links.toml`/`plugins/`/`src/` are left untouched)
- Version resolution in `install.sh` now queries the GitHub Releases API instead of `git describe`, and the from-source fallback pulls a tagged source tarball via HTTPS instead of `git clone` — `git` is no longer a dependency of the installer at all
- The TUI now shows a brief boxed banner in the top-right corner when a newer release is found, instead of relying solely on the easily-overwritten one-line flash message
- The Symlinks/Tools/Configs dashboard tiles now read from a cache refreshed on a background timer instead of re-reading the symlink manifest, spawning a `which` per dependency, and rescanning the configs directory on every single frame
- A Lua plugin's `dots.sh(cmd)` call is now killed and returns `""` if it runs past 5 seconds, instead of blocking the TUI thread indefinitely — a slow `gh`/`aws` call used to freeze the whole dashboard

### Fixed
- Applying a premade config a second time (or running `dots premade apply` twice) no longer overwrites the backup of the user's real original with the premade content
- `dots link add` now compares the expanded absolute path when checking for an existing `links.toml` entry, so `~/…` and absolute forms of the same target are recognized as the same link

## [2.3.0] - 2026-07-27

### Added
- Modular dashboard **zones** — the dashboard is now a set of user-defined regions, each holding an ordered list of widgets (built-in tiles or plugin panes). Zones live in `~/.dots/layout.toml`; with no file the default reproduces the previous fixed grid exactly
- `dots layout show | init [--force] | path` — resolve the layout the way the TUI does (plugins included), scaffold a commented `layout.toml`, or print its path
- Plugin API: `ui.zone{…}` declares a zone, and `ui.pane{ zone = "…" }` places a pane in one. A zone the user's `layout.toml` already defines always wins over a plugin's
- `examples/plugins/zones.lua` — a plugin that groups its panes into its own region
- Lua plugin system with TUI panes, CLI, and examples (`ui.pane`, `ui.layout`, `dots.sh/env/dir`)
- Config commands `add`, `sync`, `get-config`, `config install --all`, TUI add/sync, and a unified manifest
- `dots --version` now prints the commit and how the version was resolved; `-V` keeps the short form

### Changed
- **One navigation model across the whole TUI.** Every screen now carries the same nav strip showing its siblings with the current one lit, and you move between them from anywhere: `1`–`6` jump directly, `[` / `]` / `tab` cycle with wrapping, `esc` returns to the dashboard. A single `NAV` table drives the strip, the digits, and the cycling, so they cannot disagree. Screens holding a prompt — search box, path entry, confirmation, settings popup — keep every key, so navigation never fires mid-edit
- **Symlinks and Tools are now separate screens.** Both dashboard panes used to open the same Health screen scrolled to a different section; each now drills into its own screen with its own rows, title, and cursor position. `r` (repair all) is available only on Symlinks and `i` (install all) only on Tools, so neither key can act on rows you aren't looking at
- **TUI restyled** to a minimal, low-contrast terminal theme — thin rounded borders in muted grey (`#3B4252`), block titles inlined into the top border, cyan reserved for the focused block and screen titles, and status carried by `●` green / `◐` `○` amber / `✗` red bullets that read even without color
- Dashboard tiles now list their items — name left-aligned, status metadata right-aligned, problems sorted to the top so they survive truncation — above a dim summary footer (`4 links · 3 ok · 1 missing`)
- Top bar shows a cyan title with a muted subtitle and right-aligned hints; the bottom bar renders keys dimmed against lighter action text
- The palette moved into named roles in `tui-core::theme` (borders, titles, keys, status), so a screen picks a role rather than a color. The original five style functions remain as aliases
- The configs detail column no longer paints over the description bar's row
- Version resolution hardened: `git describe` on an untagged checkout no longer becomes the version (it degraded to a bare commit hash, which compares as older than every release and pinned self-update to a permanent "update available")
- `crate::version` is the single place the binary's own version is read from, replacing `env!("DOTS_VERSION")` scattered across the CLI, settings header, and updater
- Release workflow gained a `verify-version` job that fails a tag disagreeing with `Cargo.toml` before any binary is built
- Workspace version synced to the version under development — it had drifted to `2.0.0` while `v2.2.0` shipped, so any build without git reported the wrong number
- `ui.layout{ columns = N }` now reflows tiles within zones, and is honoured only while you have no `layout.toml` of your own
- Settings rebound to the space key and dropped from the number menu; update folded into the settings popup
- Site: neofetch-style hero with ASCII art, consolidated CSS/JS with terminal components, legacy pages dropped, install URL normalized to `ctrluserknown.github.io`, `base href` fixed for the GitHub Pages subdirectory

### Removed
- rustfmt from the CI workflow

## [2.2.0] - 2026-07-23

### Added
- Static site pages (`site/`) — about, docs, changelog with version-based navigation
- Interactive TUI demo page with live ratatui-style dashboard rendering
- GitHub Pages deploy workflow (`deploy-site.yml`)
- Root file mirroring in deploy (README, CHANGELOG, install.sh, etc.)

### Changed
- README updated with site links, CHANGELOG reference, and clean documentation structure

## [2.1.0] - 2026-07-22

### Added
- Configs module (`configs.rs`) for discovering and managing dotfiles configs from your repo — replaces the old premade-only system with a full config browser that shows install status per app
- TUI Configs screen (`tui/configs.rs`) with config listing, file preview, and install/remove actions
- GitHub Releases-based binary self-update with SHA-256 verification
- Install-source detection (Homebrew vs self-managed) — defers to package managers when appropriate

### Changed
- Self-update now downloads release tarballs from GitHub Releases instead of `git pull --ff-only`
- Simplified installer: removed `--branch` flag, always clones default branch
- Install script `build_binary` paths updated for root workspace layout (no more `dots-rs/` subdirectory)

### Removed
- Developer mode setting
- Legacy `dots.bak/` backup directory

## [2.0.0] - 2026-07-21

### Added
- Complete rewrite in Rust — single static binary with no runtime dependencies
- Interactive TUI built on ratatui with 6-pane dashboard (symlinks, tools, plugins, configs, updates, network)
- Live network monitor: connectivity, latency, network name, DNS servers, VPN detection (macOS + Linux)
- Symlink engine: GNU Stow–style link management via `links.toml` with explicit links and stow-style folding
- Cross-platform dependency management: Homebrew, apt, and dnf with per-platform package name resolution
- Three dependency tiers: Required (7), Optional (6), Dev (12)
- Premade app configs (Ghostty, Neovim, opencode) compiled into the binary
- Portable profiles: export/import `personal.json` locally or from GitHub
- Alias management: built-in + user aliases with TUI and CLI support
- Ghostty theme picker from installed themes
- Self-updating via `git pull --ff-only` with automatic symlink re-repair
- Mouse support in TUI (scroll wheel, left-click on dashboard panes)
- Keyboard shortcuts: number keys 1-4 for quick screen access
- Settings via `~/.dots/settings.toml` with `~/.personal/config.toml` overlay
- Shell installer script (POSIX sh) with prebuilt binary download fallback
- Man page
- Shared `tui-core` crate for reusable TUI chrome
- Comprehensive test suite: unit tests, integration tests, shell integration test
- CI on Linux and macOS via GitHub Actions

### Changed
- Project structure: Cargo workspace with `crates/dots` (binary) and `crates/tui-core` (library)
- All configuration under `~/.dots/` and `~/.personal/`
- Version detection: `DOTS_VERSION` env var > `git describe --tags` > `CARGO_PKG_VERSION`

### Removed
- Python-based `dots.py` TUI and CLI
- SSM (SSH session manager) — moved to separate project
- Shell-specific setup scripts (`setup.sh`)
- Brewfile-based dependency management
- Zsh-specific config files (`.zshrc`, `rc.zsh`, `.aliases`, `.functions`)
- Custom Zsh prompt (`charModel`)
- tmux configuration and plugins
- `noir-cat` and `knew-pines` custom Ghostty themes from repo
- Per-package install timeouts (replaced by Cargo build optimization)

## [1.5.5] - 2026-07-05

### Added
- Dev-mode support for update checking: `check_upstream` accepts `dev_mode` param; fetches without `--tags` in dev mode, returns short SHA instead of release tag
- Python syntax check and test-python job in CI workflow
- Settings key validation in `dots.py --set` — unknown keys are now rejected with a friendly error

### Changed
- `dots.py` / `ssm.py`: update views show commit info instead of version when in dev mode
- `update-check.zsh`: split dev/normal update paths; skip version stamp and upgrade notices in dev mode; dev mode prompt shows commit count and SHA
- Bumped all `# dotfiles v` version headers to v1.5.5

## [1.5.0] - 2026-06-28

### Added
- **ssm.py**: Update check view in the SSH session manager TUI — bound to `u` key, fetches upstream commits and offers to pull
- **ssm.py**: Autoreload sessions when `sessions.json` changes externally

### Changed
- **ssm.py**: Refactored TUI layout with `draw_header`, `draw_footer`, `draw_desc` helper functions
- **ssm.py**: Consolidated color definitions (removed `COLOR_SUCCESS`, `COLOR_ACCENT`)

## [1.4.1] - 2026-06-28

### Fixed
- **charModel prompt**: SSH indicator (`∧`) and machine symbol (`⋧`/`⟚`) are now mutually exclusive — SSH sessions show only `∧`, local herdr machines show only `⟚`, and other local machines show only `⋧`.
- **.zshrc**: Added `config:herdr` block that auto-launches herdr via `exec` on SSH start, with `HERDR_AUTOSTART=1` guard to prevent re-entry in child shells. Falls back gracefully if herdr is not installed.
- Bumped all `# dotfiles v` version headers to v1.4.1.

## [1.4.0] - 2026-06-28

### Added
- `dots.py` — a curses-based TUI for managing dotfiles: health checks, theme picker (200+ Ghostty themes), settings, git operations, add alias, view logs, and reset. Also includes CLI flags (`--repair-symlinks`, `--install-deps`, `--health`, `--theme`, etc.) for headless use.
- `dots` shell function — entry point to the `dots.py` TUI; also accepts `dots -v` for version info.
- `.developer` marker file — create/delete `~/.dots/.developer` to toggle developer mode (no more `DEVELOPER_MODE` env var or `$USER` check).
- Interactive package categories during setup (Ghostty, Neovim config, optional tools, personal packages) with Gum-powered prompts when available.
- `dots.py --install-optional` / `--install-personal` for installing optional and personal package tiers.
- Settings persistence (`~/.dots/.settings` JSON file) — update check and greeting preferences.
- `herdr` integration — replaces `tmux` as the default terminal multiplexer.

### Changed
- `setup.sh`: replaced Brewfile-based install with `dots.py` dependency manager; replaced manual symlink commands with `dots.py --repair-symlinks`; added Ghostty install step.
- `.zshrc`: `DOTFILES_VERSION` now derived from latest git tag (`git describe --tags`) instead of manual bumps; developer mode detected via `.developer` file; removed `brew-sync.zsh` hook; greeting now hints at `dots` command.
- `.aliases`: `mux` and `attach` aliases now point to `herdr` instead of `tmux`; added inline comments documenting every alias.
- `.functions`: `commands custom` parses aliases live from `.aliases` file (no drift); removed `config()` function (replaced by `dots` → Edit Configs); removed `attach()` (use `herdr` directly); refactored `create()` with `_create_project` helper.
- `update-check.zsh`: reduced poll interval from 24h to 10 minutes; derive version from git tags instead of parsing `.zshrc`; replaced manual `ln -sf` calls with `dots.py --repair-symlinks`; fetches with `--tags`.
- `charModel` prompt: SSH indicator changed from `(ssh)` to a compact `∧` symbol.
- `.gitignore`: added `.developer`, `.settings`, `.update_stamp`, `.version_stamp`.

### Removed
- `assets/Brewfile` — 215-line Homebrew manifest replaced by declarative `DEPS` list in `dots.py` (supports brew, dnf, and apt).
- `brew-sync.zsh` — no longer needed without a Brewfile to sync to.
- `config()` function — replaced by `dots.py`'s Edit Configs menu.
- `attach()` function — use `herdr` directly.

## [1.3.1] - 2026-06-27

### Added
- SSH connection awareness in the `charModel` Zsh prompt: shows a pink `(ssh)` indicator when connected over SSH

### Changed
- Updated README to document the new SSH indicator feature

## [1.3.0] - 2026-06-25

### Added
- `DOTFILES_VERSION` variable in `.zshrc` for explicit version tracking across config files
- Version-change prompt in `update-check.zsh`: on the first shell open after an update, shows "✨ Dotfiles updated: vX.X.X → vY.Y.Y" with a pointer to `config` and the CHANGELOG
- Upstream version preview in the update prompt — shows the new version number before the user accepts a pull
- `~/.config/zsh/.version_stamp` file to persist the last-seen version between shell sessions

### Changed
- Version comments bumped to `v1.3.0` in `.zshrc`, `update-check.zsh`, `.aliases`, and `.functions`
- `update-check.zsh` writes the new version stamp immediately after a successful pull so the upgrade notice fires on the next shell open

## [1.2.1] - 2026-06-17

### Added
- Neovim prompt: user is asked during setup if they want Neovim and/or Neovim config, with conditional install and linking
- Version markers (`dotfiles v1.2.1`) added to all key config files for release tracking
- Cross-platform `run_timeout` wrapper so `setup.sh` works on macOS runners without GNU `timeout`

### Changed
- `brew-sync.zsh` now respects neovim opt-out: if neovim was removed from the Brewfile, the sync hook won't re-add it

### Fixed
- GitHub Actions `macos-latest` runner no longer fails with `timeout: command not found` — fallback to direct execution

## [1.2.0] - 2026-06-17

### Added
- Automatic daily dotfiles update check on shell start — prompts to pull new changes
- Developer mode: auto-enables for repo author, skips update prompts; users can toggle via `DEVELOPER_MODE` flag
- `brew-sync.zsh`: auto-regenerates `assets/Brewfile` on `brew install`/`uninstall`/`tap`/`untap` (developer mode only)
- Individual per-package timeout for Brewfile installations (120s each) so one hanging dep won't block others
- GitHub dependency installers: `fzf-tab`, Tmux Plugin Manager (`tpm`), and opencode npm deps

### Changed
- Migrated update check logic from inline in `.zshrc` to standalone `update-check.zsh`
- Fixed CI zsh syntax check to actually cover `.zshrc`, `.aliases`, `.functions`, and theme files

### Removed
- Unused zsh themes: `charMulti` and `charMux`; only `charModel` remains

### Fixed
- Removed orphaned gitlink at `src/tmux/tpm/plugins/tpm` that caused CI to fail with `fatal: No url found for submodule path` — the gitlink survived a 2025 restructure cleanup and was never deleted from the index
- Stripped all remaining tmux/tpm references from `setup.sh`, `tests/test_setup.sh`, and `.github/workflows/main.yml`

## [1.1.0] - 2026-03-29

### Added
- Integrated `fzf-tab` for enhanced Zsh completions
- Added `mux-session` function for smoother Tmux session management
- Added MIT license to the repository

### Changed
- Refactored Zsh aliases and command functions for improved UX
- Reworked Tmux session management and `attach` function workflow
- Updated README with minor fixes and documentation improvements

### Fixed
- Resolved ASCII display issues in `fzf`
- Fixed conflict between `zoxide` and Zsh configurations

## [1.0.0] - 2026-03-07

This is the first stable release of the Ctrlk Dotfiles project, providing a robust and automated environment setup for macOS.

### Added
- Automated testing suite for `setup.sh` in the `tests/` directory
- Timeout logic for `git clone` and `brew bundle` in `setup.sh` to prevent hangs
- Automatic shallow clone fallback for dotfiles if a full clone times out

### Changed
- Replaced custom spinner with direct output for network-dependent tasks to improve reliability
- Improved `install_gum` function with timeouts for better error handling

## [0.3.0] - 2026-03-06

### Added
- Zsh functions for enhanced terminal productivity
- GitHub Actions workflow for automated install script testing and ShellCheck linting
- Configuration for `bat` (cat clone with wings)
- Configuration for macOS terminal
- macOS version check in `setup.sh` to ensure system compatibility
- System Recommendations and Installation guide in `README.md`

### Changed
- Major restructuring: moved configuration files into the `src/` directory for better organization
- Updated Tmux keybindings for improved workflow
- Improved `fzf` configuration within `.zshrc`
- Refined Neovim statusline and cmdline (prior to migration)
- Replaced Zsh-specific globbing with `find` in `.zshrc` for better ShellCheck compatibility
- Removed emojis from `README.md` for a cleaner look

### Fixed
- Resolved various bugs in Tmux, Neovim, and Zsh
- Fixed Ghostty padding and theme consistency issues
- Corrected Zsh alias issues and `ls` command bugs
- Fixed multiple syntax errors and logic issues in `setup.sh`
- Fixed ShellCheck linting errors in `.zshrc` and `setup.sh`

### Removed
- Neovim configuration (migrated to standalone repository: `CtrlUserKnown/Charvim`)
- `zimfw` to resolve completion errors, in favor of framework-integrated completions
- Tmux plugins from git tracking (now ignored via `.gitignore`)

## [0.2.0] - 2025-10-03

### Added
- Oh-my-zsh added for plugin support in zsh
- "Winnie" oh-my-zsh theme added
- Neovim config added with "pckr.vim" as the plugin manager
- Fastfetch added to replace Neofetch
- Added hyprland config
- fzf config within ".zshrc"
- Rose pine theme in Zsh, Neovim, Fastfetch, and Tmux
- Wezterm w/ config
- Improvements to "autoclose.lua" for Visual mode in Neovim

### Changed
- Restructured file structure for better management during development
- Rose pines themes for consistency in the terminal
- Renamed some zsh files
- Replaced "nvim-surround.vim" with updates to "autoclose.lua"

### Removed
- "nvim-surround.vim" plugin

## [0.1.7] - 2025-08-03

### Added
- Ghostty w/ config
- Rectangle WM w/ config 
- "Lazy.vim" plugin manager added
- Brewfile to track dependencies

### Removed
- "Pckr.vim" plugin manager due to have issues maintianing

### Changed
- Moved to "Lazy.vim" for better plugin support
- Rewrote Ghostty config, added better theme

## [0.1.5] - 2025-06-14

### Added
- Tmux w/ config 
- Neovim plugin "autoclose.vim" added for better usability
- New zsh themes added: "CharlynderModel" & "CharlynderLite"

### Removed
- Old "Winnie" zsh theme
- Old Neovim config

### Changed
- Changed Tmux keybindings: prefix changed from `<Ctrl + b>` to `<Ctrl + Space>`
- Replaced "Winnie" zsh theme with "CharlynderModel" theme as the default
- Made better Neovim plugin & UI changes
- Added "Tree-sitter.vim", "oil.vim", & "completions.vim" to Neovim for better usability
