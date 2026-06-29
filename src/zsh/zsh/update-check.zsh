# Dotfiles update check
# dotfiles v1.5.3
# Sources from .zshrc — checks every 10 minutes for upstream changes
# Set DEVELOPER_MODE=1 in your .zshrc to disable auto-updates

if [[ -z "$DEVELOPER_MODE" && -d ~/.dots/.git && $- == *i* ]]; then
    stamp="$HOME/.config/zsh/.update_stamp"
    version_stamp="$HOME/.config/zsh/.version_stamp"
    # stamp throttles the daily check, version_stamp persists the last version to detect upgrades on next shell open
    now=$(date +%s)
    last_check=0
    [[ -f "$stamp" ]] && last_check=$(<"$stamp")

    # Detect version change from a previous update (runs every shell start)
    if [[ -f "$version_stamp" ]]; then
        last_version=$(<"$version_stamp")
        if [[ -n "${DOTFILES_VERSION:-}" && "$last_version" != "$DOTFILES_VERSION" ]]; then
            print ""
            print "✨ Dotfiles updated: v${last_version} → v${DOTFILES_VERSION}"
            print "   Run 'dots' to review your config files, or check the CHANGELOG at ~/.dots/CHANGELOG"
            print ""
            echo "$DOTFILES_VERSION" > "$version_stamp"
        fi
    else
        # First run — write the current version
        [[ -n "${DOTFILES_VERSION:-}" ]] && echo "$DOTFILES_VERSION" > "$version_stamp"
    fi

    if (( now - last_check > 600 )); then
        echo "$now" > "$stamp"
        (
            cd ~/.dots 2>/dev/null || exit
            git fetch --depth 1 --tags origin 2>/dev/null
            upstream_version=$(git tag --list 'v*' --sort=-version:refname 2>/dev/null | head -1 | sed 's/^v//')
            local_version="${DOTFILES_VERSION:-}"

            if [[ -n "$upstream_version" && -n "$local_version" && "$upstream_version" != "$local_version" ]]; then
                newer=$(printf '%s\n%s' "$local_version" "$upstream_version" | sort -V | tail -1)
                if [[ "$newer" == "$upstream_version" ]]; then
                    print ""
                    print "📦 Dotfiles update available: v${local_version} → v${upstream_version}"
                    print -n "Pull changes? [Y/n] "
                    read -q reply
                    print ""
                    if [[ "$reply" == "y" || "$reply" == "Y" || -z "$reply" ]]; then
                        # refuse to merge if diverged — this repo should never have local commits
                        git pull --ff-only 2>/dev/null
                        python3 ~/.config/zsh/dots.py --repair-symlinks
                        echo "$upstream_version" > "$version_stamp"
                        print "✅ Dotfiles updated to v${upstream_version}. Restart your shell to apply changes."
                    else
                        print "Skipped."
                    fi
                    print ""
                fi
            fi
        )
    fi
fi
