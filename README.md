# Ctrlk Dotfiles

A professional, performance-oriented macOS development environment. This repository automates the setup of a modern terminal workflow using Homebrew, Zsh, Tmux, and Ghostty.

> [!TIP]
> [Test Setup Script](https://github.com/CtrlUserKnown/dotfiles/actions/workflows/main.yml/badge.svg)

## Features

- **Automated Setup:** A robust `setup.sh` script that handles Homebrew, dependencies, and symlinking.
- **Resilient Installation:** Built-in timeout logic and shallow clone fallbacks to prevent hanging on slow connections.
- **Modern Stack:** Optimized configurations for:
  - **Terminal:** [Ghostty](https://ghostty.org/) (macOS 12.0+)
  - **Shell:** Zsh with custom themes (`charModel`, `charMulti`)
  - **Multiplexer:** Tmux with TPM (Tmux Plugin Manager)
  - **Utilities:** `bat`, `fastfetch`, `fzf`, `git`
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

The script will:
1. Check your macOS version.
2. Install [Homebrew](https://brew.sh/) and [Gum](https://github.com/charmbracelet/gum) if missing.
3. Install all dependencies from the `Brewfile`.
4. Create symlinks for all configurations in `~/.config`.

## Project Structure

- [`src/zsh/`](src/zsh/) - Zsh configuration and custom themes.
- [`src/tmux/`](src/tmux/) - Tmux configuration and plugin management.
- [`src/ghostty/`](src/ghostty/) - Configuration for the Ghostty terminal.
- [`src/bat/`](src/bat/) - Themes and config for the `bat` utility.
- [`src/fastfetch/`](src/fastfetch/) - System information display config.
- [`assets/Brewfile`](assets/Brewfile) - Managed list of Homebrew packages.

## Testing

This project includes a safe, isolated test suite to verify the installation process without affecting your actual home directory.

```bash
cd tests
./test_setup.sh
```

## Themes

- **Char Model:** A clean, minimal Zsh prompt. [View Config](src/zsh/zsh/themes/charModel)
- **Char Multi:** A feature-rich, multi-line Zsh prompt. [View Config](src/zsh/zsh/themes/charMulti)
- **Rose Pine:** Consistent color schemes across all tools.

---
*Neovim configuration has been migrated to its own repository: [Charvim](https://github.com/CtrlUserKnown/Charvim)*
