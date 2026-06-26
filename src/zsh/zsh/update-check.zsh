# Dotfiles update check
# dotfiles v1.3.0
# Sources from .zshrc — checks daily for upstream changes
# Set DEVELOPER_MODE=1 in your .zshrc to disable auto-updates

if [[ -z "$DEVELOPER_MODE" && -d ~/.dots/.git && $- == *i* ]]; then
    stamp="$HOME/.config/zsh/.update_stamp"
    version_stamp="$HOME/.config/zsh/.version_stamp"
    now=$(date +%s)
    last_check=0
    [[ -f "$stamp" ]] && last_check=$(<"$stamp")

    # Detect version change from a previous update (runs every shell start)
    if [[ -f "$version_stamp" ]]; then
        last_version=$(<"$version_stamp")
        if [[ -n "${DOTFILES_VERSION:-}" && "$last_version" != "$DOTFILES_VERSION" ]]; then
            print ""
            print "✨ Dotfiles updated: v${last_version} → v${DOTFILES_VERSION}"
            print "   Run 'config' to review your config files, or check the CHANGELOG at ~/.dots/CHANGELOG"
            print ""
            echo "$DOTFILES_VERSION" > "$version_stamp"
        fi
    else
        # First run — write the current version
        [[ -n "${DOTFILES_VERSION:-}" ]] && echo "$DOTFILES_VERSION" > "$version_stamp"
    fi

    if (( now - last_check > 86400 )); then
        echo "$now" > "$stamp"
        (
            cd ~/.dots 2>/dev/null || exit
            git fetch --depth 1 origin 2>/dev/null
            behind=$(git rev-list --count HEAD..origin/HEAD 2>/dev/null)
            if [[ -n "$behind" && "$behind" -gt 0 ]]; then
                # Peek at the version in the upstream .zshrc before pulling
                upstream_version=$(git show origin/HEAD:src/zsh/.zshrc 2>/dev/null | grep '^DOTFILES_VERSION=' | head -1 | tr -d '"' | cut -d= -f2)

                print ""
                print "📦 Dotfiles update available ($behind new commit(s))"
                [[ -n "$upstream_version" ]] && print "   New version: v${upstream_version}"
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
                    # Write the new version to stamp so next shell open shows the upgrade notice
                    [[ -n "$upstream_version" ]] && echo "$upstream_version" > "$version_stamp"
                    print "✅ Dotfiles updated to v${upstream_version:-?}. Restart your shell to apply changes."
                else
                    print "Skipped."
                fi
                print ""
            fi
        )
    fi
fi
