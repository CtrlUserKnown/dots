#!/bin/sh
# dots installer — portable, POSIX sh. Safe to run via:
#   curl -fsSL https://raw.githubusercontent.com/CtrlUserKnown/dots/main/install.sh | sh
#
# Downloads a prebuilt `dots` binary for your OS/arch straight from GitHub
# Releases, puts it on your PATH (regardless of which shell you use), and
# wires up your dotfiles. Never clones this tool's own repo — the only git
# repo you need is your own personal dotfiles repo, which `dots` manages
# separately (see `dots get-config`).
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

fetch() {  # fetch <url>  — prints body to stdout
    url="$1"
    if have curl; then
        curl -fsSL "$url"
    elif have wget; then
        wget -qO- "$url"
    else
        return 1
    fi
}

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

# ── step 1: clean up a legacy full-repo clone, if this dir has one ────────────

# Installs before this rework did `git clone` of this tool's own repo into
# $DOTS_DIR, so upgraders may have Cargo.toml/crates/.git/etc. sitting next to
# their real config (settings.toml, links.toml, ...). Remove exactly the
# known tool-repo artifacts by allowlist; everything else is left alone.
migrate_legacy_clone() {
    [ -d "$DOTS_DIR/.git" ] || return 0
    have git || return 0

    origin=$(git -C "$DOTS_DIR" remote get-url origin 2>/dev/null || echo "")
    # Exact match only — a substring match would also fire for a user's own
    # fork of this repo (e.g. kept as their personal dotfiles remote), which
    # would then have its .git/Cargo.toml/crates/ deleted as "legacy" files.
    slug="${OWNER}/${REPO}"
    case "$origin" in
        "https://github.com/${slug}"|"https://github.com/${slug}.git"|"https://github.com/${slug}/") ;;
        "git@github.com:${slug}"|"git@github.com:${slug}.git") ;;
        *) return 0 ;;
    esac

    info "Found a legacy full-repo clone at $DOTS_DIR — cleaning up tool-repo files…"
    removed=""
    for item in .git Cargo.toml Cargo.lock crates .github site target \
                README.md CHANGELOG.md man tests examples docs img test-env \
                uninstall.sh install.sh BUILD_MACOS.md; do
        path="$DOTS_DIR/$item"
        if [ -e "$path" ]; then
            rm -rf "$path"
            removed="$removed $item"
        fi
    done
    if [ -n "$removed" ]; then
        ok "Removed:$removed"
    fi
    ok "Your config (settings.toml, links.toml, plugins/, src/, ...) was left untouched"
}

# ── step 2: obtain the binary (download prebuilt, else build) ─────────────────

resolve_version() {
    [ -n "$VERSION" ] && return 0
    info "Resolving latest release…"
    json=$(fetch "https://api.github.com/repos/${OWNER}/${REPO}/releases/latest" 2>/dev/null || echo "")
    VERSION=$(printf '%s\n' "$json" | grep -m1 '"tag_name"' | sed -E 's/.*"tag_name" *: *"([^"]+)".*/\1/')
}

# verify_checksum <file> <asset-name> — fetches <asset-name>.sha256 (published
# alongside every release asset by release.yml) and checks it against <file>.
# Fails closed: no sha256/shasum tool, or no sidecar reachable, is a failure,
# not a silent skip — otherwise blocking the sidecar fetch would defeat it.
verify_checksum() {
    file="$1"; asset="$2"
    if have sha256sum; then hasher() { sha256sum "$1" | awk '{print $1}'; }
    elif have shasum;  then hasher() { shasum -a 256 "$1" | awk '{print $1}'; }
    else warn "no sha256sum/shasum found — cannot verify download integrity"; return 1
    fi
    sidecar=$(fetch "${REPO_URL}/releases/download/${VERSION}/${asset}.sha256" 2>/dev/null) || return 1
    expected=$(printf '%s\n' "$sidecar" | awk '{print $1}')
    [ -n "$expected" ] || return 1
    actual=$(hasher "$file")
    if [ "$expected" != "$actual" ]; then
        warn "checksum mismatch for ${asset} (expected ${expected}, got ${actual})"
        return 1
    fi
}

download_binary() {
    [ -n "$VERSION" ] || return 1
    asset="dots-${VERSION}-${OS}-${ARCH}.tar.gz"
    url="${REPO_URL}/releases/download/${VERSION}/${asset}"
    mkdir -p "$BIN_DIR"
    info "Downloading ${asset}…"
    tmp_tar=$(mktemp) || return 1
    fetch "$url" > "$tmp_tar" 2>/dev/null || { rm -f "$tmp_tar"; return 1; }
    if ! verify_checksum "$tmp_tar" "$asset"; then
        rm -f "$tmp_tar"
        return 1
    fi
    tar -xzf "$tmp_tar" -C "$BIN_DIR" 2>/dev/null
    rm -f "$tmp_tar"
    [ -x "$DOTS_BIN" ]
}

# Fallback for platforms/tags with no prebuilt asset: pull the tagged source
# tarball straight from GitHub (no git needed) and build it in a scratch dir.
build_binary() {
    have cargo || return 1
    [ -n "$VERSION" ] || return 1
    info "Building from source ($VERSION)…"

    tmp_src=$(mktemp -d) || return 1
    src_url="https://github.com/${OWNER}/${REPO}/archive/refs/tags/${VERSION}.tar.gz"
    if ! fetch "$src_url" 2>/dev/null | tar -xz -C "$tmp_src" --strip-components=1 2>/dev/null; then
        rm -rf "$tmp_src"
        return 1
    fi

    if ! ( cd "$tmp_src" && cargo build --release --manifest-path Cargo.toml ); then
        rm -rf "$tmp_src"
        return 1
    fi

    mkdir -p "$BIN_DIR"
    cp "$tmp_src/target/release/dots" "$DOTS_BIN" || { rm -rf "$tmp_src"; return 1; }
    rm -rf "$tmp_src"
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
            # macOS login shells read .bash_profile, not .bashrc. Guarded by
            # an explicit `if` (not a bare `&&` chain) — under `set -e`, a
            # bare chain as the last statement in a `case` branch aborts the
            # whole script the moment either test is false, even though
            # that's a routine "skip this" outcome, not a real error.
            if [ "$OS" = darwin ] && [ -f "$HOME/.bash_profile" ]; then
                append_line "$HOME/.bash_profile" "$export_line"
            fi ;;
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
have curl || have wget || die "curl or wget is required. Install one and re-run."
detect_platform
migrate_legacy_clone
setup_binary
configure_path
setup_dotfiles

printf '\n  ✓ %sdots installed%s — %s\n\n' "$B" "$R" "$("$DOTS_BIN" --version 2>/dev/null || echo "$VERSION")"
if [ "${NEEDS_SOURCE:-1}" -eq 1 ]; then
    printf '  Restart your shell or run:\n    %ssource %s%s\n\n' "$B" "$SHELL_RC" "$R"
fi
printf '  Then run %sdots%s to get started.\n\n' "$B" "$R"
