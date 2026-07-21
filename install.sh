#!/bin/sh
# dots installer — portable, POSIX sh. Safe to run via:
#   curl -fsSL https://raw.githubusercontent.com/CtrlUserKnown/dots/main/install.sh | sh
#
# Installs the dots repo to ~/.dots, puts the `dots` binary on your PATH
# (regardless of which shell you use), and wires up your dotfiles.
set -eu

# ── configuration ─────────────────────────────────────────────────────────────

OWNER="CtrlUserKnown"
REPO="dots"
REPO_URL="https://github.com/${OWNER}/${REPO}"
DOTS_DIR="${DOTS_DIR:-$HOME/.dots}"
BIN_DIR="$DOTS_DIR/bin"
DOTS_BIN="$BIN_DIR/dots"
VERSION=""   # optional pin, e.g. --version v1.6.0

while [ $# -gt 0 ]; do
    case "$1" in
        --version) VERSION="${2:?--version needs a value}"; shift 2 ;;
        --dir)     DOTS_DIR="${2:?--dir needs a value}"; BIN_DIR="$DOTS_DIR/bin"; DOTS_BIN="$BIN_DIR/dots"; shift 2 ;;
        -h|--help)
            printf 'usage: install.sh [--version <tag>] [--dir <path>]\n'
            exit 0 ;;
        *) printf 'unknown option: %s\n' "$1" >&2; exit 1 ;;
    esac
done

# ── output helpers ────────────────────────────────────────────────────────────

if [ -t 1 ]; then B=$(printf '\033[1m'); D=$(printf '\033[2m'); R=$(printf '\033[0m'); else B=; D=; R=; fi
info() { printf '  %s→%s %s\n' "$D" "$R" "$*"; }
ok()   { printf '  ✓ %s\n' "$*"; }
warn() { printf '  ⚠ %s\n' "$*" >&2; }
die()  { printf '\n%sError:%s %s\n' "$B" "$R" "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

# ── platform detection ────────────────────────────────────────────────────────

detect_platform() {
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)
    case "$os" in
        linux)  OS=linux  ;;
        darwin) OS=darwin ;;
        *) die "unsupported OS '$os' — build from source with cargo (https://rustup.rs)" ;;
    esac
    case "$arch" in
        x86_64|amd64)  ARCH=x86_64  ;;
        arm64|aarch64) ARCH=aarch64 ;;
        *) die "unsupported architecture '$arch' — build from source with cargo (https://rustup.rs)" ;;
    esac
}

# ── step 1: clone or update the repo ──────────────────────────────────────────

setup_repo() {
    have git || die "git is required. Install git and re-run."
    if [ -d "$DOTS_DIR/.git" ]; then
        info "Updating $DOTS_DIR…"
        git -C "$DOTS_DIR" fetch --quiet origin || die "git fetch failed"
        git -C "$DOTS_DIR" pull --ff-only --quiet ||
            die "'git pull --ff-only' failed — you have local changes in $DOTS_DIR. Stash or commit them, then re-run."
    else
        info "Cloning $REPO…"
        git clone --quiet "$REPO_URL" "$DOTS_DIR" || die "git clone failed"
    fi
    ok "Repository ready"
}

# ── step 2: obtain the binary (download prebuilt, else build) ─────────────────

resolve_version() {
    [ -n "$VERSION" ] && return 0
    VERSION=$(git -C "$DOTS_DIR" describe --tags --abbrev=0 2>/dev/null || echo "")
}

download_binary() {
    [ -n "$VERSION" ] || return 1
    asset="dots-${VERSION}-${OS}-${ARCH}.tar.gz"
    url="${REPO_URL}/releases/download/${VERSION}/${asset}"
    mkdir -p "$BIN_DIR"
    info "Downloading $asset…"
    if have curl; then
        curl -fsSL "$url" | tar -xz -C "$BIN_DIR" 2>/dev/null || return 1
    elif have wget; then
        wget -qO- "$url" | tar -xz -C "$BIN_DIR" 2>/dev/null || return 1
    else
        return 1
    fi
    [ -x "$DOTS_BIN" ]
}

build_binary() {
    have cargo || return 1
    info "Building from source ($VERSION)…"
    cargo build --release --manifest-path "$DOTS_DIR/dots-rs/Cargo.toml" || return 1
    mkdir -p "$BIN_DIR"
    cp "$DOTS_DIR/dots-rs/target/release/dots" "$DOTS_BIN"
    [ -x "$DOTS_BIN" ]
}

setup_binary() {
    if [ -x "$DOTS_BIN" ] && "$DOTS_BIN" --version >/dev/null 2>&1; then
        ok "Binary already installed ($("$DOTS_BIN" --version))"
        return 0
    fi
    resolve_version
    if download_binary; then
        ok "Binary downloaded ($VERSION)"
    elif build_binary; then
        ok "Binary built from source"
    else
        die "could not obtain the dots binary.
    No prebuilt release for ${OS}-${ARCH}@${VERSION:-unknown}, and cargo is not installed.
    Install Rust (https://rustup.rs) and re-run, or file a bug at ${REPO_URL}/issues"
    fi
    chmod +x "$DOTS_BIN"
}

# ── step 3: put the binary on PATH for the user's actual shell ────────────────

append_line() {  # append_line <file> <line>
    file="$1"; line="$2"
    [ -f "$file" ] || : > "$file"
    if ! grep -qF "$BIN_DIR" "$file" 2>/dev/null; then
        printf '\n%s\n' "$line" >> "$file"
        ok "Added $BIN_DIR to PATH in ${file#"$HOME"/~}"
    else
        ok "PATH already configured in ${file#"$HOME"/~}"
    fi
}

configure_path() {
    case ":$PATH:" in *":$BIN_DIR:"*) NEEDS_SOURCE=0 ;; *) NEEDS_SOURCE=1 ;; esac
    shell_name=$(basename "${SHELL:-sh}")
    export_line="export PATH=\"$BIN_DIR:\$PATH\""
    case "$shell_name" in
        zsh)
            SHELL_RC="$HOME/.zshrc"
            append_line "$SHELL_RC" "$export_line" ;;
        bash)
            SHELL_RC="$HOME/.bashrc"
            append_line "$SHELL_RC" "$export_line"
            # macOS login shells read .bash_profile, not .bashrc
            [ "$OS" = darwin ] && [ -f "$HOME/.bash_profile" ] &&
                append_line "$HOME/.bash_profile" "$export_line" ;;
        fish)
            SHELL_RC="$HOME/.config/fish/config.fish"
            mkdir -p "$(dirname "$SHELL_RC")"
            append_line "$SHELL_RC" "fish_add_path $BIN_DIR" ;;
        *)
            SHELL_RC="$HOME/.profile"
            append_line "$SHELL_RC" "$export_line" ;;
    esac
}

# ── step 4: wire up dotfiles ──────────────────────────────────────────────────

setup_dotfiles() {
    info "Creating symlinks…"
    if "$DOTS_BIN" health >/dev/null 2>&1; then
        ok "Symlinks ready"
    else
        warn "Some symlinks need attention — run 'dots health' after install"
    fi
    info "Initializing config…"
    if "$DOTS_BIN" init --quiet 2>/dev/null; then
        ok "Config initialized"
    else
        warn "Config init skipped — run 'dots init' after install"
    fi
}

# ── run ───────────────────────────────────────────────────────────────────────

printf '\n%sInstalling dots%s\n\n' "$B" "$R"
detect_platform
setup_repo
setup_binary
configure_path
setup_dotfiles

printf '\n  ✓ %sdots installed%s — %s\n\n' "$B" "$R" "$("$DOTS_BIN" --version 2>/dev/null || echo "$VERSION")"
if [ "${NEEDS_SOURCE:-1}" -eq 1 ]; then
    printf '  Restart your shell or run:\n    %ssource %s%s\n\n' "$B" "$SHELL_RC" "$R"
fi
printf '  Then run %sdots%s to get started.\n\n' "$B" "$R"
