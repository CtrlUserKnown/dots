#!/usr/bin/env bash

# --- dotfiles:macOS install script ---
# date created: 08.29.2025

# --- charfile:start ---

# --- script prompting:gum ---
# ensure gum is installed (silent)
install_gum() {
    if ! command -v gum &> /dev/null; then
        echo "Installing gum..."

        # Use Homebrew to install gum if available, otherwise manual install
        if command -v brew &> /dev/null; then
            brew install gum >/dev/null 2>&1
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
        if curl -sSL "https://github.com/charmbracelet/gum/releases/latest/download/gum_${OS}_${ARCH}.tar.gz" \
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

(
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
    # install Homebrew on to the system
    if command -v brew &> /dev/null; then
        echo "☕️ Homebrew is already installed."
    else
        echo "🚀 Homebrew not found. Installing..."

        # Non-interactive installation for CI
        if [ -n "$CI" ]; then
            NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
        else
            /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
        fi

        # Add Homebrew to PATH if necessary
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

        # Try to install gum via Homebrew after Homebrew is installed
        if ! command -v gum &> /dev/null; then
            brew install gum >/dev/null 2>&1 || echo "⚠️ Could not install gum via Homebrew"
        fi
    fi

    # --- configuration:.config ---
    # make syslinks for config
    echo "🔧 Checking for configuration files..."

    # make .config directory
    if [ ! -d ~/.config ]; then
        run_with_spinner "Creating Config folder..." mkdir ~/.config
        sleep 1
    else
        echo "✅ Config directory already exists!"
    fi

    # --- configuration:Dotfiles ---
    # make dotfiles directory
    if [ ! -d ~/.dots ]; then
        run_with_spinner "Making Dotfiles directory (~/.dots)..." \
                bash -c "mkdir -p ~/.dots && git clone https://github.com/CtrlUserKnown/dotfiles.git ~/.dots"
        sleep 1
    else
        echo "Dotfiles directory has already been created ✅"
    fi

    # --- packages:Brewfile ---
    # install Homebrew files if the packages do not exist on the system
    # compare the installed files with the Brewfile and brew list
    # if the package is missing, install it
    if [ -f "./assets/Brewfile" ]; then
        run_with_spinner "Installing packages from Brewfile..." brew bundle --file=./assets/Brewfile
        echo "✅ Brewfile packages installation complete."
    else
        echo "⚠️ No Brewfile found in the current directory. Skipping package installation."
    fi

    # --- configuration:Edits ---
    # zprofile will not be need since homebrew will be called in .zshrc file (only to be use during install)
    rm -f ~/.zprofile

    # --- configuration:Links ---
    # create links for configurations, final form (lol)
    run_with_spinner "Creating links for configuration files..." bash -c "
        # Create symlinks for .config directories
        ln -sf $HOME/.dots/src/bat $HOME/.config/bat
        ln -sf $HOME/.dots/src/fastfetch $HOME/.config/fastfetch
        ln -sf $HOME/.dots/src/ghostty $HOME/.config/ghostty
        ln -sf $HOME/.dots/src/tmux $HOME/.config/tmux

        # Create symlinks for home directory
        ln -sf $HOME/.dots/src/zsh/zsh $HOME/.config/zsh
        ln -sf $HOME/.dots/src/zsh/.zshrc $HOME/.zshrc
    "
    sleep 1

    # --- verification:Check ---
    # verify that the files are working correctly
    run_with_spinner "Verifying installation..." bash -c "
        all_good=true

        # Check .config symlinks
        for dir in bat fastfetch ghostty tmux zsh; do
            if [ ! -L \$HOME/.config/\$dir ]; then
                echo \"⚠️ Missing symlink: \$HOME/.config/\$dir\"
                all_good=false
            fi
        done

        # Check home directory symlinks
        if [ ! -L \$HOME/.zshrc ]; then
            echo \"⚠️ Missing symlink: \$HOME/.zshrc\"
            all_good=false
        fi

        # Check if dotfiles repo exists
        if [ ! -d \$HOME/.dots/.git ]; then
            echo \"⚠️ Dotfiles repository not properly cloned\"
            all_good=false
        fi

        if [ \"\$all_good\" = true ]; then
            echo \"✅ All configuration files verified successfully\"
        else
            echo \"⚠️ Some files are missing or not properly linked\"
            exit 1
        fi
    "
    sleep 1

)

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
