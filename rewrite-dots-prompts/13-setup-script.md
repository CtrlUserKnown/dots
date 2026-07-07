# Prompt 13 — Setup Script

## Before writing any code

1. Read `~/development/dots/setup.sh` in full — understand every step: clone, symlink creation, zsh config, existing file handling, and what the Python version was calling.
2. Read `~/development/dots/dots-rs/Cargo.toml` — confirm the workspace structure and binary name.
3. Read `~/development/dots/src/zsh/zsh/rc.zsh` — note where the dots binary is expected in PATH and how DOTFILES_VERSION is set.
4. Read `~/development/dots/src/zsh/.zshrc` — confirm the minimal bootstrapper structure.
5. State your plan: how the new `setup.sh` builds (or downloads) the Rust binary, where it is placed, how it interacts with the new config system, and how it handles existing Python installs.
6. **Wait for the user to confirm before writing any code.**

---

## Objective

Replace `setup.sh` with a new version that:
1. Clones (or updates) `~/.dots` from the GitHub repo
2. Builds the Rust binary **or** downloads a prebuilt release binary
3. Places the binary at `~/.dots/bin/dots` and adds it to PATH
4. Creates all symlinks (delegates to `dots health --fix`)
5. Handles existing Python installs gracefully (no Python files will be present after migration, but the script should not break if they are)
6. Is idempotent — safe to run again on an already-set-up system

---

## Setup script structure

The new `setup.sh` must be POSIX sh (not bash-specific) for maximum portability. Exception: one bash feature is allowed — `set -euo pipefail`. But prefer `sh -e` semantics throughout.

```sh
#!/usr/bin/env bash
set -euo pipefail
```

### Steps in order

**Step 1 — Clone or update `~/.dots`**
```sh
DOTS_DIR="$HOME/.dots"
DOTS_REPO="https://github.com/CtrlUserKnown/dots"

if [ -d "$DOTS_DIR/.git" ]; then
    echo "→ Updating ~/.dots…"
    git -C "$DOTS_DIR" pull --ff-only
else
    echo "→ Cloning dots…"
    git clone "$DOTS_REPO" "$DOTS_DIR"
fi
```

**Step 2 — Build or download binary**

The script checks for three scenarios in order:
1. Binary already at `~/.dots/bin/dots` and matches current git tag → skip build.
2. Cargo is available → build from source.
3. Cargo is not available → download prebuilt binary from GitHub Releases.

```sh
DOTS_BIN="$DOTS_DIR/bin/dots"
DOTS_VERSION=$(git -C "$DOTS_DIR" describe --tags --abbrev=0 2>/dev/null || echo "dev")

# Skip if already at correct version
if [ -x "$DOTS_BIN" ] && [ "$("$DOTS_BIN" --version 2>/dev/null)" = "dots $DOTS_VERSION" ]; then
    echo "→ Binary already up to date ($DOTS_VERSION)"
elif command -v cargo >/dev/null 2>&1; then
    echo "→ Building dots from source…"
    cargo build --manifest-path "$DOTS_DIR/dots-rs/Cargo.toml" --release
    mkdir -p "$DOTS_DIR/bin"
    cp "$DOTS_DIR/dots-rs/target/release/dots" "$DOTS_BIN"
else
    echo "→ Downloading prebuilt binary…"
    _download_binary "$DOTS_VERSION"
fi
```

`_download_binary()` function:
- Detect OS + arch: `uname -s` + `uname -m`
- Map to release asset filename: `dots-{version}-{os}-{arch}.tar.gz`
- macOS arm64: `dots-{version}-darwin-aarch64.tar.gz`
- macOS x86_64: `dots-{version}-darwin-x86_64.tar.gz`
- Linux x86_64: `dots-{version}-linux-x86_64.tar.gz`
- Download with `curl -fsSL` or `wget -q -O -`
- Extract to `~/.dots/bin/`
- If download fails: print instructions to install Rust and run `cargo build`, then exit 1.

**Step 3 — Add binary to PATH**

Check `~/.zshrc` (or shell profile). If `~/.dots/bin` is not already in PATH:
```sh
echo 'export PATH="$HOME/.dots/bin:$PATH"' >> "$HOME/.zshrc"
```

Do not duplicate the line if it already exists: `grep -q 'dots/bin' "$HOME/.zshrc"` guard.

**Step 4 — Run `dots health --fix`**

This delegates all symlink creation to the Rust binary (prompt 03+05):
```sh
"$DOTS_BIN" health --fix
```

If the binary is not yet in PATH (fresh install), use the full path.

**Step 5 — Initialize config**

```sh
"$DOTS_BIN" init --quiet
```

`dots init` creates `~/.dots/settings.toml` if absent (with defaults) and `~/.personal/` if absent. The `--quiet` flag suppresses the greeting.

**Step 6 — Post-install message**

```
✓ dots installed — version 1.6.0

  restart your shell or run:
    source ~/.zshrc

  then type 'dots' to get started
```

---

## Idempotency contract

Running `setup.sh` a second time must:
- Not re-clone if `~/.dots/.git` exists
- Not rebuild if binary is already at the correct version
- Not duplicate the PATH export in `.zshrc`
- Not overwrite existing `settings.toml` or `~/.personal/` files
- Re-run `dots health --fix` (which is idempotent itself)

---

## Handling Python leftovers

If `~/.dots/src/zsh/zsh/dots.py` exists from the Python era:
- Do nothing. The Rust binary is at a different path (`~/.dots/bin/dots`).
- The old Python TUI may still run as `python3 ~/.dots/src/zsh/zsh/dots.py` but dots no longer calls it.
- The setup script does NOT delete Python files — that's user's choice.

---

## Uninstall script

Add a separate `uninstall.sh` that:
1. Removes `~/.dots/bin/dots`
2. Removes all symlinks created by dots (reads the symlink manifest from `~/.dots/.symlinks.json`)
3. Removes the PATH export line from `~/.zshrc`
4. Does NOT remove `~/.dots/` itself (user's data)
5. Prints: `"dots uninstalled — run 'rm -rf ~/.dots' to remove all configuration"`

---

## Error handling to cover

| Scenario | Expected behavior |
|----------|-------------------|
| No `git` on system | `echo "Error: git is required"; exit 1` |
| `git pull --ff-only` fails (local changes) | `echo "Error: local changes in ~/.dots — stash or commit them"; exit 1` |
| `cargo build` fails | Print cargo's stderr; suggest filing a bug; exit 1 |
| Download fails (no internet) | Print manual install instructions; exit 1 |
| Unknown OS/arch | Print unsupported message; suggest building from source; exit 1 |
| `dots health --fix` fails | Print the error; do not exit — setup continues with a warning |

---

## Testing — three passes

**Pass 1 — idempotency on existing install:**
```sh
bash setup.sh    # fresh install
bash setup.sh    # run again
# No errors, no duplicate PATH line, binary version unchanged
grep -c 'dots/bin' ~/.zshrc | grep '^1$'
```

**Pass 2 — PATH not duplicated:**
```sh
# Manually add the PATH line
echo 'export PATH="$HOME/.dots/bin:$PATH"' >> ~/.zshrc
bash setup.sh
# Confirm still only one occurrence
grep -c 'dots/bin' ~/.zshrc | grep '^1$'
```

**Pass 3 — full fresh install simulation:**
Rename `~/.dots` to `~/.dots.bak`. Run `bash setup.sh`. Confirm:
- `~/.dots/bin/dots` exists and is executable.
- `dots --version` prints a version string.
- `dots health` shows green for core symlinks.
- Restore `~/.dots` from backup.

---

## Completion criteria

- [ ] `bash setup.sh` completes without errors on a clean system
- [ ] Running `setup.sh` twice does not duplicate PATH entries
- [ ] `dots health --fix` is called and succeeds
- [ ] `uninstall.sh` removes the binary and PATH export cleanly
- [ ] All three tests pass
