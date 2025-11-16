#!/usr/bin/env bash

# --- Charfiles:macOS install script ---
#
# date created: 08.29.2025

# --- charfile:start ---
# psudocode:
# 1. prompt user if they want to setup git
# 2. install homebrew if missing
# 3. install brewfile packages
# 4. show preview of the prompt and prompt for which prompt theme to use
# 5. create symlinks for config files
# 6. make sure all of the files are working correctly

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
    ________  ___  ___  ________  ________  ________ ___  ___       _______      
    |\   ____\|\  \|\  \|\   __  \|\   __  \|\  _____\\  \|\  \     |\  ___ \     
    \ \  \___|\ \  \\\  \ \  \|\  \ \  \|\  \ \  \__/\ \  \ \  \    \ \   __/|    
     \ \  \    \ \   __  \ \   __  \ \   _  _\ \   __\\ \  \ \  \    \ \  \_|/__  
      \ \  \____\ \  \ \  \ \  \ \  \ \  \\  \\ \  \_| \ \  \ \  \____\ \  \_|\ \ 
       \ \_______\ \__\ \__\ \__\ \__\ \__\\ _\\ \__\   \ \__\ \_______\ \_______\
        \|_______|\|__|\|__|\|__|\|__|\|__|\|__|\|__|    \|__|\|_______|\|_______|
EOF

    # --- git:setup ---
    # prompt user to setup git
    if gum confirm "Do you want to set up Git now?"; then
        # prompt for user name
        git_username=$(gum input --placeholder "Enter your Git username" --prompt "Username: ")
        # prompt for user email
        git_email=$(gum input --placeholder "Enter your Git email" --prompt "Email: ")
        # set git config
        git config --global user.name "$git_username"
        git config --global user.email "$git_email"
        gum style --foreground 34 --border thick --padding "1" --margin "1" <<EOF
✅ Git has been configured with the provided username and email.
EOF
    else
        gum style --foreground 208 --border thick --padding "1" --margin "1" <<EOF
⚠️ Skipping Git setup. You can configure it later using 'git config --global'.
EOF
    fi

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

    # --- configuration:Charfiles ---
    # make charfiles directory
    if [ ! -d ~/.charfiles ]; then
        gum spin --spinner dot --title "Making Charfiles directory..." -- \
                bash -c "mkdir -p ~/.charfiles && git clone https://github.com/Charlynder/Charfiles.git ~/.charfiles"
        sleep 1
    else
        echo "Charfiles directory has already been created ✅"
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
    # remove the install script, readme file, and changelog file from the installation
    rm -rf ~/.charfiles/install.sh
    rm -rf ~/.charfiles/README.md
    rm -rf ~/.charfiles/CHANGELOG.md

    # --- configuration:Prompts ---
    # prompt user for which prompt theme to use
    # list of available themes from the ./zsh/themes directory
    if [ -d "./zsh/themes" ]; then
        available_themes=($(ls ./zsh/themes))
        if [ ${#available_themes[@]} -gt 0 ]; then
            # Create theme descriptions
            theme_options=()
            for theme in "${available_themes[@]}"; do
                case "$theme" in
                    *tmux*)
                        theme_options+=("$theme - Opens tmux by default")
                        ;;
                    *modal* | *mode*)
                        theme_options+=("$theme - Modal theme that changes dynamically")
                        ;;
                    *multi*)
                        theme_options+=("$theme - Multi-line prompt with detailed information")
                        ;;
                    *)
                        theme_options+=("$theme")
                        ;;
                esac
            done
            
            selected_option=$(gum choose --header "Select a Zsh prompt theme:" --height 15 "${theme_options[@]}")
            
            # Extract just the theme name (before the dash if description exists)
            selected_theme=$(echo "$selected_option" | awk '{print $1}')
            
            # modify the .zshrc file to use the selected theme
            if [ -n "$selected_theme" ]; then
                gum spin --spinner dot --title "Setting Zsh prompt theme to $selected_theme..." -- bash -c "sed -i '' 's/^ZSH_THEME=.*/ZSH_THEME=\"$selected_theme\"/' ~/.zshrc"
                sleep 1
            else
                gum style --foreground 208 --border normal --padding "1" --margin "1" <<EOF
⚠️ No theme selected. Keeping the default Zsh prompt theme.
EOF
            fi
        else
            echo "⚠️ No themes found in ./zsh/themes directory."
        fi
    else
        echo "⚠️ Theme directory ./zsh/themes not found. Skipping theme selection."
    fi

    # --- configuration:Links ---
    # make links for configurations
    gum spin --spinner dot --title "Creating links for configuration files..." -- bash -c "ln -sf ~/.charfiles/*/ ~/.config/"
    sleep 1

    # --- verification:Check ---
    # verify that the files are working correctly
    gum spin --spinner dot --title "Verifying installation..." -- ls ~/.config/
    sleep 1
)

# --- Charfiles:finish ---
gum style --foreground 200 --border normal --padding "0.5" --margin "0.5" <<EOF
🎉 Installation Complete! 🎉

Your macOS environment has been set up with Homebrew, Git, and your configuration files.
You can now start customizing your setup further!

Thank you for using Charfiles!
EOF
