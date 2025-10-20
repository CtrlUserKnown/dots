# Charlynder zshrc file
#
# date created: 07.18.2022
#
# v0.1.2: 09.08.2025

# Path to your oh-my-zsh installation.
export ZSH="$HOME/.oh-my-zsh"

ZSH_THEME="charlynderModel"

# auto-update oh-my-zsh
# zstyle ':omz:update' mode auto      # update automatically without asking

# Plugins
plugins=(git zsh-autosuggestions)

source $ZSH/oh-my-zsh.sh

# Load zsh-autosuggestions
# source $(brew --prefix)/share/zsh-autosuggestions/zsh-autosuggestions.zsh

# User configuration

# Run neofetch only once per day
if [[ ! -f /tmp/neofetch_run_$(date +%Y%m%d) ]]; then
    fastfetch
    touch /tmp/neofetch_run_$(date +%Y%m%d)
fi

# Language environment
export LANG=en_US.UTF-8

# Preferred editor for local and remote sessions
if [[ -n $SSH_CONNECTION ]]; then
  export EDITOR='vim'
else
  export EDITOR='mvim'
fi

# Set fzf key bindings and fuzzy completion (disabled)
# source <(fzf --zsh)


# import aliases
if [ -f ~/.aliases ]; then
	source ~/.aliases
fi

# zoxide (keep this at the very end so hooks and completions are not overridden)
# Ensure the completion system is initialized so compdef is available
if ! typeset -f compdef >/dev/null; then
  autoload -Uz compinit && compinit -i
fi
# Initialize zoxide (provides `z` and `zi`, plus completions)
eval "$(zoxide init zsh)"
