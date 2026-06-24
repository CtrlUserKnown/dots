# CtrlUserKnown zshrc configuration file
# dotfiles v1.2.1
# date created: 10.14.2025

# --- config:locale ---
export LANG=en_US.UTF-8
export LC_ALL=en_US.UTF-8
export PATH="$HOME/.config/emacs/bin:$PATH"

# --- config:Homebrew ---
if [[ -f "/opt/homebrew/bin/brew" ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
elif [[ -f "/usr/local/bin/brew" ]]; then
    eval "$(/usr/local/bin/brew shellenv)"
fi

# --- config:XDG (Linux) ---
if [[ "$(uname -s)" == "Linux" ]]; then
    export PATH="$HOME/.local/bin:$PATH"
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
  --color=fg:-1,bg:-1,hl:13
  --color=fg+:-1,bg+:8,hl+:14
  --color=info:12,prompt:10,pointer:13
  --color=marker:11,spinner:13,header:6
  --color=border:8,preview-bg:-1
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

# --- config:developer mode ---
# Author is auto-enrolled in developer mode (no update prompts).
# Other users get auto-updates by default.
# Override by setting DEVELOPER_MODE=0 or DEVELOPER_MODE=1 before this line.
if [[ -z "${DEVELOPER_MODE:-}" ]]; then
    [[ "$USER" == "christian" ]] && typeset -g DEVELOPER_MODE=1
fi

# --- config:developer hooks ---
# brew sync — auto-updates Brewfile on install/uninstall (developer mode only)
if [[ -f ~/.config/zsh/brew-sync.zsh ]]; then
    source ~/.config/zsh/brew-sync.zsh
fi

# --- config:update check ---
# checks daily for dotfiles updates (skipped in developer mode)
if [[ -f ~/.config/zsh/update-check.zsh ]]; then
    source ~/.config/zsh/update-check.zsh
fi

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
elif [[ -d "/usr/share/zsh/site-functions" ]]; then
    fpath=(/usr/share/zsh/site-functions $fpath)
fi

autoload -Uz compinit
zmodload zsh/complist

compinit

# --- config:zoxide ---
eval "$(zoxide init zsh)"

# --- config:carapace ---
if command -v carapace >/dev/null 2>&1; then
    source <(carapace _carapace)
fi

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
    zstyle ':fzf-tab:complete:-command-:*' fzf-preview 'whence -v $word 2>/dev/null'

    # show what a command name resolves to (alias, function, or binary)
    zstyle ':fzf-tab:complete:*:command-word' fzf-preview 'whence -v $word'

    # show file preview for most completions
    zstyle ':fzf-tab:complete:*:*' fzf-preview 'bat --color=always --style=numbers $realpath 2>/dev/null || eza --color=always --icons $realpath'

    # use terminal-native colors so fzf matches the current ghostty theme
    zstyle ':fzf-tab:*' fzf-flags \
        --color=fg:-1,bg:-1,hl:13 \
        --color=fg+:-1,bg+:8,hl+:14 \
        --color=info:12,prompt:10,pointer:13 \
        --color=marker:11,spinner:13,header:6 \
        --color=border:8

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
if [[ "$(uname -s)" == "Darwin" ]]; then
    export PATH="/opt/homebrew/opt/ruby@3.4/bin:$PATH"
    export LDFLAGS="-L/opt/homebrew/opt/ruby@3.4/lib"
    export CPPFLAGS="-I/opt/homebrew/opt/ruby@3.4/include"
fi

# --- config:java ---
if [[ "$(uname -s)" == "Darwin" ]]; then
    export JAVA_HOME=$(/usr/libexec/java_home 2>/dev/null)
elif command -v java &>/dev/null; then
    export JAVA_HOME=$(dirname "$(dirname "$(readlink -f "$(which java)")")")
fi

# --- config:plugins ---
# zsh-autosuggestions
if [[ -f "/opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh" ]]; then
    source /opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh
elif [[ -f "/usr/share/zsh-autosuggestions/zsh-autosuggestions.zsh" ]]; then
    source /usr/share/zsh-autosuggestions/zsh-autosuggestions.zsh
fi
unset ZSH_AUTOSUGGEST_USE_ASYNC
ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE='fg=242'
ZSH_AUTOSUGGEST_BUFFER_MAX_SIZE=20
ZSH_AUTOSUGGEST_CLEAR_WIDGETS+=(expand-or-complete)

# zsh-history-substring-search
if [[ -f "/opt/homebrew/share/zsh-history-substring-search/zsh-history-substring-search.zsh" ]]; then
    source /opt/homebrew/share/zsh-history-substring-search/zsh-history-substring-search.zsh
elif [[ -f "$HOME/.config/zsh/plugins/zsh-history-substring-search/zsh-history-substring-search.zsh" ]]; then
    source "$HOME/.config/zsh/plugins/zsh-history-substring-search/zsh-history-substring-search.zsh"
fi
bindkey '^[[A' history-substring-search-up
bindkey '^[[B' history-substring-search-down

# zsh-syntax-highlighting — must be sourced last
ZSH_HIGHLIGHT_HIGHLIGHTERS=(main brackets)
typeset -A ZSH_HIGHLIGHT_STYLES
ZSH_HIGHLIGHT_STYLES[comment]='fg=242'
if [[ -f "/opt/homebrew/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh" ]]; then
    source /opt/homebrew/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh
elif [[ -f "/usr/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh" ]]; then
    source /usr/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh
fi


