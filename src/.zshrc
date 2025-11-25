# Char zshrc file
#
# date created: 10.14.2025
# Modified to work without Zim Framework

# --- config:homebrew ---
# Initialize Homebrew
if [[ -f "/opt/homebrew/bin/brew" ]];
then
  eval "$(/opt/homebrew/bin/brew shellenv)"
elif [[ -f "/usr/local/bin/brew" ]]; then
  eval "$(/usr/local/bin/brew shellenv)"
fi

# --- config:bat ---
# Bat color themes
export BAT_THEME="rose-pine"

# --- config:editor ---
export EDITOR="nvim"
export VISUAL="nvim"

# --- config:ls/eza ---
# Rose Pine colors for eza
export EZA_COLORS="\
da=38;5;246:\
di=38;2;196;167;231:\
ln=38;5;211:\
ex=38;2;86;148;159:\
*.txt=38;5;224:\
*.md=38;5;224:\
*.json=38;5;180:\
*.yml=38;5;180:\
*.yaml=38;5;180"

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
export FZF_ALT_C_COMMAND='fd 
--type d --hidden --follow --exclude .git'
export FZF_ALT_C_OPTS='--preview "tree -C {} |
head -200"'

# CTRL-R options (command history)
export FZF_CTRL_R_OPTS='--preview "echo {}" --preview-window down:3:wrap'

# --- config:history ---
HISTFILE=${ZDOTDIR:-$HOME}/.zsh_history
HISTSIZE=10000
SAVEHIST=10000
setopt HIST_IGNORE_ALL_DUPS
setopt SHARE_HISTORY
setopt APPEND_HISTORY
setopt INC_APPEND_HISTORY

# --- config:options ---
# Remove path separator from WORDCHARS
WORDCHARS=${WORDCHARS//[\/]}

# Enable extended globbing
setopt EXTENDED_GLOB

# Auto CD when typing directory name
setopt AUTO_CD

# --- config:completions ---
# Set up fpath for completions (add zsh-completions if installed via Homebrew)
if [[ -d "/opt/homebrew/share/zsh-completions" ]]; then
  fpath=(/opt/homebrew/share/zsh-completions $fpath)
elif [[ -d "/usr/local/share/zsh-completions" ]]; then
  fpath=(/usr/local/share/zsh-completions $fpath)
fi

# Initialize completion system
autoload -Uz compinit
if [[ -n ${ZDOTDIR}/.zcompdump(#qN.mh+24) ]]; then
  compinit
else
  compinit -C
fi

# Completion styling
zstyle ':completion:*' menu select
zstyle ':completion:*' matcher-list 'm:{a-zA-Z}={A-Za-z}' 'r:|[._-]=* r:|=*' 'l:|=* r:|=*'
zstyle ':completion:*' list-colors "${(s.:.)LS_COLORS}"
zstyle ':completion:*' completer _complete _match _approximate
zstyle ':completion:*:match:*' original only
zstyle ':completion:*:approximate:*' max-errors 1 numeric
zstyle ':completion:*' group-name ''
zstyle ':completion:*:descriptions' format '%F{yellow}-- %d --%f'
zstyle ':completion:*:messages' format '%F{purple}-- %d --%f'
zstyle ':completion:*:warnings' format '%F{red}-- no matches found --%f'

# --- config:zsh-syntax-highlighting ---
# Load zsh-syntax-highlighting (install via: brew install zsh-syntax-highlighting)
if [[ -f "/opt/homebrew/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh" ]];
then
  source /opt/homebrew/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh
elif [[ -f "/usr/local/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh" ]]; then
  source /usr/local/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh
fi

# Syntax highlighting customization
ZSH_HIGHLIGHT_HIGHLIGHTERS=(main brackets)
typeset -A ZSH_HIGHLIGHT_STYLES
ZSH_HIGHLIGHT_STYLES[comment]='fg=242'

# --- config:zsh-autosuggestions ---
# Load zsh-autosuggestions (install via: brew install zsh-autosuggestions)
if [[ -f "/opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh" ]];
then
  source /opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh
elif [[ -f "/usr/local/share/zsh-autosuggestions/zsh-autosuggestions.zsh" ]]; then
  source /usr/local/share/zsh-autosuggestions/zsh-autosuggestions.zsh
fi

ZSH_AUTOSUGGEST_MANUAL_REBIND=1
ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE='fg=242'

# --- config:zsh-history-substring-search ---
# Load zsh-history-substring-search (install via: brew install zsh-history-substring-search)
if [[ -f "/opt/homebrew/share/zsh-history-substring-search/zsh-history-substring-search.zsh" ]];
then
  source /opt/homebrew/share/zsh-history-substring-search/zsh-history-substring-search.zsh
elif [[ -f "/usr/local/share/zsh-history-substring-search/zsh-history-substring-search.zsh" ]]; then
  source /usr/local/share/zsh-history-substring-search/zsh-history-substring-search.zsh
fi

# Bind keys for history substring search
if (( ${+functions[_zsh_highlight_bind_widgets]} ));
then
  bindkey '^[[A' history-substring-search-up
  bindkey '^[[B' history-substring-search-down
fi

# --- config:aliases ---
# import aliases
if [ -f ~/.config/zsh/aliases.zsh ];
then
  source ~/.config/zsh/aliases.zsh
fi

# --- config:hooks ---
# Load add-zsh-hook utility (needed by themes)
autoload -Uz add-zsh-hook

# --- config:theme ---
# Load theme
source ~/.config/zsh/themes/charModel

# Completion menu keybindings
bindkey -N menuselect
bindkey -M menuselect '^[[A' up-line-or-history          # Up arrow: navigate up
bindkey -M menuselect '^[[B' down-line-or-history        # Down arrow: navigate down
bindkey -M menuselect '^[[D' backward-char               # Left arrow: go back
bindkey -M menuselect '^[[C' accept-line                 # Right arrow: accept selection
bindkey -M menuselect '^[OA' up-line-or-history          # Up arrow (alt terminal)
bindkey -M menuselect '^[OB' down-line-or-history        # Down arrow (alt terminal)
bindkey -M menuselect '^[OD' backward-char               # Left arrow (alt terminal)
bindkey -M menuselect '^[OC' accept-line                 # Right arrow: accept selection
bindkey -M menuselect '^M' accept-line                   # Enter: accept selection
bindkey -M menuselect '^I' menu-complete                 # Tab: cycle forward
bindkey -M menuselect '^[[Z' reverse-menu-complete       # Shift-Tab: cycle backward
bindkey -M menuselect '^[' send-break                    # Esc: cancel completion

# --- config:fastfetch ---
# Run fastfetch only once per session
if [[ !
-f /tmp/neofetch_run_$$ ]]; then
  fastfetch
  touch /tmp/neofetch_run_$$
fi

# --- config:zoxide ---
# initialize zoxide (provides `z` and `zi`, plus completions)
eval "$(zoxide init zsh)"

# --- config:keybindings (Final Fix for Autosuggestion) ---
# FIX: Force Right Arrow to accept autosuggestion by binding it after all plugins load.
# This ensures it overrides any conflict from zsh-history-substring-search.
# The key code for your terminal is confirmed to be '^[[C'
bindkey '^[[C' autosuggest-accept
bindkey '^[OC' autosuggest-accept
