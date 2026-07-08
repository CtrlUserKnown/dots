use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Required,
    Optional,
    Dev,
}

#[derive(Debug)]
pub struct Dep {
    pub bin:      &'static str,
    pub brew:     &'static str,
    pub dnf:      &'static str,
    pub apt:      &'static str,
    pub desc:     &'static str,
    pub category: Category,
    pub tap:      &'static str,
    pub cask:     bool,
    pub script:   &'static str,
}

pub static DEPS: &[Dep] = &[
    // ── required ──────────────────────────────────────────────────────────────
    Dep { bin: "git",       brew: "git",        dnf: "git",        apt: "git",        desc: "version control",                   category: Category::Required, tap: "", cask: false, script: "" },
    Dep { bin: "eza",       brew: "eza",        dnf: "eza",        apt: "eza",        desc: "modern ls replacement",              category: Category::Required, tap: "", cask: false, script: "" },
    Dep { bin: "bat",       brew: "bat",        dnf: "bat",        apt: "bat",        desc: "fzf previewer & syntax highlighter", category: Category::Required, tap: "", cask: false, script: "" },
    Dep { bin: "fd",        brew: "fd",         dnf: "fd-find",    apt: "fd-find",    desc: "fast file finder (fzf command)",     category: Category::Required, tap: "", cask: false, script: "" },
    Dep { bin: "fzf",       brew: "fzf",        dnf: "fzf",        apt: "fzf",        desc: "fuzzy finder",                       category: Category::Required, tap: "", cask: false, script: "" },
    Dep { bin: "fastfetch", brew: "fastfetch",  dnf: "fastfetch",  apt: "fastfetch",  desc: "system info at shell start",         category: Category::Required, tap: "", cask: false, script: "" },
    Dep { bin: "zoxide",    brew: "zoxide",     dnf: "zoxide",     apt: "zoxide",     desc: "smarter cd",                         category: Category::Required, tap: "", cask: false, script: "" },
    // ── optional ──────────────────────────────────────────────────────────────
    Dep { bin: "nvim",      brew: "neovim",     dnf: "neovim",     apt: "neovim",     desc: "text editor ($EDITOR)",              category: Category::Optional, tap: "", cask: false, script: "" },
    Dep { bin: "herdr",     brew: "herdr",      dnf: "",           apt: "",           desc: "terminal multiplexer (mux alias)",   category: Category::Optional, tap: "", cask: false, script: "curl -fsSL https://herdr.dev/install.sh | sh" },
    Dep { bin: "btop",      brew: "btop",       dnf: "btop",       apt: "btop",       desc: "system monitor (b alias)",           category: Category::Optional, tap: "", cask: false, script: "" },
    Dep { bin: "lazygit",   brew: "lazygit",    dnf: "lazygit",    apt: "lazygit",    desc: "git TUI (lz alias)",                 category: Category::Optional, tap: "", cask: false, script: "" },
    Dep { bin: "yazi",      brew: "yazi",       dnf: "",           apt: "",           desc: "file manager (y alias)",             category: Category::Optional, tap: "", cask: false, script: "" },
    Dep { bin: "carapace",  brew: "carapace",   dnf: "carapace",   apt: "",           desc: "shell completions engine",           category: Category::Optional, tap: "", cask: false, script: "" },
    // ── dev ───────────────────────────────────────────────────────────────────
    Dep { bin: "java",      brew: "openjdk@21", dnf: "java-21-openjdk", apt: "default-jdk", desc: "Java Development Kit (LTS)", category: Category::Dev, tap: "", cask: false, script: "" },
    Dep { bin: "go",        brew: "go",         dnf: "golang",     apt: "golang",     desc: "Go programming language",            category: Category::Dev, tap: "", cask: false, script: "" },
    Dep { bin: "lua",       brew: "lua",        dnf: "lua",        apt: "lua",        desc: "Lua scripting language",             category: Category::Dev, tap: "", cask: false, script: "" },
    Dep { bin: "mvn",       brew: "maven",      dnf: "maven",      apt: "maven",      desc: "Java / Maven build tool",            category: Category::Dev, tap: "", cask: false, script: "" },
    Dep { bin: "cmake",     brew: "cmake",      dnf: "cmake",      apt: "cmake",      desc: "cross-platform build system",        category: Category::Dev, tap: "", cask: false, script: "" },
    Dep { bin: "gcc",       brew: "gcc",        dnf: "gcc",        apt: "gcc",        desc: "GNU C/C++ compiler",                 category: Category::Dev, tap: "", cask: false, script: "" },
    Dep { bin: "rg",        brew: "ripgrep",    dnf: "ripgrep",    apt: "ripgrep",    desc: "fast grep replacement",              category: Category::Dev, tap: "", cask: false, script: "" },
    Dep { bin: "shellcheck",brew: "shellcheck", dnf: "ShellCheck", apt: "shellcheck", desc: "shell script linter",                category: Category::Dev, tap: "", cask: false, script: "" },
    Dep { bin: "gh",        brew: "gh",         dnf: "gh",         apt: "gh",         desc: "GitHub CLI",                         category: Category::Dev, tap: "", cask: false, script: "" },
    Dep { bin: "docker",    brew: "docker",     dnf: "",           apt: "",           desc: "container runtime",                  category: Category::Dev, tap: "", cask: false, script: "" },
    Dep { bin: "nmap",      brew: "nmap",       dnf: "nmap",       apt: "nmap",       desc: "network scanner",                    category: Category::Dev, tap: "", cask: false, script: "" },
    Dep { bin: "ffmpeg",    brew: "ffmpeg",     dnf: "ffmpeg",     apt: "ffmpeg",     desc: "audio/video converter",              category: Category::Dev, tap: "", cask: false, script: "" },
];

#[derive(Debug)]
pub struct Plugin {
    pub name:  &'static str,
    pub desc:  &'static str,
    pub paths: &'static [&'static str],
}

pub static PLUGINS: &[Plugin] = &[
    Plugin {
        name:  "zsh-autosuggestions",
        desc:  "fish-like suggestions as you type",
        paths: &[
            "/opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh",
            "/usr/share/zsh-autosuggestions/zsh-autosuggestions.zsh",
        ],
    },
    Plugin {
        name:  "zsh-syntax-highlighting",
        desc:  "color-codes commands as you type",
        paths: &[
            "/opt/homebrew/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh",
            "/usr/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh",
        ],
    },
    Plugin {
        name:  "zsh-history-substring-search",
        desc:  "search history by typing a fragment",
        paths: &[
            "~/.config/zsh/plugins/zsh-history-substring-search",
            "/opt/homebrew/share/zsh-history-substring-search/zsh-history-substring-search.zsh",
        ],
    },
    Plugin {
        name:  "fzf-tab",
        desc:  "fuzzy-search tab completions",
        paths: &["~/.config/zsh/plugins/fzf-tab"],
    },
];

pub fn check_dep(dep: &Dep) -> bool {
    if dep.bin.is_empty() { return false; }
    let alts: Vec<&str> = match dep.bin {
        "fd"  => vec!["fd", "fdfind"],
        "bat" => vec!["bat", "batcat"],
        b     => vec![b],
    };
    alts.iter().any(|b| which_exists(b))
}

pub fn check_plugin(plugin: &Plugin) -> bool {
    let home = dirs::home_dir().unwrap_or_default();
    plugin.paths.iter().any(|p| {
        let path: PathBuf = if let Some(rest) = p.strip_prefix("~/") {
            home.join(rest)
        } else {
            PathBuf::from(p)
        };
        path.exists()
    })
}

pub fn detect_pkg_manager() -> Option<&'static str> {
    if which_exists("brew") { return Some("brew"); }
    if which_exists("dnf")  { return Some("dnf"); }
    if which_exists("apt")  { return Some("apt"); }
    None
}

fn which_exists(bin: &str) -> bool {
    std::process::Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn install_one(dep: &Dep) -> Result<()> {
    crate::installer::install_dep(dep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_dep_git_is_present() {
        let git = DEPS.iter().find(|d| d.bin == "git").unwrap();
        assert!(check_dep(git), "git must be installed to build this crate");
    }

    #[test]
    fn check_dep_fake_binary_is_absent() {
        let fake = Dep {
            bin: "dots-definitely-not-a-real-binary-xyz",
            brew: "", dnf: "", apt: "", desc: "",
            category: Category::Required, tap: "", cask: false,
        };
        assert!(!check_dep(&fake));
    }

    #[test]
    fn detect_pkg_manager_does_not_panic() {
        let _ = detect_pkg_manager();
    }
}
