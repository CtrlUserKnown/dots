use std::io;

use anyhow::Context;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

#[derive(Parser)]
#[command(name = "dots", version = env!("DOTS_VERSION"), about = "dots dotfiles manager")]
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
    /// View and (un)install app configs discovered in your dotfiles repo
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Export/import personal config
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Manage your own config symlinks (GNU Stow style)
    Link {
        #[command(subcommand)]
        action: LinkAction,
    },
    /// Install packages declared in ~/.dots/packages.toml
    Packages {
        #[command(subcommand)]
        action: PackagesAction,
    },
    /// Pull your dotfiles repo into ~/.dots/src (user/repo or full git URL)
    #[command(visible_alias = "gc")]
    GetConfig {
        /// Repo to pull: user/repo, user/repo@ref, or a full git URL
        #[arg(short = 'u', long = "url", value_name = "SPEC")]
        url: String,
        /// After pulling, install all configs and packages it declares
        #[arg(long)]
        apply: bool,
        /// Skip the confirmation prompt before applying
        #[arg(long)]
        yes: bool,
        /// Replace an existing source repo at ~/.dots/src
        #[arg(long)]
        force: bool,
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
    /// Apply a premade config (turn it on)
    Apply { app: String },
    /// Remove a premade config (turn it off, restoring any backup)
    Remove { app: String },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// List discovered configs and their install status
    List,
    /// List the files a config links (with per-file status)
    View { name: String },
    /// Install a config by name, or all of them with --all
    Install {
        /// Config to install (omit when using --all)
        name: Option<String>,
        /// Install every discovered config
        #[arg(long)]
        all: bool,
    },
    /// Remove a config (unlink only; real files are untouched)
    Remove { name: String },
    /// Adopt an existing dotfile into ~/.dots/src and link it back
    Add {
        /// File or directory to add (e.g. ~/.config/nvim)
        path: std::path::PathBuf,
        /// Config name to file it under (inferred from the path if omitted)
        #[arg(long)]
        app: Option<String>,
    },
    /// Pull the latest source repo and re-apply installed configs
    Sync,
}

#[derive(Subcommand)]
enum LinkAction {
    /// Adopt an existing file/dir and symlink it (records it in links.toml)
    Add {
        /// Where the real file lives (or will be moved to, if adopting)
        source: std::path::PathBuf,
        /// Where the symlink is created
        target: std::path::PathBuf,
    },
    /// List declared links and their status
    List,
    /// Remove a declared link and its symlink
    Remove {
        /// The symlink path to remove
        target: std::path::PathBuf,
    },
    /// Create or repair all declared links
    Apply,
}

#[derive(Subcommand)]
enum PackagesAction {
    /// Install everything in packages.toml that matches this system
    Install {
        /// Print what would run without executing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Show the resolved plan for this system without installing (alias for `install --dry-run`)
    Plan,
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
        Some(Command::Config { action }) => cli_config(action)?,
        Some(Command::Profile { action }) => cli_profile(action)?,
        Some(Command::Link { action }) => cli_link(action)?,
        Some(Command::Packages { action }) => cli_packages(action)?,
        Some(Command::GetConfig { url, apply, yes, force }) => {
            cli_get_config(url, apply, yes, force)?
        }
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
            println!("{:<20} {:<40} TYPE", "NAME", "COMMAND");
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

fn cli_packages(action: PackagesAction) -> anyhow::Result<()> {
    use dots::user_packages::{execute, plan_current};

    let manifest = dots::manifest::load()?.to_packages_manifest();
    let plan = plan_current(&manifest);

    let dry_run = match action {
        PackagesAction::Plan => true,
        PackagesAction::Install { dry_run } => dry_run,
    };

    if plan.actions.is_empty() && plan.skips.is_empty() {
        println!("Nothing declared for this system in {}.", dots::manifest::manifest_path().display());
        return Ok(());
    }
    if dry_run {
        println!("Plan for this system (nothing will be executed):");
    }
    execute(&plan, dry_run)?;
    if !dry_run {
        println!("✓ done");
    }
    Ok(())
}

fn cli_get_config(url: String, apply: bool, yes: bool, force: bool) -> anyhow::Result<()> {
    use dots::config::settings::dots_dir;
    use dots::configs;
    use dots::import::{clone_source, resolve};
    use dots::user_packages::{execute, plan_current};

    let source = resolve(&url)?;
    let src = dots_dir().join("src");
    if src.exists() && !force {
        anyhow::bail!(
            "a source repo already exists at {} — pass --force to replace it",
            src.display()
        );
    }
    let ref_note = source.reference.as_deref().map(|r| format!("@{r}")).unwrap_or_default();
    println!("Pulling '{}' from {}{}…", source.name, source.url, ref_note);
    let repo = clone_source(&source, force)?;
    println!("✓ pulled to {}", repo.display());

    // Now that ~/.dots/src exists, discovery and the unified manifest see it.
    let cfgs = configs::discover();
    println!("\nConfigs found ({}):", cfgs.len());
    for c in &cfgs {
        println!("  {:<20} {}", c.name, c.status.badge());
    }

    let plan = plan_current(&dots::manifest::load()?.to_packages_manifest());
    if !plan.actions.is_empty() || !plan.skips.is_empty() {
        println!("\nPackage plan (review — [script] steps run arbitrary code):");
        execute(&plan, true)?; // preview
    }

    if !apply {
        println!(
            "\nNothing applied. Run with --apply to set everything up, or use\n  \
             'dots config install --all' and 'dots packages install' individually."
        );
        return Ok(());
    }
    if !yes && !confirm("\nInstall all configs and packages now?") {
        println!("Aborted — nothing applied.");
        return Ok(());
    }

    println!("\nApplying…");
    let mut links = 0;
    for c in &cfgs {
        let r = configs::install(c)?;
        links += r.ok + r.repaired;
    }
    println!("  ✓ {} config(s), {links} link(s)", cfgs.len());
    if !plan.actions.is_empty() {
        execute(&plan, false)?;
    }
    println!("✓ done");
    Ok(())
}

/// Prompt for a yes/no answer on stdin, defaulting to no.
fn confirm(prompt: &str) -> bool {
    use std::io::Write;
    print!("{prompt} [y/N] ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

fn cli_premade(action: PremadeAction) -> anyhow::Result<()> {
    use dots::installer::PREMADE_CONFIGS;

    match action {
        PremadeAction::List => {
            println!("{:<12} DESCRIPTION", "APP");
            println!("{}", "-".repeat(60));
            for p in PREMADE_CONFIGS {
                println!("{:<12} {}", p.app, p.description);
            }
        }
        PremadeAction::Apply { app } => {
            let entry = PREMADE_CONFIGS.iter().find(|p| p.app == app)
                .ok_or_else(|| anyhow::anyhow!("unknown premade config '{app}'"))?;
            dots::installer::apply_premade(entry)?;
            println!("✓ {} config applied (backup created if existed)", app);
        }
        PremadeAction::Remove { app } => {
            let entry = PREMADE_CONFIGS.iter().find(|p| p.app == app)
                .ok_or_else(|| anyhow::anyhow!("unknown premade config '{app}'"))?;
            dots::installer::remove_premade(entry)?;
            println!("✓ {} config removed (backup restored if existed)", app);
        }
    }
    Ok(())
}

fn cli_config(action: ConfigAction) -> anyhow::Result<()> {
    use dots::configs;

    let find = |name: &str| -> anyhow::Result<configs::Config> {
        configs::discover().into_iter().find(|c| c.name == name)
            .ok_or_else(|| anyhow::anyhow!("unknown config '{name}' (try 'dots config list')"))
    };

    match action {
        ConfigAction::List => {
            let cfgs = configs::discover();
            if cfgs.is_empty() {
                match configs::configs_root() {
                    Some(root) => println!("No config directories in {}", root.display()),
                    None => println!("No dotfiles configs directory found (set a [stow] dir in links.toml, or create ~/dotfiles)."),
                }
                return Ok(());
            }
            for c in &cfgs {
                // Left-aligned name, right-aligned badge — the requested look.
                println!("{:<20} {:>18}", c.name, c.status.badge());
            }
        }
        ConfigAction::View { name } => {
            let c = find(&name)?;
            println!("{} — {}", c.name, c.dir.display());
            if c.links.is_empty() {
                println!("  (no linkable files)");
            }
            for link in &c.links {
                let mark = match dots::symlinks::check(link) {
                    dots::symlinks::SymlinkStatus::Ok => "✓",
                    dots::symlinks::SymlinkStatus::Missing => "·",
                    _ => "✗",
                };
                println!("  {mark} {}", link.link.display());
            }
        }
        ConfigAction::Install { name, all } => {
            if all {
                let cfgs = configs::discover();
                if cfgs.is_empty() {
                    println!("No configs found.");
                    return Ok(());
                }
                let mut links = 0;
                for c in &cfgs {
                    let r = configs::install(c)?;
                    links += r.ok + r.repaired;
                    println!("  ✓ {}", c.name);
                }
                println!("✓ installed {} config(s), {links} link(s)", cfgs.len());
            } else if let Some(name) = name {
                let c = find(&name)?;
                let r = configs::install(&c)?;
                println!("✓ {} installed ({} linked, {} skipped)", name, r.repaired + r.ok, r.skipped);
            } else {
                println!("Usage: dots config install <name>  |  dots config install --all");
            }
        }
        ConfigAction::Remove { name } => {
            let c = find(&name)?;
            let n = configs::remove(&c)?;
            println!("✓ {} removed ({n} link(s) unlinked)", name);
        }
        ConfigAction::Add { path, app } => {
            let msg = configs::add(&path, app.as_deref())?;
            println!("{msg}");
        }
        ConfigAction::Sync => {
            let msg = configs::sync()?;
            println!("✓ {msg}");
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

fn cli_link(action: LinkAction) -> anyhow::Result<()> {
    use dots::links;
    use dots::symlinks::{check, SymlinkStatus};

    match action {
        LinkAction::Add { source, target } => {
            links::add_link(&source, &target)?;
            println!("✓ Linked {} → {}", target.display(), source.display());
        }
        LinkAction::List => {
            let syms = links::planned_symlinks();
            if syms.is_empty() {
                println!("No declared links.");
                println!("Add one with 'dots link add <source> <target>', or edit {}.",
                    links::manifest_path().display());
                return Ok(());
            }
            println!("{:<40} {:<40} STATUS", "LINK", "SOURCE");
            println!("{}", "-".repeat(90));
            for s in &syms {
                let status = match check(s) {
                    SymlinkStatus::Ok => "ok",
                    SymlinkStatus::Missing => "missing",
                    SymlinkStatus::Broken => "broken",
                    SymlinkStatus::NotALink => "not a link",
                    SymlinkStatus::WrongTarget => "wrong target",
                };
                println!("{:<40} {:<40} {}", s.link.display(), s.target.display(), status);
            }
        }
        LinkAction::Remove { target } => {
            links::remove_link(&target)?;
            println!("✓ Removed link {}", target.display());
        }
        LinkAction::Apply => {
            let report = dots::symlinks::repair_list(&links::planned_symlinks());
            println!("  {} OK, {} repaired, {} skipped", report.ok, report.repaired, report.skipped);
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
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        prev_hook(info);
    }));

    enable_raw_mode().context("could not enter raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend  = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        }
    }
    let _guard = Guard;

    dots::tui::app::run(&mut term, start, &settings)?;
    Ok(())
}
