# Ghostty Debug Theme
# Simplified theme to test spacing issues in Ghostty
# Use this theme temporarily to isolate spacing problems

# Initialize vcs_info for git branch display
autoload -Uz vcs_info
setopt prompt_subst

# Configure vcs_info styles (simplified)
zstyle ':vcs_info:*' check-for-changes true
zstyle ':vcs_info:*' unstagedstr '*'
zstyle ':vcs_info:*' stagedstr '+'
zstyle ':vcs_info:*' formats '[%b%c%u] '
zstyle ':vcs_info:*' enable git

# Function to update vcs_info before each prompt
theme_precmd() {
  vcs_info
}

# Simple prompt without complex Unicode characters
PROMPT='%n:%~ ${vcs_info_msg_0_}$ '

# Add the precmd hook
autoload -U add-zsh-hook
add-zsh-hook precmd theme_precmd