# CrtlUserKnown Dotfiles

## System Recommendations

For the best experience and full compatibility with all tools (especially Ghostty and Homebrew), the following is recommended:

- Operating System: macOS 12.0 (Monterey) or newer.
- Architecture: Apple Silicon (M1/M2/M3/M4) is preferred, though Intel Macs are supported.
- Default Shell: zsh (standard on macOS 10.15 Catalina and later).
- Terminal: [Ghostty](https://ghostty.org/) (Config included, requires macOS 12.0+).

### Installation

To set up your environment, clone this repository and run the setup script:

```bash
git clone https://github.com/CtrlUserKnown/dotfiles.git ~/.dots
cd ~/.dots
./setup.sh
```

### Themes

Char Model: [Char Model zsh prompt theme](/src/zsh/zsh/themes/charModel)

Char Multi : [Char Multi zsh prompt theme](/src/zsh/zsh/themes/charMulti)

### Configs

Dotfiles comes with configurations for zsh, PowerShell, TMUX, Ghostty, and more! Great for any new user!

ZSH configuration for `Mac` & `Linux`: [ZSH](/zsh/.zshrc)

Use Starship with PowerShell on `Windows`: [PowerShell](/assets/otherOS/powershell/Microsoft.PowerShell_profile.ps1)

Tmux configuration: [TMUX](/src/tmux/tmux.conf)

Ghostty configuration: [Ghostty](/src/ghostty/config)

Neovim config (moved to New repo): [Charvim](https://github.com/CrtlUserKnown/Charvim)
