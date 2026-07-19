use std::io;

use anyhow::Context;
use clap::{Parser, Subcommand};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

#[derive(Parser)]
#[command(name = "dots", version, about = "dots dotfiles manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Check and repair symlinks, tools, and plugins
    Health {
        /// Repair all symlinks (same as default behavior)
        #[arg(long)]
        fix: bool,
    },
    /// Check for and apply updates
    Update,
    /// Manage shell aliases
    Aliases {
        #[command(subcommand)]
        action: AliasAction,
    },
    /// Install dependencies or premade configs
    Install {
        /// Package name to install
        name: Option<String>,
        /// Install all missing core dependencies
        #[arg(long)]
        all: bool,
        /// Install all optional dependencies
        #[arg(long)]
        optional: bool,
    },
    /// Manage premade app configs
    Premade {
        #[command(subcommand)]
        action: PremadeAction,
    },
    /// Export/import personal config
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Initialize dots config (idempotent)
    Init {
        /// Suppress greeting output
        #[arg(long)]
        quiet: bool,
    },
}

#[derive(Subcommand)]
enum AliasAction {
    /// List all aliases (built-in + user)
    List,
    /// Add a user alias
    Add { name: String, value: String },
    /// Remove a user alias
    Remove { name: String },
}

#[derive(Subcommand)]
enum PremadeAction {
    /// List available premade configs
    List,
    /// Apply a premade config
    Apply { app: String },
}

#[derive(Subcommand)]
enum ProfileAction {
    /// Generate personal.json from current system
    Generate {
        #[arg(value_name = "PATH")]
        path: Option<std::path::PathBuf>,
    },
    /// Import personal.json from a local file
    Import { path: std::path::PathBuf },
    /// Import personal.json from GitHub (user/repo/path/to/file.json)
    ImportGit { spec: String },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None => run_tui(dots::tui::app::Screen::Main)?,
        Some(Command::Update) => run_tui(dots::tui::app::Screen::Update)?,
        Some(Command::Health { .. }) => {
            let report = dots::symlinks::repair_all()?;
            println!("  {} OK, {} repaired, {} skipped", report.ok, report.repaired, report.skipped);
        }
        Some(Command::Aliases { action }) => cli_aliases(action)?,
        Some(Command::Install { name, all, optional }) => cli_install(name, all, optional)?,
        Some(Command::Premade { action }) => cli_premade(action)?,
        Some(Command::Profile { action }) => cli_profile(action)?,
        Some(Command::Init { quiet }) => cli_init(quiet)?,
    }
    Ok(())
}

// ── CLI handlers ──────────────────────────────────────────────────────────────

fn cli_aliases(action: AliasAction) -> anyhow::Result<()> {
    use dots::aliases::{load_all_aliases, AliasSource};
    use dots::config::personal::personal_dir;
    use dots::config::settings::dots_dir;

    let dots    = dots_dir();
    let personal = personal_dir();
    match action {
        AliasAction::List => {
            let all = load_all_aliases(&dots, &personal);
            if all.is_empty() { println!("No aliases found."); return Ok(()); }
            println!("{:<20} {:<40} {}", "NAME", "COMMAND", "TYPE");
            println!("{}", "-".repeat(65));
            for a in &all {
                let t = if matches!(a.source, AliasSource::User) { "user" } else { "built-in" };
                println!("{:<20} {:<40} {}", a.name, a.value, t);
            }
        }
        AliasAction::Add { name, value } => {
            dots::aliases::add_user_alias(&personal, &name, &value)?;
            println!("✓ Added alias '{name}'");
        }
        AliasAction::Remove { name } => {
            dots::aliases::remove_user_alias(&personal, &name)?;
            println!("✓ Removed alias '{name}'");
        }
    }
    Ok(())
}

fn cli_install(name: Option<String>, all: bool, optional: bool) -> anyhow::Result<()> {
    use dots::installer::{detect_pm, PackageManager};
    use dots::packages::{check_dep, Category, DEPS};

    let pm = detect_pm();
    if matches!(pm, PackageManager::Unknown) {
        anyhow::bail!("no supported package manager found; install manually");
    }

    if let Some(n) = name {
        let dep = DEPS.iter().find(|d| d.bin == n || d.brew == n)
            .ok_or_else(|| anyhow::anyhow!("unknown package '{n}' — not in dots dependency list"))?;
        println!("Installing {n}…");
        dots::installer::install_dep(dep)?;
        println!("✓ {n} installed");
    } else if all {
        let missing: Vec<_> = DEPS.iter()
            .filter(|d| d.category == Category::Required && !check_dep(d))
            .collect();
        if missing.is_empty() { println!("✓ All core deps installed"); return Ok(()); }
        for dep in missing {
            println!("Installing {}…", dep.bin);
            dots::installer::install_dep(dep)?;
        }
    } else if optional {
        let missing: Vec<_> = DEPS.iter()
            .filter(|d| d.category == Category::Optional && !check_dep(d))
            .collect();
        if missing.is_empty() { println!("✓ All optional deps installed"); return Ok(()); }
        for dep in missing {
            println!("Installing {}…", dep.bin);
            dots::installer::install_dep(dep)?;
        }
    } else {
        println!("Usage: dots install <name>  |  dots install --all  |  dots install --optional");
    }
    Ok(())
}

fn cli_premade(action: PremadeAction) -> anyhow::Result<()> {
    use dots::installer::PREMADE_CONFIGS;
    use dots::config::settings::dots_dir;

    match action {
        PremadeAction::List => {
            println!("{:<12} {}", "APP", "DESCRIPTION");
            println!("{}", "-".repeat(60));
            for p in PREMADE_CONFIGS {
                println!("{:<12} {}", p.app, p.description);
            }
        }
        PremadeAction::Apply { app } => {
            let entry = PREMADE_CONFIGS.iter().find(|p| p.app == app)
                .ok_or_else(|| anyhow::anyhow!("unknown premade config '{app}'"))?;
            let dots = dots_dir();
            dots::installer::apply_premade(&dots, entry)?;
            println!("✓ {} config applied (backup created if existed)", app);
        }
    }
    Ok(())
}

fn cli_profile(action: ProfileAction) -> anyhow::Result<()> {
    use dots::config::profile::{
        apply_personal_config, fetch_github_raw, generate_personal_config,
        load_from_value, personal_config_path, validate_personal_config,
    };

    match action {
        ProfileAction::Generate { path } => {
            let path = path.unwrap_or_else(personal_config_path);
            generate_personal_config(&path)?;
            println!("✓ Saved to {}", path.display());
        }
        ProfileAction::Import { path } => {
            if !path.exists() {
                anyhow::bail!("file not found: {}", path.display());
            }
            let text = std::fs::read_to_string(&path)?;
            let v: serde_json::Value = serde_json::from_str(&text)?;
            validate_personal_config(&v)?;
            let cfg  = load_from_value(&v)?;
            let miss = apply_personal_config(&cfg)?;
            if miss.is_empty() {
                println!("✓ Config applied — all packages present");
            } else {
                println!("✓ Config applied — run 'dots install --optional' for: {}", miss.join(", "));
            }
        }
        ProfileAction::ImportGit { spec } => {
            println!("Fetching {spec}…");
            let text = fetch_github_raw(&spec)?;
            let v: serde_json::Value = serde_json::from_str(&text)?;
            validate_personal_config(&v)?;
            let cfg  = load_from_value(&v)?;
            let miss = apply_personal_config(&cfg)?;
            if miss.is_empty() {
                println!("✓ Config applied from GitHub");
            } else {
                println!("✓ Config applied — run 'dots install --optional' for: {}", miss.join(", "));
            }
        }
    }
    Ok(())
}

fn cli_init(quiet: bool) -> anyhow::Result<()> {
    use dots::config::personal::ensure_personal_dir;
    use dots::config::settings::{load, save, settings_path};

    // Create settings.toml with defaults if absent
    let spath = settings_path();
    if !spath.exists() {
        let defaults = load().unwrap_or_default();
        save(&defaults)?;
        if !quiet { println!("✓ Created settings.toml"); }
    }

    // Create ~/.personal/ if absent
    ensure_personal_dir()?;
    if !quiet { println!("✓ Personal directory ready"); }

    Ok(())
}

// ── TUI entry points ──────────────────────────────────────────────────────────

fn run_tui(start: dots::tui::app::Screen) -> anyhow::Result<()> {
    let settings = dots::config::settings::load().unwrap_or_default();

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        prev_hook(info);
    }));

    enable_raw_mode().context("could not enter raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend  = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
        }
    }
    let _guard = Guard;

    dots::tui::app::run(&mut term, start, &settings)?;
    Ok(())
}
