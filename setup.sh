#!/usr/bin/env bash

# --- dotfiles install script ---
# dotfiles v1.3.0
# date created: 08.29.2025

# --- charfile:start ---

# --- os detection ---
CURRENT_OS=$(uname -s)

is_fedora() {
    [[ -f /etc/os-release ]] && grep -q '^ID=fedora' /etc/os-release
}

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
        OS="$CURRENT_OS"

        # Map architecture names (Linux uses aarch64, releases use arm64)
        case "$ARCH" in
            x86_64) ARCH="x86_64" ;;
            arm64|aarch64) ARCH="arm64" ;;
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

# Check macOS version (skipped on Linux)
check_os_version() {
    [[ "$CURRENT_OS" != "Darwin" ]] && return

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
}

check_os_version

# --- dependency installer ---
DEPENDENCY_TIMEOUT=120

# --- macOS / Homebrew helpers ---

brew_installed() {
    brew list "$1" &>/dev/null
}

cask_installed() {
    brew list --cask "$1" &>/dev/null
}

# --- shared helpers (go / cargo / npm) ---

go_installed() {
    local pkg="$1"
    local bin_name
    bin_name=$(basename "$pkg")
    command -v "$bin_name" &>/dev/null
}

cargo_installed() {
    local pkg="$1"
    local bin_name
    bin_name=$(basename "$pkg")
    command -v "$bin_name" &>/dev/null
}

npm_installed() {
    npm list -g "$1" &>/dev/null 2>&1 || command -v "$1" &>/dev/null
}

install_go_pkg() {
    local pkg="$1"
    if go_installed "$pkg"; then
        echo "✅ $(basename "$pkg") (go) already installed"
    else
        run_with_spinner "Installing $pkg (go)..." \
            go install "$pkg@latest" || \
            echo "⚠️ $pkg (go) failed to install"
    fi
}

install_cargo_pkg() {
    local pkg="$1"
    local bin_name
    bin_name=$(basename "$pkg")
    if cargo_installed "$pkg"; then
        echo "✅ $bin_name (cargo) already installed"
    else
        run_with_spinner "Installing $bin_name (cargo)..." \
            run_timeout "$DEPENDENCY_TIMEOUT" cargo install "$bin_name" || \
            echo "⚠️ $bin_name (cargo) failed to install"
    fi
}

install_npm_pkg() {
    local pkg="$1"
    if npm_installed "$pkg"; then
        echo "✅ $pkg (npm) already installed"
    else
        run_with_spinner "Installing $pkg (npm)..." \
            npm install -g "$pkg" || \
            echo "⚠️ $pkg (npm) failed to install"
    fi
}

# --- Homebrew package installer ---

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
        install_go_pkg "${BASH_REMATCH[1]}"

    # Cargo package
    elif [[ "$line" =~ ^cargo\ \"(.*)\"$ ]]; then
        install_cargo_pkg "${BASH_REMATCH[1]}"

    # npm package
    elif [[ "$line" =~ ^npm\ \"(.*)\"$ ]]; then
        install_npm_pkg "${BASH_REMATCH[1]}"
    fi
}

install_brewfile() {
    local brewfile="$1"

    if [ ! -f "$brewfile" ]; then
        echo "⚠️ Brewfile not found at $brewfile. Skipping."
        return 1
    fi

    echo "🔧 Installing packages from Brewfile..."

    while IFS= read -r line || [ -n "$line" ]; do
        [[ -z "$line" || "$line" == \#* ]] && continue
        install_brew_entry "$line"
    done < "$brewfile"

    echo "✅ Brewfile package installation complete."
}

# --- Fedora / dnf package installer ---

dnf_installed() {
    rpm -q "$1" &>/dev/null
}

enable_copr() {
    local repo="$1"
    if dnf copr list --enabled 2>/dev/null | grep -q "$(echo "$repo" | cut -d/ -f2)"; then
        echo "✅ Copr $repo already enabled"
    else
        run_with_spinner "Enabling Copr $repo..." \
            sudo dnf copr enable -y "$repo" 2>/dev/null || \
            echo "⚠️ Failed to enable Copr $repo"
    fi
}

install_dnf_entry() {
    local line="$1"

    if [[ "$line" =~ ^copr\ \"(.*)\"$ ]]; then
        enable_copr "${BASH_REMATCH[1]}"

    elif [[ "$line" =~ ^dnf\ \"(.*)\"$ ]]; then
        local pkg="${BASH_REMATCH[1]}"
        if [ "$pkg" = "neovim" ] && [ "$INSTALL_NEOVIM" = false ]; then
            echo "⏭️ Skipping neovim (declined by user)"
            return 0
        fi
        if dnf_installed "$pkg"; then
            echo "✅ $pkg already installed"
        else
            run_with_spinner "Installing $pkg..." \
                run_timeout "$DEPENDENCY_TIMEOUT" sudo dnf install -y "$pkg" || \
                echo "⚠️ $pkg failed to install"
        fi

    elif [[ "$line" =~ ^go\ \"(.*)\"$ ]]; then
        install_go_pkg "${BASH_REMATCH[1]}"

    elif [[ "$line" =~ ^cargo\ \"(.*)\"$ ]]; then
        install_cargo_pkg "${BASH_REMATCH[1]}"

    elif [[ "$line" =~ ^npm\ \"(.*)\"$ ]]; then
        install_npm_pkg "${BASH_REMATCH[1]}"
    fi
}

install_fedorafile() {
    local file="$1"

    if [ ! -f "$file" ]; then
        echo "⚠️ packages.fedora not found at $file. Skipping."
        return 1
    fi

    echo "🔧 Installing packages from packages.fedora..."

    while IFS= read -r line || [ -n "$line" ]; do
        [[ -z "$line" || "$line" == \#* ]] && continue
        install_dnf_entry "$line"
    done < "$file"

    echo "✅ Fedora package installation complete."
}

# Enable repos that need special handling before package install
setup_fedora_repos() {
    # dnf-plugins-core provides copr and config-manager subcommands
    if ! dnf_installed "dnf-plugins-core"; then
        run_with_spinner "Installing dnf-plugins-core..." \
            sudo dnf install -y dnf-plugins-core || \
            echo "⚠️ Could not install dnf-plugins-core"
    fi

    # GitHub CLI
    if ! rpm -q "gh" &>/dev/null && ! sudo dnf repolist | grep -q "gh-cli"; then
        run_with_spinner "Adding GitHub CLI repository..." \
            sudo dnf config-manager --add-repo https://cli.github.com/packages/rpm/gh-cli.repo 2>/dev/null || \
            echo "⚠️ Could not add GitHub CLI repo"
    fi

    # Docker CE
    if ! command -v docker &>/dev/null && ! sudo dnf repolist | grep -q "docker-ce"; then
        run_with_spinner "Adding Docker CE repository..." \
            sudo dnf config-manager --add-repo https://download.docker.com/linux/fedora/docker-ce.repo 2>/dev/null || \
            echo "⚠️ Could not add Docker CE repo"
        run_with_spinner "Installing Docker CE..." \
            sudo dnf install -y docker-ce docker-ce-cli containerd.io \
                docker-buildx-plugin docker-compose-plugin 2>/dev/null || \
            echo "⚠️ Docker CE install failed — install manually if needed"
    else
        echo "✅ Docker already installed"
    fi
}

# Install ollama via official script
install_ollama() {
    if command -v ollama &>/dev/null; then
        echo "✅ ollama already installed"
    else
        run_with_spinner "Installing ollama..." \
            run_timeout 60s bash -c "curl -fsSL https://ollama.com/install.sh | sh" || \
            echo "⚠️ ollama failed to install"
    fi
}

# Install yt-dlp via pip3
install_yt_dlp() {
    if command -v yt-dlp &>/dev/null; then
        echo "✅ yt-dlp already installed"
    else
        run_with_spinner "Installing yt-dlp..." \
            pip3 install --user yt-dlp || \
            echo "⚠️ yt-dlp failed to install"
    fi
}

# --- shared installers ---

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

# zsh-history-substring-search is not packaged for Fedora; clone it manually
install_zsh_history_substring_search() {
    local target="$HOME/.config/zsh/plugins/zsh-history-substring-search"
    if [ -d "$target" ]; then
        echo "✅ zsh-history-substring-search already installed"
    else
        echo "🔧 Installing zsh-history-substring-search from GitHub..."
        mkdir -p "$(dirname "$target")"
        if run_timeout 30s git clone --depth 1 \
            https://github.com/zsh-users/zsh-history-substring-search.git "$target"; then
            echo "✅ zsh-history-substring-search installed"
        else
            echo "⚠️ zsh-history-substring-search failed to install"
        fi
    fi
}

install_opencode_config_deps() {
    local dir="$HOME/.dots/src/opencode"
    if [ -f "$dir/package.json" ] && [ ! -d "$dir/node_modules" ]; then
        echo "🔧 Installing opencode config dependencies..."
        if command -v npm &>/dev/null; then
            run_with_spinner "Installing opencode npm packages..." \
                run_timeout 60s npm install --prefix "$dir" || \
                echo "⚠️ opencode npm install failed"
        else
            echo "⚠️ npm not found, skipping opencode dependencies"
        fi
    elif [ -d "$dir/node_modules" ]; then
        echo "✅ opencode config dependencies already installed"
    fi
}

install_opencode() {
    if command -v opencode &>/dev/null; then
        echo "✅ opencode already installed"
    else
        run_with_spinner "Installing opencode..." \
            run_timeout 60s bash -c "curl -fsSL https://opencode.ai/install | sh" || \
            echo "⚠️ opencode failed to install"
    fi
}

install_fedorafile_section() {
    local file="$1"
    local section="$2"

    if [ ! -f "$file" ]; then
        echo "⚠️ $file not found. Skipping."
        return 1
    fi

    local in_section=false
    while IFS= read -r line || [ -n "$line" ]; do
        if [[ "$line" =~ ^\#\ \[([a-z]+)\]$ ]]; then
            [[ "${BASH_REMATCH[1]}" == "$section" ]] && in_section=true || in_section=false
            continue
        fi
        [[ "$in_section" == false || -z "$line" || "$line" == \#* ]] && continue
        install_dnf_entry "$line"
    done < "$file"
}

relink_and_verify() {
    local dots_dir="$1"

    echo "🔧 Creating links for configuration files..."
    if python3 "$dots_dir/src/zsh/zsh/dots.py" --repair-symlinks; then
        echo "✅ All configuration files verified successfully"
    else
        echo "⚠️ Some symlinks could not be created (a real file may be in the way)"
        [ -z "$CI" ] && exit 1
    fi

    if [ "$INSTALL_NEOVIM_CONFIG" = true ] && [ -d "$dots_dir/src/nvim" ]; then
        ln -sf "$dots_dir/src/nvim" "$HOME/.config/nvim"
    fi

    if [ ! -d "$dots_dir/.git" ]; then
        echo "⚠️ Dotfiles repository not properly cloned"
        [ -z "$CI" ] && exit 1
    fi
}

install_charvim() {
    local charvim_dir="$HOME/.charvim"
    local charvim_repo="https://github.com/CrtlUserKnown/Charvim.git"
    local nvim_link="$HOME/.config/nvim"

    if [ -d "$charvim_dir/.git" ]; then
        run_with_spinner "Updating Charvim..." git -C "$charvim_dir" pull
        echo "✅ Charvim updated"
    else
        if run_timeout 30s git clone "$charvim_repo" "$charvim_dir" 2>/dev/null; then
            echo "✅ Charvim cloned to $charvim_dir"
        else
            echo "⚠️ Charvim clone timed out. Trying shallow clone..."
            rm -rf "$charvim_dir"
            if run_timeout 20s git clone --depth 1 "$charvim_repo" "$charvim_dir" 2>/dev/null; then
                echo "✅ Charvim shallow clone successful"
            else
                echo "❌ Charvim clone failed. Skipping."
                return 1
            fi
        fi
    fi

    if [ -e "$nvim_link" ] || [ -L "$nvim_link" ]; then
        local backup="${nvim_link}.bak.$(date +%Y%m%d_%H%M%S)"
        mv "$nvim_link" "$backup"
        echo "✅ Backed up existing nvim config to $backup"
    fi

    ln -sf "$charvim_dir/nvim" "$nvim_link"
    echo "✅ Linked: $charvim_dir/nvim → $nvim_link"
}

# --- setup:SSM ---
setup_ssm() {
    echo "🔧 Setting up SSM..."

    # Find or generate an SSH key
    local key_file=""
    if [ -f "$HOME/.ssh/id_ed25519" ]; then
        key_file="$HOME/.ssh/id_ed25519"
        echo "✅ Found existing SSH key: ~/.ssh/id_ed25519"
    elif [ -f "$HOME/.ssh/id_rsa" ]; then
        key_file="$HOME/.ssh/id_rsa"
        echo "✅ Found existing SSH key: ~/.ssh/id_rsa"
    fi

    if [ "$SETUP_SSH_KEY" = true ]; then
        if [ -z "$key_file" ]; then
            echo "🔑 No SSH key found — generating ed25519 key..."
            mkdir -p "$HOME/.ssh"
            chmod 700 "$HOME/.ssh"
            ssh-keygen -t ed25519 -C "$(whoami)@$(hostname)" \
                -f "$HOME/.ssh/id_ed25519" -N "" && \
                echo "✅ SSH key generated at ~/.ssh/id_ed25519" || \
                echo "⚠️ ssh-keygen failed"
            key_file="$HOME/.ssh/id_ed25519"
        fi

        # Load key into agent (macOS: persist to Keychain)
        if [ -n "$key_file" ]; then
            if [[ "$CURRENT_OS" == "Darwin" ]]; then
                ssh-add --apple-use-keychain "$key_file" 2>/dev/null && \
                    echo "✅ SSH key added to macOS Keychain" || \
                    echo "⚠️ Could not add to Keychain (may already be present)"
            else
                ssh-add "$key_file" 2>/dev/null && \
                    echo "✅ SSH key added to agent" || \
                    echo "⚠️ Could not add key to agent (is ssh-agent running?)"
            fi
        fi

        # Optionally copy public key to a remote server
        local copy_key=false
        if command -v gum &> /dev/null; then
            gum confirm "  Copy your public key to a remote server now?" \
                --default=false && copy_key=true
        else
            read -p "  Copy your public key to a remote server now? (y/N) " -n 1 -r; echo
            [[ $REPLY =~ ^[Yy]$ ]] && copy_key=true
        fi

        if [ "$copy_key" = true ]; then
            local remote_host=""
            if command -v gum &> /dev/null; then
                remote_host=$(gum input --placeholder "user@host  (e.g. userknown@192.168.12.38)")
            else
                read -p "  Remote host (user@host): " remote_host
            fi
            if [ -n "$remote_host" ]; then
                echo "🔧 Copying public key to $remote_host..."
                ssh-copy-id "$remote_host" && \
                    echo "✅ Public key copied to $remote_host" || \
                    echo "⚠️ Could not copy key — run manually: ssh-copy-id $remote_host"
            fi
        fi
    fi

    echo "✅ SSM setup complete — run 'ssm' to manage your SSH sessions."
}

# --- install ---
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

    # --- configuration:.config ---
    echo "🔧 Checking for configuration files..."

    if [ ! -d ~/.config ]; then
        run_with_spinner "Creating Config folder..." mkdir ~/.config
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
    else
        echo "Dotfiles directory has already been created ✅"
    fi

    # --- packages:prompts ---
    # Collect all install choices upfront so the rest of the script runs unattended.
    INSTALL_GHOSTTY=true
    INSTALL_NEOVIM=true
    INSTALL_NEOVIM_CONFIG=true
    INSTALL_CHARVIM=false
    INSTALL_OPTIONAL=false
    INSTALL_DEV=false
    INSTALL_SSM=false
    SETUP_SSH_KEY=false
    PERSONAL_PKG_FILE=""

    if [ -z "$CI" ]; then
        echo ""
        if command -v gum &> /dev/null; then
            if [[ "$CURRENT_OS" == "Darwin" ]]; then
                gum confirm "Install Ghostty terminal? (the terminal this config is built for)" \
                    --default=true  || INSTALL_GHOSTTY=false
            fi
            gum confirm "Install Neovim text editor?" \
                --default=true      || INSTALL_NEOVIM=false
            if [ "$INSTALL_NEOVIM" = true ] && [ -d "$(dirname "$0")/src/nvim" ]; then
                gum confirm "  ↳ Link Neovim config from dotfiles?" \
                    --default=true  || INSTALL_NEOVIM_CONFIG=false
            fi
            if [ "$INSTALL_NEOVIM" = true ]; then
                gum confirm "  ↳ Install Charvim (Neovim config)?" \
                    --default=true  || INSTALL_CHARVIM=false
            fi
            gum confirm "Install optional tools? (btop, lazygit, yazi, herdr)" \
                --default=false     && INSTALL_OPTIONAL=true
            gum confirm "Install developer tools? (gh, java, go, cmake, gcc, docker...)" \
                --default=false     && INSTALL_DEV=true
            gum confirm "Set up SSM (SSH Session Manager with herdr --remote)?" \
                --default=false     && INSTALL_SSM=true
            if [ "$INSTALL_SSM" = true ]; then
                gum confirm "  ↳ Generate/configure SSH key for passwordless auth?" \
                    --default=true  && SETUP_SSH_KEY=true
            fi
            if gum confirm "Import a personal packages file?" --default=false; then
                PERSONAL_PKG_FILE=$(gum input --placeholder "Path to file (e.g. ~/system-packages/packages.personal)")
                PERSONAL_PKG_FILE="${PERSONAL_PKG_FILE/#\~/$HOME}"
            fi
        else
            if [[ "$CURRENT_OS" == "Darwin" ]]; then
                read -p "Install Ghostty terminal? (Y/n) " -n 1 -r; echo
                [[ $REPLY =~ ^[Nn]$ ]] && INSTALL_GHOSTTY=false
            fi
            read -p "Install Neovim text editor? (Y/n) " -n 1 -r; echo
            [[ $REPLY =~ ^[Nn]$ ]] && INSTALL_NEOVIM=false
            if [ "$INSTALL_NEOVIM" = true ] && [ -d "$(dirname "$0")/src/nvim" ]; then
                read -p "  Link Neovim config from dotfiles? (Y/n) " -n 1 -r; echo
                [[ $REPLY =~ ^[Nn]$ ]] && INSTALL_NEOVIM_CONFIG=false
            fi
            if [ "$INSTALL_NEOVIM" = true ]; then
                read -p "  Install Charvim (Neovim config)? (Y/n) " -n 1 -r; echo
                [[ $REPLY =~ ^[Nn]$ ]] && INSTALL_CHARVIM=false
            fi
            read -p "Install optional tools? (btop, lazygit, yazi, herdr) (y/N) " -n 1 -r; echo
            [[ $REPLY =~ ^[Yy]$ ]] && INSTALL_OPTIONAL=true
            read -p "Install developer tools? (gh, java, go, cmake, gcc, docker...) (y/N) " -n 1 -r; echo
            [[ $REPLY =~ ^[Yy]$ ]] && INSTALL_DEV=true
            read -p "Set up SSM (SSH Session Manager with herdr --remote)? (y/N) " -n 1 -r; echo
            [[ $REPLY =~ ^[Yy]$ ]] && INSTALL_SSM=true
            if [ "$INSTALL_SSM" = true ]; then
                read -p "  Generate/configure SSH key for passwordless auth? (Y/n) " -n 1 -r; echo
                [[ ! $REPLY =~ ^[Nn]$ ]] && SETUP_SSH_KEY=true
            fi
            read -p "Personal packages file path (press Enter to skip): " PERSONAL_PKG_FILE
            PERSONAL_PKG_FILE="${PERSONAL_PKG_FILE/#\~/$HOME}"
        fi
        echo ""
    fi

    # --- OS-specific package install ---
    if [[ "$CURRENT_OS" == "Darwin" ]]; then
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

        # --- packages:ghostty ---
        if [ "$INSTALL_GHOSTTY" = true ]; then
            if cask_installed "ghostty"; then
                echo "✅ Ghostty already installed"
            else
                run_with_spinner "Installing Ghostty..." \
                    run_timeout "$DEPENDENCY_TIMEOUT" brew install --cask ghostty || \
                    echo "⚠️ Ghostty failed to install"
            fi
        fi

        # --- packages:deps ---
        if [ -f "$HOME/.dots/src/zsh/zsh/dots.py" ]; then
            echo "📦 Installing config dependencies..."
            if [ "$INSTALL_NEOVIM" = false ]; then
                DOTS_SKIP_BINS=nvim python3 "$HOME/.dots/src/zsh/zsh/dots.py" --install-deps
            else
                python3 "$HOME/.dots/src/zsh/zsh/dots.py" --install-deps
            fi
        fi

        # --- packages:optional ---
        if [ "$INSTALL_OPTIONAL" = true ] && [ -f "$HOME/.dots/src/zsh/zsh/dots.py" ]; then
            echo "📦 Installing optional tools..."
            python3 "$HOME/.dots/src/zsh/zsh/dots.py" --install-optional
        fi

        # --- packages:dev ---
        if [ "$INSTALL_DEV" = true ] && [ -f "$HOME/.dots/src/zsh/zsh/dots.py" ]; then
            echo "📦 Installing developer tools..."
            python3 "$HOME/.dots/src/zsh/zsh/dots.py" --install-dev
        fi

        # --- configuration:Edits ---
        rm -f ~/.zprofile

    elif is_fedora; then
        echo "🐧 Fedora detected — using dnf"

        setup_fedora_repos
        install_fedorafile_section "$HOME/.dots/assets/packages.fedora" "required"
        install_zsh_history_substring_search

        if [ "$INSTALL_OPTIONAL" = true ]; then
            echo "📦 Installing optional tools..."
            install_fedorafile_section "$HOME/.dots/assets/packages.fedora" "optional"
        fi

        if [ "$INSTALL_DEV" = true ]; then
            echo "📦 Installing developer tools..."
            install_fedorafile_section "$HOME/.dots/assets/packages.fedora" "dev"
        fi

        install_ollama
        install_yt_dlp

    else
        echo "⚠️ Unsupported OS: $CURRENT_OS"
        echo "This script supports macOS and Fedora Linux."
        exit 1
    fi

    # --- packages:GitHub ---
    install_fzf_tab

    # --- packages:opencode ---
    install_opencode
    install_opencode_config_deps

    # --- packages:personal ---
    if [ -n "$PERSONAL_PKG_FILE" ] && [ -f "$PERSONAL_PKG_FILE" ]; then
        echo "📦 Installing personal packages from $PERSONAL_PKG_FILE..."
        if [[ "$CURRENT_OS" == "Darwin" ]]; then
            install_brewfile "$PERSONAL_PKG_FILE"
        elif is_fedora; then
            install_fedorafile "$PERSONAL_PKG_FILE"
        fi
    elif [ -n "$PERSONAL_PKG_FILE" ]; then
        echo "⚠️ Personal packages file not found at: $PERSONAL_PKG_FILE"
    fi

    # --- configuration:SSM ---
    if [ "$INSTALL_SSM" = true ]; then
        setup_ssm
    fi

    # --- configuration:Links & Verify ---
    relink_and_verify "$HOME/.dots"

    # --- configuration:Charvim ---
    if [ "$INSTALL_CHARVIM" = true ]; then
        echo "🔧 Installing Charvim..."
        install_charvim
    fi
}

do_install

# --- Charfiles:finish ---
if command -v gum &> /dev/null; then
    gum style --foreground 200 --border normal --padding "0.5" --margin "0.5" <<EOF
🎉 Installation Complete! 🎉

Your environment has been set up with your configuration files.
You can now start customizing your setup further!

Thank you for using my dotfiles!
EOF
else
    echo ""
    echo "================================"
    echo "🎉 Installation Complete! 🎉"
    echo "================================"
    echo ""
    echo "Your environment has been set up with your configuration files."
    echo "You can now start customizing your setup further!"
    echo ""
    echo "Thank you for using my dotfiles!"
    echo ""
fi
