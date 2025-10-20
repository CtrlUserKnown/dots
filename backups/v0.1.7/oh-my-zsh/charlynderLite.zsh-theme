# Charlynder Lite
# very basic and lightwight zsh prompt does not change like the model prompt

# create the symbol for the apple icon
function cancer {
  echo -n "♋︎"
}

# Variable to track arrow direction
ARROW_DIRECTION="»"

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

# Custom backspace widget
function my-backward-delete-char() {
  ARROW_DIRECTION="«"
  zle backward-delete-char
  zle reset-prompt
}

# Custom self-insert widget to catch normal typing
function my-self-insert() {
  ARROW_DIRECTION="»"
  zle self-insert
  zle reset-prompt
}

# Create the widgets
zle -N my-backward-delete-char
zle -N my-self-insert

# Bind the widgets
bindkey '^?' my-backward-delete-char  # Backspace
bindkey '^H' my-backward-delete-char  # Ctrl+H

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
  # Reset arrow to default after command execution
  ARROW_DIRECTION="»"
}

# Set up the prompt with dynamic vcs_info
PROMPT=' $(cancer) %{$reset_color%}${vcs_info_msg_0_}%{$reset_color%} $'\n'
 [%F{050}%n%f: %~] ${ARROW_DIRECTION} '

# Add the precmd hook to update git info automatically
autoload -U add-zsh-hook
add-zsh-hook precmd theme_precmd
