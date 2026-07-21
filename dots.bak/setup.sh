#!/usr/bin/env bash
set -euo pipefail

# ── dots setup script ─────────────────────────────────────────────────────────

DOTS_DIR="$HOME/.dots"
DOTS_REPO="https://github.com/CtrlUserKnown/dots"
DOTS_BIN="$DOTS_DIR/bin/dots"
DOTS_BRANCH="main"

# Parse flags
while [[ $# -gt 0 ]]; do
    case $1 in
        --branch)
            DOTS_BRANCH="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

# ── helpers ───────────────────────────────────────────────────────────────────

info()  { printf "  → %s\n" "$*"; }
ok()    { printf "  ✓ %s\n" "$*"; }
warn()  { printf "  ⚠ %s\n" "$*" >&2; }
die()   { printf "\nError: %s\n" "$*" >&2; exit 1; }

_download_binary() {
    local version="$1"
    local os arch asset url

    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$os" in
        darwin) os="darwin" ;;
        linux)  os="linux"  ;;
        *)      die "Unsupported OS '$os'. Build from source: cargo build --release" ;;
    esac
    case "$arch" in
        x86_64)          arch="x86_64"  ;;
        arm64|aarch64)   arch="aarch64" ;;
        *)               die "Unsupported arch '$arch'. Build from source: cargo build --release" ;;
    esac

    asset="dots-${version}-${os}-${arch}.tar.gz"
    url="https://github.com/CtrlUserKnown/dots/releases/download/${version}/${asset}"

    info "Downloading $asset…"
    mkdir -p "$DOTS_DIR/bin"

    if command -v curl >/dev/null 2>&1; then
        if ! curl -fsSL "$url" | tar -xz -C "$DOTS_DIR/bin"; then
            die "Download failed. Install Rust (https://rustup.rs) and run: cargo build --manifest-path $DOTS_DIR/Cargo.toml --release"
        fi
    elif command -v wget >/dev/null 2>&1; then
        if ! wget -q -O - "$url" | tar -xz -C "$DOTS_DIR/bin"; then
            die "Download failed. Install Rust (https://rustup.rs) and run: cargo build --manifest-path $DOTS_DIR/Cargo.toml --release"
        fi
    else
        die "Neither curl nor wget found. Install one and retry."
    fi
}

# ── step 1: clone or update ───────────────────────────────────────────────────

if ! command -v git >/dev/null 2>&1; then
    die "git is required but not found. Install git and retry."
fi

if [ -d "$DOTS_DIR/.git" ]; then
    info "Updating ~/.dots…"
    if ! git -C "$DOTS_DIR" pull --ff-only 2>/dev/null; then
        die "git pull --ff-only failed — local changes in ~/.dots. Stash or commit them first."
    fi
else
    info "Cloning dots…"
    git clone --branch "$DOTS_BRANCH" "$DOTS_REPO" "$DOTS_DIR"
fi
ok "Repository ready"

# ── step 2: build or download binary ─────────────────────────────────────────

DOTS_VERSION=$(git -C "$DOTS_DIR" describe --tags --abbrev=0 2>/dev/null || echo "dev")

if [ -x "$DOTS_BIN" ] && "$DOTS_BIN" --version &>/dev/null; then
    ok "Binary ready ($("$DOTS_BIN" --version))"
elif command -v cargo >/dev/null 2>&1; then
    info "Building dots from source ($DOTS_VERSION)…"
    if ! cargo build --manifest-path "$DOTS_DIR/Cargo.toml" --release; then
        die "cargo build failed. Please file a bug at https://github.com/CtrlUserKnown/dots/issues"
    fi
    mkdir -p "$DOTS_DIR/bin"
    cp "$DOTS_DIR/target/release/dots" "$DOTS_BIN"
    ok "Binary built and installed"
else
    _download_binary "$DOTS_VERSION"
    ok "Binary downloaded"
fi

chmod +x "$DOTS_BIN"

# ── step 3: add binary to PATH ────────────────────────────────────────────────

ZSHRC="$HOME/.zshrc"
PATH_LINE='export PATH="$HOME/.dots/bin:$PATH"'

if [ -f "$ZSHRC" ]; then
    if ! grep -q 'dots/bin' "$ZSHRC" 2>/dev/null; then
        printf "\n%s\n" "$PATH_LINE" >> "$ZSHRC"
        ok "Added ~/.dots/bin to PATH in ~/.zshrc"
    else
        ok "PATH already configured in ~/.zshrc"
    fi
else
    printf "%s\n" "$PATH_LINE" > "$ZSHRC"
    ok "Created ~/.zshrc with PATH export"
fi

# ── step 4: create symlinks ───────────────────────────────────────────────────

info "Checking symlinks…"
if "$DOTS_BIN" health; then
    ok "Symlinks ready"
else
    warn "Some symlinks could not be created — run 'dots health' to investigate"
fi

# ── step 5: initialize config ─────────────────────────────────────────────────

info "Initializing config…"
"$DOTS_BIN" init --quiet
ok "Config initialized"

# ── step 6: done ─────────────────────────────────────────────────────────────

printf "\n  ✓ dots installed — version %s\n\n" "$DOTS_VERSION"
printf "  restart your shell or run:\n    source ~/.zshrc\n\n"
printf "  then type 'dots' to get started\n\n"
