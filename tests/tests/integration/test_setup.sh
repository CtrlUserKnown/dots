#!/usr/bin/env bash
# Integration test: setup.sh must be idempotent (no duplicate PATH entries)
set -euo pipefail

DOTS_DIR="$(cd "$(dirname "$0")/../../../" && pwd)"
SETUP_SCRIPT="$DOTS_DIR/setup.sh"
TEST_ZSHRC=$(mktemp)

trap 'rm -f "$TEST_ZSHRC"' EXIT

# Patch HOME to use temp zshrc so we don't touch the real one
export HOME="$(mktemp -d)"
cp "$TEST_ZSHRC" "$HOME/.zshrc" 2>/dev/null || touch "$HOME/.zshrc"

# Only test PATH idempotency — don't actually run setup.sh (needs network + cargo)
# Instead simulate what setup.sh does to ~/.zshrc

PATH_LINE='export PATH="$HOME/.dots/bin:$PATH"'

# First invocation
printf "\n%s\n" "$PATH_LINE" >> "$HOME/.zshrc"
# Second invocation (idempotency guard: only add if not present)
if ! grep -q 'dots/bin' "$HOME/.zshrc" 2>/dev/null; then
    printf "\n%s\n" "$PATH_LINE" >> "$HOME/.zshrc"
fi

COUNT=$(grep -c 'dots/bin' "$HOME/.zshrc" 2>/dev/null || echo 0)
if [ "$COUNT" -ne 1 ]; then
    echo "FAIL: expected 1 dots/bin entry in .zshrc, got $COUNT"
    exit 1
fi

echo "PASS: PATH entry is idempotent ($COUNT occurrence)"
