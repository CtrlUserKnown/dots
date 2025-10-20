# Charlynder Model theme for zsh
# A theme that changes based on different modes

# Environment variables
# Set the symbol for different environments
function tmuxbolt {
    echo -n "⚡️"
}

function cancer {
    echo -n "♋︎"
}

ARROW_DIRECTION="»"  # Default arrow direction

# Initialize vcs_info for git branch display
autoload -Uz vcs_info
setopt prompt_subst

# Configure vcs_info styles
zstyle ':vcs_info:*' check-for-changes true
zstyle ':vcs_info:*' unstagedstr '%F{red}*'   # display this when there are unstaged changes
zstyle ':vcs_info:*' stagedstr '%F{yellow}+'  # display this when there are staged changes
zstyle ':vcs_info:*' actionformats '%F{5}[%F{2}%b%F{3}|%F{1}%a%c%u%F{5}]%f '
zstyle ':vcs_info:*' formats '%F{5}[%F{2}%b%c%u%F{5}]%f '
zstyle ':vcs_info:svn:*' branchformat '%b'
zstyle ':vcs_info:svn:*' actionformats '%F{5}[%F{2}%b%F{1}:%F{3}%i%F{3}|%F{1}%a%c%u%F{5}]%f '
zstyle ':vcs_info:svn:*' formats '%F{5}[%F{2}%b%F{1}:%F{3}%i%c%u%F{5}]%f '
zstyle ':vcs_info:*' enable git cvs svn

# Functions to detect if we're in tmux or python 
function in_tmux {
    [[ -n "$TMUX" ]]
}

# Custom self-insert widget to catch normal typing
function my-self-insert() {
  ARROW_DIRECTION="»"  # Right arrow when typing (matches charlynderLite)
  zle self-insert
  zle reset-prompt
}

# Custom backspace widget
function my-backward-delete-char() {
  ARROW_DIRECTION="«"  # Left arrow when backspacing (matches charlynderLite)
  zle backward-delete-char
  zle reset-prompt
}

# Navigation key widgets
function my-backward-char() {
  ARROW_DIRECTION="«"  # Left arrow when moving left
  zle backward-char
  zle reset-prompt
}

function my-forward-char() {
  ARROW_DIRECTION="»"  # Right arrow when moving right
  zle forward-char
  zle reset-prompt
}

function my-up-line-or-history() {
  ARROW_DIRECTION="↑"  # Up arrow when moving up
  zle up-line-or-history
  zle reset-prompt
}

function my-down-line-or-history() {
  ARROW_DIRECTION="↓"  # Down arrow when moving down
  zle down-line-or-history
  zle reset-prompt
}

# Create the widgets
zle -N my-backward-delete-char
zle -N my-self-insert
zle -N my-backward-char
zle -N my-forward-char
zle -N my-up-line-or-history
zle -N my-down-line-or-history

# Bind the widgets
bindkey '^?' my-backward-delete-char  # Backspace
bindkey '^H' my-backward-delete-char  # Ctrl+H

# Bind navigation keys (primary sequences)
bindkey '^[[D' my-backward-char       # Left arrow
bindkey '^[[C' my-forward-char        # Right arrow
bindkey '^[[A' my-up-line-or-history  # Up arrow
bindkey '^[[B' my-down-line-or-history # Down arrow

# Alternative sequences for different terminals
bindkey '^[OD' my-backward-char       # Left arrow (alternative)
bindkey '^[OC' my-forward-char        # Right arrow (alternative)
bindkey '^[OA' my-up-line-or-history  # Up arrow (alternative)
bindkey '^[OB' my-down-line-or-history # Down arrow (alternative)

# Override the default self-insert to catch normal typing
for key in {a..z} {A..Z} {0..9}; do
  bindkey "$key" my-self-insert
done

# Common symbols
bindkey ' ' my-self-insert
bindkey '.' my-self-insert
bindkey ',' my-self-insert
bindkey '!' my-self-insert
bindkey '?' my-self-insert
bindkey ':' my-self-insert
bindkey ';' my-self-insert

# Function to update vcs_info and reset arrow after command
theme_precmd() {
  vcs_info
  # Reset arrow to default after command execution (matches charlynderLite)
  ARROW_DIRECTION="»"
}


# Function to build the appropriate prompt
function build_prompt {
    local prompt_str=""

    if in_tmux; then 
        prompt_str="$(tmuxbolt) [%F{cyan}%n%f:%F{blue}%c%f] %{$reset_color%}${vcs_info_msg_0_}%{$reset_color%} ~ "
    else
        prompt_str="$(cancer)[%F{050}%n%f: %~] %{$reset_color%}${vcs_info_msg_0_}%{$reset_color%}${ARROW_DIRECTION} "
    fi
    
    echo -n "$prompt_str"
}

# Set up the prompt with dynamic detection
PROMPT='$(build_prompt)'

# Add the precmd hook to update git info automatically
autoload -U add-zsh-hook
add-zsh-hook precmd theme_precmd
