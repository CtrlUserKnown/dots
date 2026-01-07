#!/usr/bin/env bash

# --- dotfiles:macOS install script ---
# date created: 08.29.2025

# --- charfile:start ---

# --- script prompting:gum ---
# ensure gum is installed (silent)
install_gum() {
    if ! command -v gum &> /dev/null; then
        tmpdir=$(mktemp -d)
        curl -sSL "https://github.com/charmbracelet/gum/releases/latest/download/gum_$(uname -s)_$(uname -m).tar.gz" \
        | tar -xz -C "$tmpdir"
        sudo mv "$tmpdir/gum" /usr/local/bin/gum >/dev/null 2>&1
        rm -rf "$tmpdir"
    fi
}

# install gum if missing
install_gum

(
    # --- script:Banner ---
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

    # --- package manager:homebrew ---
    # install Homebrew on to the system
    if command -v brew &> /dev/null; then
        echo "☕️ Homebrew is already installed."
    else
        echo "🚀 Homebrew not found. Installing..."

        # download the installer script first
        gum spin --spinner dot --title "Downloading Homebrew installer..." -- \
            curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh -o /tmp/homebrew_install.sh

        # run the installer (needs to be interactive)
        gum spin --spinner dot --title "Running Homebrew installer..." -- /bin/bash /tmp/homebrew_install.sh

        # Clean up
        rm -f /tmp/homebrew_install.sh

        # Add Homebrew to PATH if necessary
        if [[ -d "/opt/homebrew/bin" ]]; then
            echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
            eval "$(/opt/homebrew/bin/brew shellenv)"
        elif [[ -d "/usr/local/bin" ]]; then
            echo 'eval "$(/usr/local/bin/brew shellenv)"' >> ~/.zprofile
            eval "$(/usr/local/bin/brew shellenv)"
        fi

        echo "✅ Homebrew installation complete."
    fi

    # --- configuration:.config ---
    # make syslinks for config
    echo "🔧 Checking for configuration files..."

    # make .config directory
    if [ ! -d ~/.config ]; then
        gum spin --spinner dot --title "Creating Config folder..." -- mkdir ~/.config
        sleep 1
    else
        echo "✅ Config directory already exists!"
    fi

    # --- configuration:Dotfiles ---
    # make dotfiles directory
    if [ ! -d ~/.dots ]; then
        gum spin --spinner dot --title "Making Dotfiles directory (~/.dots)..." -- \
                bash -c "mkdir -p ~/.dots && git clone https://github.com/CtrlUserKnown/dotfiles.git ~/.dots"
        sleep 1
    else
        echo "Dotfiles directory has already been created ✅"
    fi

    # --- packages:Brewfile ---
    # install Homebrew files if the packages do not exist on the system
    # compare the installed files with the Brewfile and brew list
    # if the package is missing, install it
    if [ -f "./Brewfile" ]; then
        gum spin --spinner dot --title "Installing packages from Brewfile..." -- brew bundle --file=./Brewfile
        echo "✅ Brewfile packages installation complete."
    else
        echo "⚠️ No Brewfile found in the current directory. Skipping package installation."
    fi

    # --- configuration:Edits ---
    # zprofile will not be need since homebrew will be called in .zshrc file (only to be use during install)
    rm -rf ~/.zprofile

    # --- configuration:Links ---
    # create links for configurations, final form (lol)
    gum spin --spinner dot --title "Creating links for configuration files..." -- bash -c '
        # Create symlinks for .config directories
        ln -sf ~/.dots/src/bat ~/.config/bat
        ln -sf ~/.dots/src/fastfetch ~/.config/fastfetch
        ln -sf ~/.dots/src/ghostty ~/.config/ghostty
        ln -sf ~/.dots/src/tmux ~/.config/tmux

        # Create symlinks for home directory
        ln -sf ~/.dots/src/zsh/zsh ~/.config/zsh
        ln -sf ~/.dots/src/zsh/.zshrc ~/.zshrc
    '
    sleep 1

    # --- verification:Check ---
    # verify that the files are working correctly
    gum spin --spinner dot --title "Verifying installation..." -- bash -c '
        all_good=true

        # Check .config symlinks
        for dir in bat fastfetch ghostty tmux zsh; do
            if [ ! -L ~/.config/$dir ]; then
                echo "⚠️ Missing symlink: ~/.config/$dir"
                all_good=false
            fi
        done

        # Check home directory symlinks
        if [ ! -L ~/.zshrc ]; then
            echo "⚠️ Missing symlink: ~/.zshrc"
            all_good=false
        fi

        # Check if dotfiles repo exists
        if [ ! -d ~/.dots/.git ]; then
            echo "⚠️ Dotfiles repository not properly cloned"
            all_good=false
        fi

        if [ "$all_good" = true ]; then
            echo "✅ All configuration files verified successfully"
        else
            echo "⚠️ Some files are missing or not properly linked"
            exit 1
        fi
    '
    sleep 1

)

# --- Charfiles:finish ---
gum style --foreground 200 --border normal --padding "0.5" --margin "0.5" <<EOF
🎉 Installation Complete! 🎉

Your macOS environment has been set up with Homebrew, Git, and your configuration files.
You can now start customizing your setup further!

Thank you for using my dotfiles!
EOF
