#!/usr/bin/env bash
set -euo pipefail

# ── dots uninstall script ─────────────────────────────────────────────────────

DOTS_BIN="$HOME/.dots/bin/dots"
ZSHRC="$HOME/.zshrc"

info()  { printf "  → %s\n" "$*"; }
ok()    { printf "  ✓ %s\n" "$*"; }
warn()  { printf "  ⚠ %s\n" "$*" >&2; }

# ── remove binary ─────────────────────────────────────────────────────────────

if [ -f "$DOTS_BIN" ]; then
    rm -f "$DOTS_BIN"
    ok "Removed $DOTS_BIN"
else
    warn "Binary not found at $DOTS_BIN"
fi

# ── remove symlinks ───────────────────────────────────────────────────────────

KNOWN_LINKS=(
    "$HOME/.config/bat"
    "$HOME/.config/fastfetch"
    "$HOME/.config/zsh"
    "$HOME/.zshrc"
    "$HOME/.config/ghostty"
)

info "Removing dots-managed symlinks…"
for link in "${KNOWN_LINKS[@]}"; do
    if [ -L "$link" ]; then
        rm -f "$link"
        ok "Removed symlink $link"
    fi
done

# ── remove PATH export from ~/.zshrc ─────────────────────────────────────────

if [ -f "$ZSHRC" ] && grep -q 'dots/bin' "$ZSHRC" 2>/dev/null; then
    grep -v 'dots/bin' "$ZSHRC" > "$ZSHRC.tmp" && mv "$ZSHRC.tmp" "$ZSHRC"
    ok "Removed dots PATH export from ~/.zshrc"
fi

# ── done ──────────────────────────────────────────────────────────────────────

printf "\n  dots uninstalled — run 'rm -rf ~/.dots' to remove all configuration\n\n"
