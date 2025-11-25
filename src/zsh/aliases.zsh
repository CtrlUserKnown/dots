# zsh costum aliases
#
# author: Charlynder
# date created: 08.28.2025

# --- alias:functionality ---
alias -g cls="command clear"
alias -g ls="command eza --color=always"
alias -g la="command eza -al --icons --color=always"
alias -g ll="command eza -l --icons color=always"
alias -g reload="source ~/.zshrc"

# --- alias:homebrew ---
alias -g update="brew update"
alias -g upgrade="brew upgrade"
alias -g remove="brew uninstall"
alias -g search="brew search"

# --- alias:programming ---
alias -g y="yazi"
alias -g py="python3"
alias -g ipy="ipython3"
alias -g spi="swift package init"
alias -g sps="swift package show-dependencies"
alias -g spt="swift package test"
alias -g srn="swift run"
alias -g jv="java"
alias -g jc="javac"
alias -g rn="rustc"
alias -g cr="cargo run"

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

function termcon() {
    # open the terminal configuration file in the default editor (neovim)
    ${EDITOR:-nvim} ~/development/dotfiles/src/.zshrc
}

function muxcon() {
    # open the tmux configuration file in the default editor (neovim)
    ${EDITOR:-nvim} ~/development/dotfiles/src/tmux/tmux.conf
}

# --- alias:application ---
alias -g mux="command tmux"
alias -g b="command btop"
alias -g f="fzf"
