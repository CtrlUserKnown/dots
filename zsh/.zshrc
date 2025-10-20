# Char zshrc file
#
# date created: 10.14.2025

# --- config:prompts ---
# remove comment to pick a theme
# default prompt theme CharModel
source ~/.config/zsh/themes/charModel
# source ~/.config/zsh/themes/charMulti

# --- config:bat ---
# Bat color themes
export BAT_THEME="rose-pine"

# --- config:eza ---
eval $(dircolors ~/.config/dircolors)

# --- config:yazi ---
export EDITOR="nvim"
export VISUAL="nvim"

# --- config:fzf ---
# default command for fzf (what it searches)
export FZF_DEFAULT_COMMAND='fd --type f --hidden --follow --exclude .git'

# Default options for fzf
export FZF_DEFAULT_OPTS='
  --height 40%
  --layout=reverse
  --border
  --preview "bat --style=numbers --color=always {}"
  --bind "alt-enter:execute(nvim {})"
  --color=fg:#e0def4,bg:#191724,hl:#eb6f92
  --color=fg+:#e0def4,bg+:#26233a,hl+:#eb6f92
  --color=info:#9ccfd8,prompt:#f6c177,pointer:#c4a7e7
  --color=marker:#ebbcba,spinner:#eb6f92,header:#31748f
  --color=border:#524f67,preview-bg:#1f1d2e
'

# CTRL-T options (file search)
export FZF_CTRL_T_COMMAND="$FZF_DEFAULT_COMMAND"
export FZF_CTRL_T_OPTS='--preview "bat --color=always --line-range :500 {}"'

# ALT-C options (directory search)
export FZF_ALT_C_COMMAND='fd --type d --hidden --follow --exclude .git'
export FZF_ALT_C_OPTS='--preview "tree -C {} | head -200"'

# CTRL-R options (command history)
export FZF_CTRL_R_OPTS='--preview "echo {}" --preview-window down:3:wrap'

# --- conifg:aliases ---
# import aliases
if [ -f ~/.config/zsh/aliases.zsh ]; then
        source ~/.config/zsh/aliases.zsh
fi

# --- conifg:fastfetch ---
# Run neofetch only once per session
if [[ ! -f /tmp/neofetch_run_$$ ]]; then
    fastfetch
    touch /tmp/neofetch_run_$$
fi

# --- conifg:zoxide ---
# initialize zoxide (provides `z` and `zi`, plus completions)
eval "$(zoxide init zsh)"

# --- config:zim ---
ZIM_HOME=${ZDOTDIR:-${HOME}}/.zim
# download zimfw plugin manager if missing.
if [[ ! -e ${ZIM_HOME}/zimfw.zsh ]]; then
  if (( ${+commands[curl]} )); then
    curl -fsSL --create-dirs -o ${ZIM_HOME}/zimfw.zsh \
        https://github.com/zimfw/zimfw/releases/latest/download/zimfw.zsh
  else
    mkdir -p ${ZIM_HOME} && wget -nv -O ${ZIM_HOME}/zimfw.zsh \
        https://github.com/zimfw/zimfw/releases/latest/download/zimfw.zsh
  fi
fi

# install missing modules, and update ${ZIM_HOME}/init.zsh if missing or outdated.
if [[ ! ${ZIM_HOME}/init.zsh -nt ${ZIM_CONFIG_FILE:-${ZDOTDIR:-${HOME}}/.zimrc} ]]; then
  source ${ZIM_HOME}/zimfw.zsh init
fi

# initialize modules.
source ${ZIM_HOME}/init.zsh

# remove older command from the history if a duplicate is to be added.
setopt HIST_IGNORE_ALL_DUPS

# remove path separator from WORDCHARS.
WORDCHARS=${WORDCHARS//[\/]}

# --- config:zsh ---
# zsh-autosuggestions
ZSH_AUTOSUGGEST_MANUAL_REBIND=1

# customize the highlighting style that the suggestions are shown with.
ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE='fg=242'

# zsh-history-substring-search
zmodload -F zsh/terminfo +p:terminfo

# zsh-syntax-highlighting
ZSH_HIGHLIGHT_HIGHLIGHTERS=(main brackets)

# customize the main highlighter styles.
typeset -A ZSH_HIGHLIGHT_STYLES
ZSH_HIGHLIGHT_STYLES[comment]='fg=242'

# --- config:hidden ---
# use degit instead of git as the default tool to install and update modules.
# zstyle ':zim:zmodule' use 'degit'
