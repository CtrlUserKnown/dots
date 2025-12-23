# zsh costum aliases
#
# author: Charlynder
# date created: 08.28.2025

# --- alias:functionality ---
alias cls="command clear"
alias ls="command eza --color=always"
alias la="command eza -al --icons --color=always"
alias ll="command eza -l --icons color=always"
alias reload="source ~/.zshrc"

# --- alias:homebrew ---
alias update="brew update"
alias upgrade="brew upgrade"
alias remove="brew uninstall"
alias search="brew search"

# --- alias:programming ---
alias y="yazi"
alias py="python3"
alias ipy="ipython3"
alias spi="swift package init"
alias sps="swift package show-dependencies"
alias spt="swift package test"
alias srn="swift run"
alias jv="java"
alias jc="javac"
alias rn="rustc"
alias cr="cargo run"

# --- alias:functions
function download() {
  # Check if curl is available using Zsh's 'whence'
  if whence -q curl; then
    echo "Using curl to download: $1"
    # -L: Follow redirects, -O: Write output to a local file
    curl -L -O "$1"
  
  # Check if wget is available
  elif whence -q wget; then
    echo "Using wget to download: $1"
    # -c: Continue download, --show-progress: Display progress
    wget -c --show-progress "$1"
    
  # Neither is found
  else
    echo "Error: Neither curl nor wget is installed." >&2
    return 1
  fi
}

function configterminal() {
    # open the terminal configuration file in the default editor (neovim)
    ${EDITOR:-nvim} ~/development/dotfiles/src/.zshrc
}

function configmux() {
    # open the tmux configuration file in the default editor (neovim)
    ${EDITOR:-nvim} ~/development/dotfiles/src/tmux/tmux.conf
}

function configghostty() {
    # open the ghost configuration file in the default editor (neovim)
    ${EDITOR:-nvim} ~/development/dotfiles/src/ghostty/config
}

function configalias() {
    # open the aliases configuration file in the default editor (neovim)
    ${EDITOR:-nvim} ~/development/dotfiles/src/zsh/aliases.zsh
}

function attach() {
    # Attach to an existing tmux session
    if tmux list-sessions &> /dev/null; then
        local session=$(tmux list-sessions | cut -d: -f1 | fzf)
        if [ -n "$session" ]; then
            tmux attach-session -t "$session"
        fi
    else
        echo "No tmux sessions found"
    fi
}

# --- alias:application ---
alias mux="command tmux"
alias b="command btop"
alias f="fzf"
