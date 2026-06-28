# CtrlUserKnown Dots

A professional, performance-oriented macOS development environment. This repository automates the setup of a modern terminal workflow using Homebrew, Zsh, and Ghostty.

> [!TIP]
> [Test Setup Script](https://github.com/CtrlUserKnown/dotfiles/actions/workflows/main.yml/badge.svg)

## Features

- **Automated Setup:** A robust `setup.sh` script that handles Homebrew, dependencies, and symlinking.
- **Resilient Installation:** Built-in timeout logic and shallow clone fallbacks to prevent hanging on slow connections.
- **Modern Stack:** Optimized configurations for:
  - **Terminal:** [Ghostty](https://ghostty.org/) (macOS 12.0+) with multiple themes and built-in theme picker
  - **Shell:** Zsh with custom `charModel` prompt theme
  - **Utilities:** `eza`, `bat`, `fastfetch`, `fzf`, `zoxide`
- **`dots` Command:** Interactive TUI for checking health, picking themes, managing settings, editing configs, and more.
- **Auto-Update:** Built-in update checker (polls every 10 minutes) to stay current with the latest dotfiles.
- **Quality Assured:** Includes a dedicated automated test suite and GitHub Actions CI.

## System Requirements

- **OS:** macOS 12.0 (Monterey) or newer (optimized for modern macOS).
- **Arch:** Apple Silicon (M1/M2/M3/M4) preferred; Intel supported.
- **Shell:** Zsh (standard on macOS 10.15+).

## Quick Start

You can install these dotfiles with a single command:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/CrtlUserKnown/dotfiles/refs/heads/main/setup.sh)"
```

Or clone the repository manually:

```bash
git clone https://github.com/CtrlUserKnown/dotfiles ~/.dots && ~/.dots/setup.sh
```

The script will:
1. Check your macOS version.
2. Install [Homebrew](https://brew.sh/) and [Gum](https://github.com/charmbracelet/gum) if missing (includes timeout/fallback logic).
3. Prompt you to install Ghostty, Neovim, optional tools, and personal packages.
4. Install required dependencies (eza, bat, fzf, fastfetch, zoxide, neovim).
5. Create symlinks for all configurations via `dots.py --repair-symlinks`.

## Project Structure

- [`src/zsh/`](src/zsh/) — Zsh configuration, custom `charModel` theme, aliases, functions, and utilities (`dots.py`, `update-check.zsh`).
- [`src/ghostty/`](src/ghostty/) — Configuration and themes for the Ghostty terminal.
- [`src/bat/`](src/bat/) — Themes and config for the `bat` utility.
- [`src/fastfetch/`](src/fastfetch/) — System information display config.
- [`src/opencode/`](src/opencode/) — Configuration for [opencode](https://opencode.ai) AI coding assistant.
- [`src/git/`](src/git/) — Git configuration.
- [`src/zsh/zsh/dots.py`](src/zsh/zsh/dots.py) — Dotfiles manager TUI and CLI (symlinks, deps, themes, settings, git).

## Usage

Run `dots` in your terminal to open the interactive TUI:

```
dots          # open the dots manager menu
dots -v       # show version
dots --health # check installed tools from the command line
```

### Package Categories

Dependencies are declared in `dots.py` in three tiers:

| Category   | Included | Examples |
|------------|----------|---------|
| **Required** | Always installed | eza, bat, fzf, fastfetch, zoxide, neovim |
| **Optional** | Prompted during setup | herdr, btop, lazygit, yazi, carapace |
| **Personal** | Prompted during setup | go, rust, docker, ffmpeg, rectangle, maccy, blender |

## Testing

This project includes a safe, isolated test suite to verify the installation process without affecting your actual home directory.

```bash
cd tests
./test_setup.sh
```

## Themes

- **Char Model:** A clean, minimal Zsh prompt with SSH connection awareness (shows `∧` in remote sessions). [View Config](src/zsh/zsh/themes/charModel)
- **Ghostty Themes:** Use `dots` → Theme to pick from 200+ built-in Ghostty themes; [noir-cat](src/ghostty/themes/noir-cat) and [knew-pines](src/ghostty/themes/knew-pines) included.
- **KnewPines:** KnewPines, KnewPines Moon, and KnewPines Dawn color schemes for `bat`. See [`src/bat/themes/`](src/bat/themes/).

---
*Neovim configuration has been migrated to its own repository: [Charvim](https://github.com/CtrlUserKnown/Charvim)*
