# Dotfiles update check
# dotfiles v1.4.1
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
            # shallow fetch — only grabs the latest ref to minimize bandwidth
            git fetch --depth 1 --tags origin 2>/dev/null
            behind=$(git rev-list --count HEAD..origin/HEAD 2>/dev/null)
            if [[ -n "$behind" && "$behind" -gt 0 ]]; then
                upstream_version=$(git describe --tags --abbrev=0 origin/HEAD 2>/dev/null | sed 's/^v//')

                print ""
                print "📦 Dotfiles update available ($behind new commit(s))"
                [[ -n "$upstream_version" ]] && print "   New version: v${upstream_version}"
                print -n "Pull changes? [Y/n] "
                read -q reply
                print ""
                if [[ "$reply" == "y" || "$reply" == "Y" || -z "$reply" ]]; then
                    # refuse to merge if diverged — this repo should never have local commits
                    git pull --ff-only 2>/dev/null
                    python3 ~/.config/zsh/dots.py --repair-symlinks
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
