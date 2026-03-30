# CtrlUserKnown zshrc configuration file
# date created: 10.14.2025

# --- config:locale ---
export LANG=en_US.UTF-8
export LC_ALL=en_US.UTF-8

# --- config:Homebrew ---
if [[ -f "/opt/homebrew/bin/brew" ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
elif [[ -f "/usr/local/bin/brew" ]]; then
    eval "$(/usr/local/bin/brew shellenv)"
fi

# --- config:editor ---
export EDITOR="nvim"
export VISUAL="nvim"

# --- config:eza ---
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

export LS_COLORS="di=38;2;196;167;231:ln=38;5;211:ex=38;2;86;148;159"

# --- config:fzf ---
export FZF_DEFAULT_COMMAND='fd --type f --hidden --follow --exclude .git'
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
export FZF_CTRL_T_COMMAND="$FZF_DEFAULT_COMMAND"
export FZF_CTRL_T_OPTS='--preview "bat --color=always --line-range :500 {}"'
export FZF_ALT_C_COMMAND='fd --type d --hidden --follow --exclude .git'
export FZF_ALT_C_OPTS='--preview "tree -C {} | head -200"'
export FZF_CTRL_R_OPTS='--preview "echo {}" --preview-window down:3:wrap'

# --- config:history ---
HISTFILE=${ZDOTDIR:-$HOME}/.zsh_history
HISTSIZE=10000
SAVEHIST=10000
setopt HIST_IGNORE_ALL_DUPS SHARE_HISTORY APPEND_HISTORY INC_APPEND_HISTORY

# --- config:options ---
WORDCHARS=${WORDCHARS//[\/]}
setopt EXTENDED_GLOB AUTO_CD

# --- config:fastfetch ---
if [[ ! -f /tmp/zsh_fastfetch_$$ ]] && [[ $- == *i* ]]; then
    fastfetch
    print ""
    print "run 'commands custom' to see your aliases and functions"
    print ""
    touch /tmp/zsh_fastfetch_$$
fi

# --- config:completions ---
if [[ -d "/opt/homebrew/share/zsh-completions" ]]; then
    fpath=(/opt/homebrew/share/zsh-completions $fpath)
fi

autoload -Uz compinit
zmodload zsh/complist

compinit

# --- config:zoxide ---
eval "$(zoxide init zsh)"

zstyle ':completion:*' menu select
zstyle ':completion:*' matcher-list 'm:{a-zA-Z}={A-Za-z}'
zstyle ':completion:*' list-colors "${(s.:.)LS_COLORS}"
zstyle ':completion:*' completer _complete _approximate
zstyle ':completion:*' group-name ''
zstyle ':completion:*:descriptions' format '%F{yellow}-- %d --%f'
zstyle ':completion:*:warnings' format '%F{red}-- no matches found --%f'

# --- config:fzf-tab ---
# clone with: git clone https://github.com/Aloxaf/fzf-tab ~/.config/zsh/plugins/fzf-tab
if [[ -f ~/.config/zsh/plugins/fzf-tab/fzf-tab.plugin.zsh ]]; then
    source ~/.config/zsh/plugins/fzf-tab/fzf-tab.plugin.zsh

    # disable sort for files/dirs so they appear in natural order
    zstyle ':completion:*' sort false

    # use fd for path completion
    zstyle ':fzf-tab:complete:cd:*' fzf-preview 'eza --color=always --icons $realpath'
    zstyle ':fzf-tab:complete:__zoxide_z:*' fzf-preview 'eza --color=always --icons $realpath'

    # show file preview for most completions
    zstyle ':fzf-tab:complete:*:*' fzf-preview 'bat --color=always --style=numbers $realpath 2>/dev/null || eza --color=always --icons $realpath'

    # Rose Pine colors for fzf-tab popup
    zstyle ':fzf-tab:*' fzf-flags \
        --color=fg:#e0def4,bg:#191724,hl:#eb6f92 \
        --color=fg+:#e0def4,bg+:#26233a,hl+:#eb6f92 \
        --color=info:#9ccfd8,prompt:#f6c177,pointer:#c4a7e7 \
        --color=marker:#ebbcba,spinner:#eb6f92,header:#31748f \
        --color=border:#524f67

    # switch between tab/shift-tab to cycle through results
    zstyle ':fzf-tab:*' switch-group '<' '>'
fi

# --- config:aliases ---
if [[ -f ~/.config/zsh/.aliases ]]; then
    source ~/.config/zsh/.aliases
fi

# --- config:functions ---
if [[ -f ~/.config/zsh/.functions ]]; then
    source ~/.config/zsh/.functions
fi

# --- config:hooks ---
chpwd() {
    local current_dir="${PWD}"
    if [[ "$current_dir" == "${HOME}/.config" || "$current_dir" == "${HOME}/development" ]]; then
        ls
    fi
}

# --- config:theme ---
source ~/.config/zsh/themes/charModel

# --- config:keybindings ---
autoload -Uz edit-command-line
zle -N edit-command-line
bindkey '^E' edit-command-line
bindkey '^_' undo

# --- config:ruby ---
export PATH="/opt/homebrew/opt/ruby@3.4/bin:$PATH"
export LDFLAGS="-L/opt/homebrew/opt/ruby@3.4/lib"
export CPPFLAGS="-I/opt/homebrew/opt/ruby@3.4/include"

# --- config:java ---
export JAVA_HOME=$(/usr/libexec/java_home)

# --- config:plugins ---
# zsh-autosuggestions
if [[ -f "/opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh" ]]; then
    source /opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh
fi
unset ZSH_AUTOSUGGEST_USE_ASYNC
ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE='fg=242'
ZSH_AUTOSUGGEST_BUFFER_MAX_SIZE=20
ZSH_AUTOSUGGEST_CLEAR_WIDGETS+=(expand-or-complete)

# zsh-history-substring-search
if [[ -f "/opt/homebrew/share/zsh-history-substring-search/zsh-history-substring-search.zsh" ]]; then
    source /opt/homebrew/share/zsh-history-substring-search/zsh-history-substring-search.zsh
fi
bindkey '^[[A' history-substring-search-up
bindkey '^[[B' history-substring-search-down

# zsh-syntax-highlighting — must be sourced last
ZSH_HIGHLIGHT_HIGHLIGHTERS=(main brackets)
typeset -A ZSH_HIGHLIGHT_STYLES
ZSH_HIGHLIGHT_STYLES[comment]='fg=242'
if [[ -f "/opt/homebrew/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh" ]]; then
    source /opt/homebrew/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh
fi
