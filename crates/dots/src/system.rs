//! OS and package-manager detection for the user package manifest
//! ([`crate::user_packages`]).
//!
//! Two questions this module answers about the current machine:
//!
//! * **Which OS is this?** — resolved to a *tag set* so a manifest can match by
//!   family (`macos`, `linux`, `bsd`) or by distro (`fedora`, `ubuntu`, …). Distro
//!   tags come from `/etc/os-release` (`ID` + `ID_LIKE`), the freedesktop standard,
//!   so `os = ["debian"]` transparently covers Ubuntu/Mint/Pop via `ID_LIKE`.
//! * **Which package manager is active?** — the first supported PM found on
//!   `PATH`, plus the exact, per-PM install invocation (assume-yes flag, whether
//!   `sudo` is required). This is deliberately *not* uniform: `apt`/`dnf`/`pkg`
//!   take `-y`, but `pacman` takes `--noconfirm` (in pacman, `-y` means "refresh
//!   the sync DB", not "assume yes"), and `brew` refuses to run as root.

use std::collections::BTreeSet;
use std::process::{Command, Stdio};

// ── package managers ──────────────────────────────────────────────────────────

/// A package manager dots can drive from a user manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pm {
    Brew,
    Apt,
    Dnf,
    Pacman,
    Pkg,
}

impl Pm {
    /// Canonical key used in a manifest's `managers = [...]` list.
    pub fn key(self) -> &'static str {
        match self {
            Pm::Brew => "brew",
            Pm::Apt => "apt",
            Pm::Dnf => "dnf",
            Pm::Pacman => "pacman",
            Pm::Pkg => "pkg",
        }
    }

    /// Parse a manifest key into a `Pm`, accepting a few friendly aliases
    /// (`homebrew`, `apt-get`, `rpm` → dnf). Unknown keys yield `None`.
    pub fn from_key(k: &str) -> Option<Pm> {
        match k.to_lowercase().as_str() {
            "brew" | "homebrew" => Some(Pm::Brew),
            "apt" | "apt-get" => Some(Pm::Apt),
            "dnf" | "rpm" => Some(Pm::Dnf),
            "pacman" => Some(Pm::Pacman),
            "pkg" => Some(Pm::Pkg),
            _ => None,
        }
    }

    /// Homebrew refuses to run under `sudo`; every other supported PM needs it.
    pub fn needs_sudo(self) -> bool {
        !matches!(self, Pm::Brew)
    }

    /// Program name and base arguments (install verb + the correct assume-yes
    /// flag) that precede any user flags and the package list.
    fn base_argv(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Pm::Brew => ("brew", &["install"]),
            Pm::Apt => ("apt-get", &["install", "-y"]),
            Pm::Dnf => ("dnf", &["install", "-y"]),
            Pm::Pacman => ("pacman", &["-S", "--noconfirm"]),
            Pm::Pkg => ("pkg", &["install", "-y"]),
        }
    }

    /// The full command to install `packages` with optional extra `flags`,
    /// returned as `(program, argv)`. When `sudo` is required the PM program is
    /// pushed as the first argument and `program` is `"sudo"`.
    pub fn install_command(self, flags: &[String], packages: &[String]) -> (String, Vec<String>) {
        let (prog, base) = self.base_argv();
        let mut argv: Vec<String> = Vec::new();
        let program = if self.needs_sudo() {
            argv.push(prog.to_string());
            "sudo".to_string()
        } else {
            prog.to_string()
        };
        argv.extend(base.iter().map(|s| s.to_string()));
        argv.extend(flags.iter().cloned());
        argv.extend(packages.iter().cloned());
        (program, argv)
    }
}

/// Every supported package manager found on `PATH`, in preference order.
pub fn detect_pms() -> Vec<Pm> {
    let mut out = Vec::new();
    if which_exists("brew") {
        out.push(Pm::Brew);
    }
    if which_exists("apt-get") || which_exists("apt") {
        out.push(Pm::Apt);
    }
    if which_exists("dnf") {
        out.push(Pm::Dnf);
    }
    if which_exists("pacman") {
        out.push(Pm::Pacman);
    }
    if which_exists("pkg") {
        out.push(Pm::Pkg);
    }
    out
}

/// The single package manager dots will use for `[[group]]` installs, if any.
pub fn active_pm() -> Option<Pm> {
    detect_pms().into_iter().next()
}

/// Whether `bin` resolves on `PATH`.
pub fn which_exists(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ── OS tags ───────────────────────────────────────────────────────────────────

/// The set of tags describing this machine. Always contains the OS family
/// (`std::env::consts::OS`); on the BSDs it also contains `bsd`, and on Linux it
/// contains the `os-release` `ID` and every `ID_LIKE` entry.
pub fn os_tags() -> BTreeSet<String> {
    let mut tags = BTreeSet::new();
    let family = std::env::consts::OS; // "linux", "macos", "freebsd", …
    tags.insert(family.to_string());
    match family {
        "freebsd" | "openbsd" | "netbsd" | "dragonfly" => {
            tags.insert("bsd".to_string());
        }
        "linux" => {
            for t in read_os_release_tags() {
                tags.insert(t);
            }
        }
        _ => {}
    }
    tags
}

fn read_os_release_tags() -> Vec<String> {
    for path in ["/etc/os-release", "/usr/lib/os-release"] {
        if let Ok(content) = std::fs::read_to_string(path) {
            return parse_os_release(&content);
        }
    }
    Vec::new()
}

/// Extract `ID` and `ID_LIKE` values from `os-release` content as lower-case
/// tags. `ID_LIKE` is a space-separated list per the freedesktop spec.
pub fn parse_os_release(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("ID=") {
            let id = unquote(v);
            if !id.is_empty() {
                tags.push(id.to_lowercase());
            }
        } else if let Some(v) = line.strip_prefix("ID_LIKE=") {
            for part in unquote(v).split_whitespace() {
                tags.push(part.to_lowercase());
            }
        }
    }
    tags
}

/// Strip a single matching pair of surrounding single or double quotes.
fn unquote(s: &str) -> String {
    let s = s.trim();
    let bytes = s.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' || first == b'\'') && first == last {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_os_release_fedora() {
        let content = "NAME=Fedora\nID=fedora\nVERSION_ID=40\n";
        assert_eq!(parse_os_release(content), vec!["fedora"]);
    }

    #[test]
    fn parse_os_release_ubuntu_id_like() {
        let content = "ID=ubuntu\nID_LIKE=debian\n";
        assert_eq!(parse_os_release(content), vec!["ubuntu", "debian"]);
    }

    #[test]
    fn parse_os_release_quoted_and_multi_id_like() {
        let content = "ID=\"linuxmint\"\nID_LIKE=\"ubuntu debian\"\n";
        assert_eq!(
            parse_os_release(content),
            vec!["linuxmint", "ubuntu", "debian"]
        );
    }

    #[test]
    fn parse_os_release_single_quotes_and_case() {
        let content = "ID='Fedora'\n";
        assert_eq!(parse_os_release(content), vec!["fedora"]);
    }

    #[test]
    fn parse_os_release_empty() {
        assert!(parse_os_release("NAME=Whatever\n").is_empty());
    }

    #[test]
    fn from_key_aliases() {
        assert_eq!(Pm::from_key("homebrew"), Some(Pm::Brew));
        assert_eq!(Pm::from_key("apt-get"), Some(Pm::Apt));
        assert_eq!(Pm::from_key("rpm"), Some(Pm::Dnf));
        assert_eq!(Pm::from_key("PACMAN"), Some(Pm::Pacman));
        assert_eq!(Pm::from_key("nope"), None);
    }

    #[test]
    fn install_command_dnf_uses_sudo_and_yes() {
        let (program, argv) =
            Pm::Dnf.install_command(&[], &["ghostty".into(), "zsh".into()]);
        assert_eq!(program, "sudo");
        assert_eq!(argv, vec!["dnf", "install", "-y", "ghostty", "zsh"]);
    }

    #[test]
    fn install_command_pacman_uses_noconfirm_not_dash_y() {
        let (program, argv) = Pm::Pacman.install_command(&[], &["ghostty".into()]);
        assert_eq!(program, "sudo");
        assert_eq!(argv, vec!["pacman", "-S", "--noconfirm", "ghostty"]);
    }

    #[test]
    fn install_command_brew_never_sudo() {
        let (program, argv) = Pm::Brew.install_command(&[], &["ghostty".into()]);
        assert_eq!(program, "brew");
        assert_eq!(argv, vec!["install", "ghostty"]);
    }

    #[test]
    fn install_command_appends_user_flags() {
        let (_p, argv) =
            Pm::Apt.install_command(&["--no-install-recommends".into()], &["zsh".into()]);
        assert_eq!(
            argv,
            vec!["apt-get", "install", "-y", "--no-install-recommends", "zsh"]
        );
    }
}
