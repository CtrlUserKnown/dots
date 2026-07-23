<p align="center">
  <img src="site/img/svg/dots_logo_color_title.svg" alt="dots" width="180" />
</p>

**dots** is a fast, cross-platform dotfiles manager with an interactive TUI, written in Rust. It installs your tools, wires up symlinks GNU Stow–style, applies premade app configs, and keeps everything healthy — on macOS (Homebrew) and Linux (apt/dnf) alike.

> This repository is the **`dots` tool** itself. Your actual dotfiles/configs live in a separate repo (e.g. [`dotfiles-CUK`](https://github.com/CtrlUserKnown/dotfiles)); `dots` manages the symlinks between them and your `$HOME`.

## Features

- **Interactive TUI** — run `dots` for a dashboard covering symlink health, installed tools, shell plugins, app configs, and updates.
- **Cross-platform installs** — one dependency list, resolved per platform via Homebrew, `apt`, or `dnf`.
- **Symlink management** — declare your own links (`dots link add`), then create/repair them idempotently. Adopts existing files with automatic backups.
- **App configs** — every directory in your dotfiles repo (`nvim/`, `git/`, `bat/`, …) shows up in the TUI's **Configs** screen with an install-status badge (`[ installed ]` / `[ partial ]` / `[ not installed ]`); view a config's files, preview their contents, and install/remove it by (un)linking into `$HOME`.
- **Premade configs** — bundled starter configs for Ghostty, Neovim, and opencode, applied on demand (existing files are backed up).
- **Portable profiles** — export your setup to `personal.json` and re-import it on another machine, locally or straight from GitHub.
- **Self-updating** — built-in update checker and one-command upgrade.
- **Single static binary** — no runtime dependencies (pure-Rust TLS, no OpenSSL/keychain), optimized for size.

Release-by-release history lives in [`CHANGELOG.md`](CHANGELOG.md).

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/CtrlUserKnown/dots/main/install.sh | sh
```

The installer clones the repo to `~/.dots`, downloads a prebuilt binary for your OS/arch (or builds from source with `cargo` if no release matches), puts `dots` on your `PATH`, and initializes config.

```
install.sh [--version <tag>] [--dir <path>]
```

The same script is mirrored on the site, if you prefer the shorter URL:

```sh
curl -fsSL https://ctrluserknown.github.io/dots/install.sh | sh
```

Prefer to build it yourself? See [`BUILD_MACOS.md`](BUILD_MACOS.md), or from a clone:

```sh
cargo build --release        # binary at target/release/dots
```

To remove it: [`uninstall.sh`](uninstall.sh).

## Usage

Run `dots` with no arguments to open the TUI. The dashboard's panes — **Symlinks**, **Tools**, **Plugins**, **Configs**, **Update** — drill into full-screen views for health checks, aliases, profile, theme, and settings.

Everything is also scriptable via subcommands:

| Command | What it does |
|---|---|
| `dots health [--fix]` | Check and repair all declared symlinks, tools, and plugins |
| `dots update` | Check for and apply updates |
| `dots install <name>` | Install a single dependency |
| `dots install --all` | Install all missing **required** dependencies |
| `dots install --optional` | Install all missing **optional** dependencies |
| `dots aliases list \| add <name> <value> \| remove <name>` | Manage shell aliases |
| `dots premade list \| apply \| remove <app>` | List/apply/remove bundled starter configs (ghostty, neovim, opencode) |
| `dots config list \| view \| install \| remove <name>` | View and (un)install app configs discovered in your dotfiles repo |
| `dots link add <source> <target>` | Adopt a file/dir and symlink it (recorded in `links.toml`) |
| `dots link list \| apply \| remove <target>` | Inspect, create/repair, or remove declared links |
| `dots profile generate [path]` | Export your setup to `personal.json` |
| `dots profile import <path>` | Import a `personal.json` from a local file |
| `dots profile import-git <user/repo/path.json>` | Import a `personal.json` from GitHub |
| `dots init [--quiet]` | Initialize config (idempotent; run automatically by the installer) |
| `dots --version` | Print the version |

### Dependencies

The dependency list is defined in [`crates/dots/src/packages.rs`](crates/dots/src/packages.rs) in three tiers, each mapped to its `brew` / `dnf` / `apt` package name:

| Category | When installed | Examples |
|---|---|---|
| **Required** | `dots install --all` | git, eza, bat, fd, fzf, fastfetch, zoxide |
| **Optional** | `dots install --optional` | neovim, herdr, btop, lazygit, yazi, carapace |
| **Dev** | `dots install <name>` | go, lua, cmake, gcc, ripgrep, gh, docker, ffmpeg, … |

## Configuration

- `~/.dots/` — the tool's home (repo checkout + `bin/dots`).
- `~/.dots/settings.toml` — tool settings, under a `[dots]` table:

  | Key | Default | Meaning |
  |---|---|---|
  | `update_check` | `true` | Check for updates periodically |
  | `update_frequency` | `1440` | Minutes between update checks |
  | `greeting` | `true` | Show the greeting banner |
  | `developer_mode` | `false` | Enable developer features |
  | `theme` | *(empty)* | Selected theme |

- `~/.personal/` — your personal, machine-local layer: `aliases.zsh` (sourced after the built-in aliases), `apps/`, and an optional `config.toml` that overrides `settings.toml`.

## Project Structure

A Cargo workspace with two crates:

- [`crates/dots/`](crates/dots/) — the `dots` binary: CLI, TUI screens, installer, symlink/link engine, config, and bundled premade `assets/`.
- [`crates/tui-core/`](crates/tui-core/) — shared TUI chrome (header/footer/description bars, color theme, flash model) used by the `dots` screens.

## Development

```sh
cargo test --all                     # unit + integration tests
cargo clippy --all -- -D warnings    # lints
cargo fmt --all -- --check           # formatting

bash tests/integration/test_setup.sh # shell integration test (matches CI)
```

An isolated container test environment lives in [`test-env/`](test-env/) (see [`test-env/manage.sh`](test-env/manage.sh)); the manual QA checklist is in [`docs/manual-test-checklist.md`](docs/manual-test-checklist.md). CI runs on Linux and macOS via [`.github/workflows/ci.yml`](.github/workflows/ci.yml).

## License

See [`LICENSE`](LICENSE).
