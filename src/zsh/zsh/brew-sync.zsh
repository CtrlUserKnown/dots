# Homebrew sync hook — auto-updates Brewfile on install/uninstall/tap/untap
# dotfiles v1.5.2
# Only runs when DEVELOPER_MODE is set (avoids trampling other users' Brewfiles)

if [[ -z "$DEVELOPER_MODE" || ! -d ~/.dots/.git ]]; then
    return
fi

_brew_sync() {
    local brewfile="$HOME/.dots/assets/Brewfile"
    local go_entries=()
    local cargo_entries=()
    local has_neovim=false

    # brew bundle dump doesn't track go or cargo installs — re-append them manually
    while IFS= read -r line || [ -n "$line" ]; do
        [[ "$line" =~ ^go\ \"(.*)\"$ ]] && go_entries+=("$line")
        [[ "$line" =~ ^cargo\ \"(.*)\"$ ]] && cargo_entries+=("$line")
        [[ "$line" =~ ^brew\ \"neovim\"$ ]] && has_neovim=true
    done < "$brewfile"

    command brew bundle dump --force --file="$brewfile" 2>/dev/null

    # Restore go/cargo entries (brew bundle dump omits them)
    for entry in "${go_entries[@]}" "${cargo_entries[@]}"; do
        echo "$entry" >> "$brewfile"
    done

    # keep neovim out if the user intentionally removed it
    if [[ "$has_neovim" == false ]]; then
        if grep -q '^brew "neovim"$' "$brewfile"; then
            grep -v '^brew "neovim"$' "$brewfile" > "${brewfile}.tmp" && mv "${brewfile}.tmp" "$brewfile"
        fi
    fi
}

brew() {
    local cmd="$1"
    command brew "$@"
    local ret=$?
    case "$cmd" in
        install|uninstall|tap|untap|rm|remove)
            [[ $ret -eq 0 ]] && _brew_sync
            ;;
    esac
    return $ret
}
