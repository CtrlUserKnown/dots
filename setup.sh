#!/usr/bin/env bash

# --- dotfiles:macOS install script ---
# dotfiles v1.3.0
# date created: 08.29.2025

# --- charfile:start ---

# --- cross-platform timeout wrapper ---
# timeout is not available on macOS by default
run_timeout() {
    local duration="$1"
    shift
    if command -v timeout &> /dev/null; then
        timeout "$duration" "$@"
    else
        "$@"
    fi
}

# --- script prompting:gum ---
# ensure gum is installed (silent)
install_gum() {
    if ! command -v gum &> /dev/null; then
        echo "Installing gum..."

        # Use Homebrew to install gum if available, otherwise manual install
        if command -v brew &> /dev/null; then
            run_timeout 30s brew install gum >/dev/null 2>&1
            return $?
        fi

        # Manual installation for CI/CD environments
        ARCH=$(uname -m)
        OS="Darwin"

        # Map architecture names
        case "$ARCH" in
            x86_64) ARCH="x86_64" ;;
            arm64) ARCH="arm64" ;;
            *) echo "Unsupported architecture: $ARCH"; return 1 ;;
        esac

        tmpdir=$(mktemp -d)

        # Download and extract gum
        if run_timeout 30s curl -sSL "https://github.com/charmbracelet/gum/releases/latest/download/gum_${OS}_${ARCH}.tar.gz" \
            | tar -xz -C "$tmpdir" 2>/dev/null; then
            sudo mv "$tmpdir/gum" /usr/local/bin/gum 2>/dev/null || {
                # Fallback if sudo fails (CI environment)
                mkdir -p "$HOME/.local/bin"
                mv "$tmpdir/gum" "$HOME/.local/bin/gum"
                export PATH="$HOME/.local/bin:$PATH"
            }
            rm -rf "$tmpdir"
            echo "✅ Gum installed successfully"
            return 0
        else
            echo "⚠️ Failed to install gum, continuing without fancy output..."
            rm -rf "$tmpdir"
            return 1
        fi
    fi
}

# Helper function to show messages (works with or without gum)
show_message() {
    if command -v gum &> /dev/null; then
        gum style --foreground 141 "$1"
    else
        echo "$1"
    fi
}

# Helper function for spinners (works with or without gum)
run_with_spinner() {
    local title="$1"
    shift

    if command -v gum &> /dev/null; then
        gum spin --spinner dot --title "$title" -- "$@"
    else
        echo "$title"
        "$@"
    fi
}

# install gum if missing (but continue if it fails)
install_gum

# Check macOS version
check_os_version() {
    if [[ "$(uname -s)" == "Darwin" ]]; then
        OS_VERSION=$(sw_vers -productVersion)
        MAJOR_VERSION=$(echo "$OS_VERSION" | cut -d. -f1)

        if [ "$MAJOR_VERSION" -lt 12 ]; then
            echo "⚠️ Warning: You are running macOS $OS_VERSION."
            echo "This setup is optimized for macOS 12.0 (Monterey) and newer."
            echo "Some tools, like Ghostty, may not be compatible with your system."

            if [ -z "$CI" ]; then
                if command -v gum &> /dev/null; then
                    gum confirm "Do you want to continue anyway?" || exit 0
                else
                    read -p "Do you want to continue anyway? (y/n) " -n 1 -r
                    echo
                    [[ ! $REPLY =~ ^[Yy]$ ]] && exit 0
                fi
            fi
        fi
    fi
}

check_os_version

# --- dependency installer ---
DEPENDENCY_TIMEOUT=120

# Check if a brew formula is already installed
brew_installed() {
    brew list "$1" &>/dev/null
}

# Check if a brew cask is already installed
cask_installed() {
    brew list --cask "$1" &>/dev/null
}

# Check if a go package is already installed (checks GOBIN or GOPATH/bin)
go_installed() {
    local pkg="$1"
    local bin_name
    bin_name=$(basename "$pkg")
    command -v "$bin_name" &>/dev/null
}

# Check if a cargo package is already installed
cargo_installed() {
    local pkg="$1"
    local bin_name
    bin_name=$(basename "$pkg")
    command -v "$bin_name" &>/dev/null
}

# Install a single brew entry safely
install_brew_entry() {
    local line="$1"

    # Tap
    if [[ "$line" =~ ^tap\ \"(.*)\"$ ]]; then
        local tap="${BASH_REMATCH[1]}"
        if brew tap | grep -q "^${tap}$"; then
            echo "✅ Tap $tap already added"
        else
            run_with_spinner "Tapping $tap..." brew tap "$tap"
        fi

    # Brew formula
    elif [[ "$line" =~ ^brew\ \"(.*)\"$ ]]; then
        local pkg="${BASH_REMATCH[1]}"

        # Skip neovim if user declined
        if [ "$pkg" = "neovim" ] && [ "$INSTALL_NEOVIM" = false ]; then
            echo "⏭️ Skipping neovim (declined by user)"
            return 0
        fi

        if brew_installed "$pkg"; then
            echo "✅ $pkg already installed"
        else
            run_with_spinner "Installing $pkg..." \
                run_timeout "$DEPENDENCY_TIMEOUT" brew install "$pkg" || \
                echo "⚠️ $pkg failed to install (timeout or error)"
        fi

    # Cask
    elif [[ "$line" =~ ^cask\ \"(.*)\"$ ]]; then
        local pkg="${BASH_REMATCH[1]}"
        if cask_installed "$pkg"; then
            echo "✅ $pkg (cask) already installed"
        else
            run_with_spinner "Installing $pkg (cask)..." \
                run_timeout "$DEPENDENCY_TIMEOUT" brew install --cask "$pkg" || \
                echo "⚠️ $pkg (cask) failed to install (timeout or error)"
        fi

    # Go package
    elif [[ "$line" =~ ^go\ \"(.*)\"$ ]]; then
        local pkg="${BASH_REMATCH[1]}"
        if go_installed "$pkg"; then
            echo "✅ $(basename "$pkg") (go) already installed"
        else
            run_with_spinner "Installing $pkg (go)..." \
                go install "$pkg@latest" || \
                echo "⚠️ $pkg (go) failed to install"
        fi

    # Cargo package
    elif [[ "$line" =~ ^cargo\ \"(.*)\"$ ]]; then
        local pkg="${BASH_REMATCH[1]}"
        local bin_name
        bin_name=$(basename "$pkg")
        if cargo_installed "$pkg"; then
            echo "✅ $bin_name (cargo) already installed"
        else
            run_with_spinner "Installing $bin_name (cargo)..." \
                run_timeout "$DEPENDENCY_TIMEOUT" cargo install "$bin_name" || \
                echo "⚠️ $bin_name (cargo) failed to install"
        fi
    fi
}

# Install all brewfile entries safely, each with its own timeout
install_brewfile() {
    local brewfile="$1"

    if [ ! -f "$brewfile" ]; then
        echo "⚠️ Brewfile not found at $brewfile. Skipping."
        return 1
    fi

    echo "🔧 Installing packages from Brewfile..."

    while IFS= read -r line || [ -n "$line" ]; do
        # Skip empty lines and comments
        [[ -z "$line" || "$line" == \#* ]] && continue
        install_brew_entry "$line"
    done < "$brewfile"

    echo "✅ Brewfile package installation complete."
}

# Install fzf-tab from GitHub if not present
install_fzf_tab() {
    local target="$HOME/.config/zsh/plugins/fzf-tab"
    if [ -d "$target" ]; then
        echo "✅ fzf-tab already installed"
    else
        echo "🔧 Installing fzf-tab from GitHub..."
        mkdir -p "$(dirname "$target")"
        if run_timeout 30s git clone --depth 1 https://github.com/Aloxaf/fzf-tab.git "$target"; then
            echo "✅ fzf-tab installed"
        else
            echo "⚠️ fzf-tab failed to install"
        fi
    fi
}

# Install npm dependencies for opencode if needed
install_opencode_deps() {
    local dir="$HOME/.dots/src/opencode"
    if [ -f "$dir/package.json" ] && [ ! -d "$dir/node_modules" ]; then
        echo "🔧 Installing opencode dependencies..."
        if command -v npm &>/dev/null; then
            run_with_spinner "Installing opencode npm packages..." \
                run_timeout 60s npm install --prefix "$dir" || \
                echo "⚠️ opencode npm install failed"
        else
            echo "⚠️ npm not found, skipping opencode dependencies"
        fi
    elif [ -d "$dir/node_modules" ]; then
        echo "✅ opencode dependencies already installed"
    fi
}

# Re-link configs and verify
relink_and_verify() {
    local dots_dir="$1"

    echo "🔧 Creating links for configuration files..."
    ln -sf "$dots_dir/src/bat" "$HOME/.config/bat"
    ln -sf "$dots_dir/src/fastfetch" "$HOME/.config/fastfetch"
    ln -sf "$dots_dir/src/ghostty" "$HOME/.config/ghostty"
    ln -sf "$dots_dir/src/zsh/zsh" "$HOME/.config/zsh"
    ln -sf "$dots_dir/src/zsh/.zshrc" "$HOME/.zshrc"
    if [ "$INSTALL_NEOVIM_CONFIG" = true ] && [ -d "$dots_dir/src/nvim" ]; then
        ln -sf "$dots_dir/src/nvim" "$HOME/.config/nvim"
    fi

    # verify
    local all_good=true
    for dir in bat fastfetch ghostty zsh; do
        if [ ! -L "$HOME/.config/$dir" ]; then
            echo "⚠️ Missing symlink: $HOME/.config/$dir"
            all_good=false
        fi
    done

    if [ ! -L "$HOME/.zshrc" ]; then
        echo "⚠️ Missing symlink: $HOME/.zshrc"
        all_good=false
    fi

    if [ ! -d "$dots_dir/.git" ]; then
        echo "⚠️ Dotfiles repository not properly cloned"
        all_good=false
    fi

    if [ "$all_good" = true ]; then
        echo "✅ All configuration files verified successfully"
    else
        echo "⚠️ Some files are missing or not properly linked"
        exit 1
    fi
}

# --- install ---
# main installation sequence
do_install() {
    # --- script:Banner ---
    if command -v gum &> /dev/null; then
        gum style \
          --border thick \
          --border-foreground 105 \
          --foreground 141 \
          --align center \
          --padding "1 2" << 'EOF'
          ______     __                __  __    __  __    __
         /      \   |  \              |  \|  \  |  \|  \  /  \
        |  $$$$$$\ _| $$_     ______  | $$| $$  | $$| $$ /  $$
        | $$   \$$|   $$ \   /      \ | $$| $$  | $$| $$/  $$
        | $$       \$$$$$$  |  $$$$$$\| $$| $$  | $$| $$  $$
        | $$   __   | $$ __ | $$   \$$| $$| $$  | $$| $$$$$\
        | $$__/  \  | $$|  \| $$      | $$| $$__/ $$| $$ \$$\
         \$$    $$   \$$  $$| $$      | $$ \$$    $$| $$  \$$\
          \$$$$$$     \$$$$  \$$       \$$  \$$$$$$  \$$   \$$
EOF
    else
        echo "================================"
        echo "    Ctrlk Dotfiles Installer    "
        echo "================================"
        echo ""
    fi

    # --- package manager:homebrew ---
    if command -v brew &> /dev/null; then
        echo "☕️ Homebrew is already installed."
    else
        echo "🚀 Homebrew not found. Installing..."

        if [ -n "$CI" ]; then
            NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
        else
            /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
        fi

        if [[ -d "/opt/homebrew/bin" ]]; then
            # shellcheck disable=SC2016
            echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
            eval "$(/opt/homebrew/bin/brew shellenv)"
        elif [[ -d "/usr/local/bin/brew" ]]; then
            # shellcheck disable=SC2016
            echo 'eval "$(/usr/local/bin/brew shellenv)"' >> ~/.zprofile
            eval "$(/usr/local/bin/brew shellenv)"
        fi

        echo "✅ Homebrew installation complete."

        if ! command -v gum &> /dev/null; then
            brew install gum >/dev/null 2>&1 || echo "⚠️ Could not install gum via Homebrew"
        fi
    fi

    # --- configuration:.config ---
    echo "🔧 Checking for configuration files..."

    if [ ! -d ~/.config ]; then
        run_with_spinner "Creating Config folder..." mkdir ~/.config
        sleep 1
    else
        echo "✅ Config directory already exists!"
    fi

    # --- configuration:Dotfiles ---
    if [ ! -d ~/.dots ]; then
        echo "🔧 Setting up Dotfiles (~/.dots)..."

        if ! run_timeout 30s git clone https://github.com/CtrlUserKnown/dotfiles.git ~/.dots; then
            echo "⚠️  Standard clone timed out or failed. Trying a shallow clone..."
            rm -rf ~/.dots

            if ! run_timeout 20s git clone --depth 1 https://github.com/CtrlUserKnown/dotfiles.git ~/.dots; then
                echo "❌  Both attempts failed. Skipping dotfiles for now."
            else
                echo "✅  Shallow clone successful!"
            fi
        else
            echo "✅  Dotfiles cloned successfully!"
        fi
        sleep 1
    else
        echo "Dotfiles directory has already been created ✅"
    fi

    # --- packages:neovim prompt ---
    # Ask user if they want Neovim and/or Neovim config
    INSTALL_NEOVIM=true
    INSTALL_NEOVIM_CONFIG=true
    if [ -z "$CI" ]; then
        if command -v gum &> /dev/null; then
            gum confirm "Install Neovim?" --default=true || INSTALL_NEOVIM=false
            if [ "$INSTALL_NEOVIM" = true ]; then
                if [ -d "$(dirname "$0")/src/nvim" ]; then
                    gum confirm "Link Neovim config from dotfiles?" --default=true || INSTALL_NEOVIM_CONFIG=false
                fi
            fi
        else
            read -p "Install Neovim? (Y/n) " -n 1 -r
            echo
            [[ ! $REPLY =~ ^[Yy]$ ]] && INSTALL_NEOVIM=false
            if [ "$INSTALL_NEOVIM" = true ] && [ -d "$(dirname "$0")/src/nvim" ]; then
                read -p "Link Neovim config from dotfiles? (Y/n) " -n 1 -r
                echo
                [[ ! $REPLY =~ ^[Yy]$ ]] && INSTALL_NEOVIM_CONFIG=false
            fi
        fi
    fi

    # --- packages:Brewfile ---
    install_brewfile "./assets/Brewfile"

    # --- packages:GitHub ---
    install_fzf_tab

    # --- packages:npm ---
    install_opencode_deps

    # --- configuration:Edits ---
    rm -f ~/.zprofile

    # --- configuration:Links & Verify ---
    relink_and_verify "$HOME/.dots"
}

do_install

# --- Charfiles:finish ---
if command -v gum &> /dev/null; then
    gum style --foreground 200 --border normal --padding "0.5" --margin "0.5" <<EOF
🎉 Installation Complete! 🎉

Your macOS environment has been set up with Homebrew, Git, and your configuration files.
You can now start customizing your setup further!

Thank you for using my dotfiles!
EOF
else
    echo ""
    echo "================================"
    echo "🎉 Installation Complete! 🎉"
    echo "================================"
    echo ""
    echo "Your macOS environment has been set up with Homebrew, Git, and your configuration files."
    echo "You can now start customizing your setup further!"
    echo ""
    echo "Thank you for using my dotfiles!"
    echo ""
fi
