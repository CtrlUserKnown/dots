# CtrlUserKnown zshrc configuration file
# dotfiles v1.5.5
# date created: 10.14.2025

# Derived from the latest git tag — no manual version bumping needed
DOTFILES_VERSION=$(git -C "${HOME}/.dots" describe --tags --abbrev=0 2>/dev/null | sed 's/^v//')
: "${DOTFILES_VERSION:=dev}"

# Abort early if the ~/.config/zsh symlink is broken — rc.zsh won't be reachable
if [[ $- == *i* && ! -e "${HOME}/.config/zsh/themes/charModel" ]]; then
    print "⚠️  dots: ~/.config/zsh symlink is broken — run setup.sh to repair"
    return
fi

source "${HOME}/.config/zsh/rc.zsh"
