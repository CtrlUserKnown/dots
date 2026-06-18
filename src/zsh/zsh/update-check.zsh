# Dotfiles update check
# dotfiles v1.2.1
# Sources from .zshrc — checks daily for upstream changes
# Set DEVELOPER_MODE=1 in your .zshrc to disable auto-updates

if [[ -z "$DEVELOPER_MODE" && -d ~/.dots/.git && $- == *i* ]]; then
    stamp="$HOME/.config/zsh/.update_stamp"
    now=$(date +%s)
    last_check=0
    [[ -f "$stamp" ]] && last_check=$(<"$stamp")

    if (( now - last_check > 86400 )); then
        echo "$now" > "$stamp"
        (
            cd ~/.dots 2>/dev/null || exit
            git fetch --depth 1 origin 2>/dev/null
            behind=$(git rev-list --count HEAD..origin/HEAD 2>/dev/null)
            if [[ -n "$behind" && "$behind" -gt 0 ]]; then
                print ""
                print "📦 Dotfiles update available ($behind new commits)"
                print -n "Pull changes? [Y/n] "
                read -q reply
                print ""
                if [[ "$reply" == "y" || "$reply" == "Y" || -z "$reply" ]]; then
                    git pull --ff-only 2>/dev/null
                    ln -sf "$HOME/.dots/src/bat" "$HOME/.config/bat"
                    ln -sf "$HOME/.dots/src/fastfetch" "$HOME/.config/fastfetch"
                    ln -sf "$HOME/.dots/src/ghostty" "$HOME/.config/ghostty"
                    ln -sf "$HOME/.dots/src/tmux" "$HOME/.config/tmux"
                    ln -sf "$HOME/.dots/src/zsh/zsh" "$HOME/.config/zsh"
                    ln -sf "$HOME/.dots/src/zsh/.zshrc" "$HOME/.zshrc"
                    print "✅ Dotfiles updated."
                else
                    print "Skipped."
                fi
                print ""
            fi
        )
    fi
fi
