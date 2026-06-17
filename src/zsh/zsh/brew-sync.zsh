# Homebrew sync hook — auto-updates Brewfile on install/uninstall/tap/untap
# Only runs when DEVELOPER_MODE is set (avoids trampling other users' Brewfiles)

if [[ -z "$DEVELOPER_MODE" || ! -d ~/.dots/.git ]]; then
    return
fi

_brew_sync() {
    local brewfile="$HOME/.dots/assets/Brewfile"
    local go_entries=()
    local cargo_entries=()

    while IFS= read -r line || [ -n "$line" ]; do
        [[ "$line" =~ ^go\ \"(.*)\"$ ]] && go_entries+=("$line")
        [[ "$line" =~ ^cargo\ \"(.*)\"$ ]] && cargo_entries+=("$line")
    done < "$brewfile"

    command brew bundle dump --force --file="$brewfile" 2>/dev/null

    for entry in "${go_entries[@]}" "${cargo_entries[@]}"; do
        echo "$entry" >> "$brewfile"
    done
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
