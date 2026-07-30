use std::io::Stdout;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{
    Frame,
    backend::CrosstermBackend,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Terminal,
};

use crate::config::settings::Settings;
use crate::plugins::{PluginHost, PluginPaneView};
use crate::tui::theme::style_error;
use crate::tui::{draw_banner, draw_desc, draw_key_bar, draw_top_bar, FlashKind};
use crate::update::{self, InstallSource, UpdateInfo};
use crate::zones::{self, Layout, PanePlacement, Resolved};

use super::aliases::{handle_alias_key, render_aliases, AliasView};
use super::blocks::{handle_blocks_key, render_blocks, BlocksView};
use super::configs::ConfigsView;
use super::health::{HealthView, Scope as HealthScope};
use super::overview::DashCache;
use super::profile::{handle_profile_key, render_profile, ProfileView};
use super::settings::{handle_settings_key, render_settings, render_theme, handle_theme_key, SettingsView, ThemeView};
use super::update::UpdateScreen;

/// How often the background thread in [`run`] recomputes [`App::dash_cache`].
const DASH_CACHE_INTERVAL: Duration = Duration::from_secs(2);

/// How long [`App::update_banner`] stays on screen before clearing itself.
const UPDATE_BANNER_LIFETIME: Duration = Duration::from_secs(5);

// ── screen enum ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main,
    /// Managed links into `$HOME`.
    Symlinks,
    /// CLI dependencies and zsh plugins. Separate from [`Screen::Symlinks`]:
    /// the two drill-downs share an implementation, not a screen.
    Tools,
    Aliases,
    Configs,
    Profile,
    Theme,
    /// Also where the update check/apply flow lives, as a row in the popup.
    Settings,
    /// Reassigns which widgets sit in which dashboard zone. Reached only
    /// from the Settings popup — not a `NAV` member, so it stays modal (see
    /// `is_modal`) rather than risking a stray digit press teleporting the
    /// user out mid-edit.
    Blocks,
}

// ── app state ─────────────────────────────────────────────────────────────────

pub struct App {
    pub should_quit:    bool,
    pub screen:         Screen,
    pub flash:          Option<(String, FlashKind)>,
    pub dash_focus:     usize,
    /// The newest release, if a background check found one newer than us.
    pub update_info:    Option<UpdateInfo>,
    pub update_error:   Option<String>,
    /// A transient "update available" notice and when it was shown — cleared a
    /// few seconds after it appears (see [`UPDATE_BANNER_LIFETIME`]), separate
    /// from `flash` so it isn't clobbered by the next keypress's flash message.
    pub update_banner:  Option<(String, Instant)>,
    /// How this binary was installed — gates whether self-update is offered.
    pub install_source: InstallSource,
    pub network:        Option<crate::network::NetworkStatus>,
    pub settings:       Settings,
    /// Cached snapshots of plugin-registered dashboard panes.
    pub plugin_panes:   Vec<PluginPaneView>,
    /// Per-file load report for the Lua plugins in `~/.dots/plugins/` — static
    /// for the session, snapshotted once at startup. Drives the Plugins tile.
    pub plugin_infos:   Vec<crate::plugins::PluginInfo>,
    /// The dashboard's zone layout, from `~/.dots/layout.toml` or the default.
    pub layout:         Layout,
    /// Last snapshot of the Symlinks/Tools/Configs tiles' data — recomputed
    /// on a background thread every [`DASH_CACHE_INTERVAL`] rather than on
    /// every frame (see [`DashCache`]).
    pub dash_cache:     DashCache,
    /// [`App::layout`] resolved against the current plugin panes — the concrete
    /// zone/widget arrangement the dashboard draws and navigates.
    pub dash:           Resolved,
    /// Set when a plugin pane is activated; the event loop runs its `on_enter`.
    pub pending_plugin_enter: Option<usize>,
}

impl App {
    pub fn new(start: Screen, settings: Settings) -> Self {
        Self {
            should_quit:    false,
            screen:         start,
            flash:          None,
            dash_focus:     0,
            update_info:    None,
            update_error:   None,
            update_banner:  None,
            install_source: update::install_source(),
            network:        None,
            settings,
            plugin_panes:   Vec::new(),
            plugin_infos:   Vec::new(),
            layout:         Layout::default(),
            dash_cache:     DashCache::default(),
            dash:           Resolved::default(),
            pending_plugin_enter: None,
        }
    }

    /// Re-resolve the layout against the current plugin panes. Called whenever
    /// either side changes — at startup, and after a plugin tick adds or
    /// renames panes — because a pane's zone placement is decided here, not at
    /// draw time.
    pub fn rebuild_dashboard(&mut self) {
        let placements: Vec<PanePlacement> = self
            .plugin_panes
            .iter()
            .map(|p| PanePlacement { id: &p.id, zone: p.zone.as_deref() })
            .collect();
        self.dash = zones::resolve(&self.layout, &placements);
        // Keep focus addressable when the widget count shrinks (a plugin whose
        // pane disappeared, say) rather than pointing past the end.
        let last = self.dash.len().saturating_sub(1);
        self.dash_focus = self.dash_focus.min(last);
    }
}

// ── screen views ──────────────────────────────────────────────────────────────

/// Every sibling screen's own state, bundled so the dispatch functions below
/// take one `&mut Views` instead of a growing list of `&mut FooView` — each
/// new screen used to mean a new parameter threaded through `render`,
/// `handle_key`, `handle_mouse`, `navigate_to`, and everything they call.
#[derive(Default)]
pub struct Views {
    pub health:   HealthView,
    pub configs:  ConfigsView,
    pub aliases:  AliasView,
    pub profile:  ProfileView,
    pub update:   UpdateScreen,
    pub settings: SettingsView,
    pub theme:    ThemeView,
    pub blocks:   BlocksView,
}

// ── navigation ────────────────────────────────────────────────────────────────

/// The sibling screens, in the order they appear in the nav strip and answer to
/// the number keys. One ordered list drives the tab strip, `[`/`]` cycling, and
/// the digit shortcuts, so those three can never disagree about what screen 3 is.
///
/// The dashboard is not a member: it is the home you `esc` back to, not a tab.
/// Settings is not either — it is a popup over whatever is behind it.
pub const NAV: &[(Screen, &str)] = &[
    (Screen::Symlinks, "symlinks"),
    (Screen::Tools,    "tools"),
    (Screen::Configs,  "configs"),
    (Screen::Aliases,  "aliases"),
    (Screen::Profile,  "profile"),
    (Screen::Theme,    "theme"),
];

/// The nav strip for `screen`, as `(label, is_active)` pairs.
pub fn nav_tabs(screen: Screen) -> Vec<(&'static str, bool)> {
    NAV.iter().map(|&(s, label)| (label, s == screen)).collect()
}

fn nav_index(screen: Screen) -> Option<usize> {
    NAV.iter().position(|&(s, _)| s == screen)
}

/// The screen `delta` steps away in the nav ring, wrapping at both ends. From
/// the dashboard, stepping forward enters the ring at its first screen.
fn nav_step(screen: Screen, delta: isize) -> Screen {
    let n = NAV.len() as isize;
    let cur = nav_index(screen).map(|i| i as isize).unwrap_or(if delta > 0 { -1 } else { 0 });
    NAV[(((cur + delta) % n + n) % n) as usize].0
}

// ── event loop ────────────────────────────────────────────────────────────────

pub fn run(
    terminal:  &mut Terminal<CrosstermBackend<Stdout>>,
    start:     Screen,
    settings:  &Settings,
) -> anyhow::Result<()> {
    let mut app   = App::new(start, settings.clone());
    let mut views = Views::default();

    // Load Lua plugins and seed the dashboard with their panes and zones. The
    // host stays on this thread (its Lua state is not Send).
    let mut plugin_host = PluginHost::load();
    plugin_host.tick();
    app.plugin_panes = plugin_host.views();
    app.plugin_infos = plugin_host.plugins().to_vec();

    // The layout file is the user's; plugin-declared zones only fill gaps in it.
    // A layout that won't parse is reported and skipped: the user needs a
    // working dashboard in order to go fix it.
    let user_layout = match Layout::try_load() {
        Ok(l) => l,
        Err(e) => {
            app.flash = Some((
                format!("layout.toml: {e} — using the default layout"),
                FlashKind::Error,
            ));
            None
        }
    };
    app.layout = user_layout.clone().unwrap_or_default();
    app.layout.merge_plugin_zones(plugin_host.zones());

    // `ui.layout{columns=N}` predates zones, where it meant "columns of dashboard
    // tiles". Zones took that meaning over, so honour the old call only while the
    // user has not written a layout of their own — then it still reflows the
    // default zone's tiles, and a hand-written layout is never overridden.
    if let (None, Some(n)) = (&user_layout, plugin_host.columns()) {
        for z in &mut app.layout.zones {
            z.columns = n as u16;
        }
    }
    app.rebuild_dashboard();
    app.dash_cache = DashCache::compute();

    if start == Screen::Settings {
        views.settings.load_from(&app.settings);
        views.update.sync_from_app(&app);
    }
    if start == Screen::Theme { views.theme.load(); }

    // Kick off the background release check, unless disabled or this binary is
    // managed by a package manager (then updating is the manager's job). The
    // spawned thread honours the throttle stamp internally.
    let update_rx = (app.settings.dots.update_check
        && app.install_source == InstallSource::SelfManaged)
        .then(|| update::spawn_check(app.settings.dots.update_frequency));

    // Spawn the live network monitor: probe now, then refresh every few seconds.
    // Sending fails once the receiver is dropped (app exit), which ends the loop.
    let (net_tx, net_rx) = std::sync::mpsc::channel::<crate::network::NetworkStatus>();
    std::thread::spawn(move || loop {
        if net_tx.send(crate::network::probe()).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_secs(5));
    });

    // Same pattern for the Symlinks/Tools/Configs tiles: `DashCache::compute`
    // reads the symlink manifest, spawns a `which` per dependency, and scans
    // the configs directory, so it runs here rather than in the render path.
    let (dash_tx, dash_rx) = std::sync::mpsc::channel::<DashCache>();
    std::thread::spawn(move || loop {
        std::thread::sleep(DASH_CACHE_INTERVAL);
        if dash_tx.send(DashCache::compute()).is_err() {
            break;
        }
    });

    loop {
        views.update.pump(&mut app);
        views.health.try_complete_install();

        if let Some((_, shown_at)) = &app.update_banner {
            if shown_at.elapsed() >= UPDATE_BANNER_LIFETIME {
                app.update_banner = None;
            }
        }

        // Refresh any plugin panes whose interval has elapsed.
        if plugin_host.tick() {
            app.plugin_panes = plugin_host.views();
            app.rebuild_dashboard();
        }

        // Drain the latest network snapshot (keep only the freshest).
        while let Ok(status) = net_rx.try_recv() {
            app.network = Some(status);
        }

        // Same for the dashboard data cache.
        while let Ok(cache) = dash_rx.try_recv() {
            app.dash_cache = cache;
        }

        // Fold in the initial release check the moment it lands.
        if let Some(rx) = &update_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(Some(info)) => {
                        if app.screen != Screen::Settings {
                            app.flash = Some((
                                format!("Update available: v{} — see settings (4)", info.latest),
                                FlashKind::Info,
                            ));
                        }
                        app.update_banner = Some((
                            format!("Update available: v{}", info.latest),
                            Instant::now(),
                        ));
                        app.update_info = Some(info);
                    }
                    Ok(None) => {}
                    Err(e)   => app.update_error = Some(e.to_string()),
                }
            }
        }

        terminal.draw(|f| render(f, &app, &views))?;

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => handle_key(&mut app, &mut views, key),
                Event::Mouse(me) => {
                    let area = terminal.size().map(|s| Rect::new(0, 0, s.width, s.height)).unwrap_or_default();
                    handle_mouse(&mut app, &mut views, me, area);
                }
                Event::Resize(..) => {}
                _ => {}
            }
        }

        // A plugin pane was activated: run its handler, then show the result.
        if let Some(i) = app.pending_plugin_enter.take() {
            plugin_host.on_enter(i);
            plugin_host.tick();
            app.plugin_panes = plugin_host.views();
            app.rebuild_dashboard();
        }

        if app.should_quit { break; }
    }

    Ok(())
}

// ── rendering dispatch ────────────────────────────────────────────────────────

fn render(f: &mut Frame, app: &App, views: &Views) {
    let area = f.area();
    if area.width < 50 || area.height < 14 {
        render_too_small(f, area);
        return;
    }
    match app.screen {
        Screen::Main     => render_main(f, area, app),
        Screen::Symlinks | Screen::Tools => super::health::render(f, area, app, &views.health),
        Screen::Configs  => super::configs::render(f, area, app, &views.configs),
        Screen::Aliases  => render_aliases(f, area, app, &views.aliases),
        Screen::Profile  => render_profile(f, area, app, &views.profile),
        // Settings (which folds in updates) is a popup: draw the dashboard
        // underneath, then overlay it, mirroring ssm's which-key popup over
        // its list screen.
        Screen::Settings => {
            render_main(f, area, app);
            render_settings(f, area, app, &views.settings, &views.update);
        }
        Screen::Theme    => render_theme(f, area, app, &views.theme),
        Screen::Blocks   => render_blocks(f, area, app, &views.blocks),
    }

    if let Some((msg, _)) = &app.update_banner {
        draw_banner(f, area, msg, &FlashKind::Info);
    }
}

/// The dashboard pane-grid region within the full terminal `area`. Shared by
/// rendering and mouse hit-testing so clicks land on what's drawn.
fn dashboard_grid(area: Rect) -> Rect {
    // Content region sits below the top bar and above the desc/footer bars.
    // One blank row under the bar keeps the tiles off the title.
    let top    = area.y + 2;
    let bottom = area.y + area.height - 4; // desc bar lives at height-4
    Rect {
        x:      area.x + 1,
        y:      top,
        width:  area.width.saturating_sub(2),
        height: bottom.saturating_sub(top),
    }
}

/// Keybindings shown in the dashboard's bottom bar.
const MAIN_KEYS: &[(&str, &str)] = &[
    ("↑↓←→/hjkl", "move"),
    ("enter", "open"),
    ("1-6", "screen"),
    ("space", "settings"),
    ("q", "quit"),
];

fn render_main(f: &mut Frame, area: Rect, app: &App) {
    use super::overview;

    draw_top_bar(f, area, "dots", "dotfiles manager", &[("q", "quit"), ("space", "settings")]);

    let grid = dashboard_grid(area);
    overview::render_grid(f, grid, app, app.dash_focus);

    let hint = overview::focus_hint(app, app.dash_focus);
    draw_desc(f, area, &hint, app.flash.as_ref());
    draw_key_bar(f, area, MAIN_KEYS);
}

fn render_too_small(f: &mut Frame, area: Rect) {
    let msg = "Terminal too small — need at least 50×14";
    let y   = area.height / 2;
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(msg, style_error()))),
        Rect { x: area.x, y: area.y + y, width: area.width, height: 1 },
    );
}

// ── key dispatch ──────────────────────────────────────────────────────────────

fn handle_key(app: &mut App, views: &mut Views, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    // Cross-screen navigation, handled before the screen sees the key — but
    // only when the screen isn't holding a prompt open. A screen mid-edit owns
    // every keystroke, or typing "3" into a path would teleport you to configs.
    if !is_modal(app, views) {
        let target = match key.code {
            KeyCode::Char(']') | KeyCode::Tab      => Some(nav_step(app.screen, 1)),
            KeyCode::Char('[') | KeyCode::BackTab  => Some(nav_step(app.screen, -1)),
            KeyCode::Char(c @ '1'..='9') => {
                NAV.get(c as usize - '1' as usize).map(|&(s, _)| s)
            }
            _ => None,
        };
        if let Some(screen) = target {
            navigate_to(app, views, screen);
            return;
        }
    }

    match app.screen {
        Screen::Main     => handle_main_key(app, views, key),
        Screen::Symlinks | Screen::Tools => super::health::handle_key(app, &mut views.health, key),
        Screen::Configs  => super::configs::handle_key(app, &mut views.configs, key),
        Screen::Aliases  => handle_alias_key(app, &mut views.aliases, key),
        Screen::Profile  => handle_profile_key(app, &mut views.profile, key),
        Screen::Settings => handle_settings_key(app, &mut views.settings, &mut views.theme, &mut views.blocks, &mut views.update, key),
        Screen::Theme    => handle_theme_key(app, &mut views.theme, key),
        Screen::Blocks   => handle_blocks_key(app, &mut views.blocks, key),
    }
}

fn handle_main_key(app: &mut App, views: &mut Views, key: KeyEvent) {
    use super::overview::{self, Dir};

    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('h') | KeyCode::Left  => app.dash_focus = overview::move_focus(app, app.dash_focus, Dir::Left),
        KeyCode::Char('l') | KeyCode::Right => app.dash_focus = overview::move_focus(app, app.dash_focus, Dir::Right),
        KeyCode::Char('j') | KeyCode::Down  => app.dash_focus = overview::move_focus(app, app.dash_focus, Dir::Down),
        KeyCode::Char('k') | KeyCode::Up    => app.dash_focus = overview::move_focus(app, app.dash_focus, Dir::Up),
        // Digits and `[`/`]` are handled globally in `handle_key`, so the
        // dashboard doesn't bind them itself — one table drives them everywhere.
        KeyCode::Char(' ') => navigate_to(app, views, Screen::Settings),
        KeyCode::Enter => activate_focus(app, views, app.dash_focus),
        _ => {}
    }
}

/// True when the active screen owns every keystroke and global navigation must
/// keep its hands off: a text prompt, a confirmation, an open pager, or the
/// settings popup (which holds unsaved edits until `esc` writes them).
fn is_modal(app: &App, views: &Views) -> bool {
    // Blocks has no `NAV` slot of its own (unlike Theme), so — like Settings —
    // it stays modal unconditionally: an un-modal Blocks screen would let a
    // stray digit press silently teleport the user out via the leading
    // digit-key check in `handle_key`, and there's no sane target for that.
    if app.screen == Screen::Settings || app.screen == Screen::Blocks {
        return true;
    }
    match app.screen {
        Screen::Symlinks | Screen::Tools => views.health.is_busy(),
        Screen::Configs => views.configs.is_capturing(),
        Screen::Aliases => views.aliases.is_capturing(),
        Screen::Profile => views.profile.is_capturing(),
        _ => false,
    }
}

/// Open whatever dashboard widget sits at `focus`: drill into a built-in's
/// screen, or hand a plugin pane off to the host in the event loop. Shared by
/// the enter key and left-click so both do exactly the same thing.
fn activate_focus(app: &mut App, views: &mut Views, focus: usize) {
    use super::overview::{focused, Focus};

    match focused(app, focus) {
        Some(Focus::Builtin(pane)) => navigate_to(app, views, pane.target()),
        Some(Focus::Plugin(i)) => app.pending_plugin_enter = Some(i),
        None => {}
    }
}

fn handle_mouse(app: &mut App, views: &mut Views, me: MouseEvent, area: Rect) {
    use super::overview;

    match me.kind {
        // Wheel scroll drives vertical navigation on whatever screen is up,
        // by reusing each screen's existing up/down key handling.
        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
            let code = if matches!(me.kind, MouseEventKind::ScrollDown) { KeyCode::Down } else { KeyCode::Up };
            let key = KeyEvent::new(code, KeyModifiers::NONE);
            handle_key(app, views, key);
        }
        // Left-click on a dashboard pane focuses and opens it. The 50×14 guard
        // matches the threshold below which the dashboard isn't drawn at all.
        MouseEventKind::Down(MouseButton::Left)
            if app.screen == Screen::Main && area.width >= 50 && area.height >= 14 =>
        {
            let grid = dashboard_grid(area);
            if let Some(i) = overview::pane_at(grid, app, me.column, me.row) {
                app.dash_focus = i;
                activate_focus(app, views, i);
            }
        }
        _ => {}
    }
}

fn navigate_to(app: &mut App, views: &mut Views, screen: Screen) {
    match screen {
        Screen::Symlinks => {
            views.health.show(HealthScope::Symlinks);
            app.screen = Screen::Symlinks;
            app.flash  = None;
        }
        Screen::Tools => {
            views.health.show(HealthScope::Tools);
            app.screen = Screen::Tools;
            app.flash  = None;
        }
        Screen::Configs => {
            views.configs.reload();
            app.screen = Screen::Configs;
            app.flash  = None;
        }
        Screen::Aliases => {
            views.aliases.reload();
            app.screen = Screen::Aliases;
            app.flash  = None;
        }
        Screen::Profile => {
            views.profile.reset();
            app.screen = Screen::Profile;
            app.flash  = None;
        }
        Screen::Settings => {
            views.settings.load_from(&app.settings);
            views.update.sync_from_app(app);
            app.screen = Screen::Settings;
            app.flash  = None;
        }
        Screen::Theme => {
            views.theme.load();
            app.screen = Screen::Theme;
            app.flash  = None;
        }
        Screen::Blocks => {
            views.blocks.load();
            app.screen = Screen::Blocks;
            app.flash  = None;
        }
        other => {
            app.screen = other;
            app.flash  = None;
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// The nav strip, the digit keys, and `[`/`]` all read the same table, so
    /// they can never disagree about which screen is which.
    #[test]
    fn nav_table_drives_every_route_into_it() {
        for (i, &(screen, label)) in NAV.iter().enumerate() {
            let tabs = nav_tabs(screen);
            assert_eq!(tabs.len(), NAV.len());
            assert!(tabs[i].1, "{label} should be the active tab on its own screen");
            assert_eq!(tabs.iter().filter(|(_, active)| *active).count(), 1);
        }
    }

    #[test]
    fn stepping_wraps_in_both_directions() {
        let first = NAV[0].0;
        let last = NAV[NAV.len() - 1].0;
        assert_eq!(nav_step(first, -1), last, "stepping back from the first wraps to the last");
        assert_eq!(nav_step(last, 1), first, "stepping past the last wraps to the first");
        assert_eq!(nav_step(first, 1), NAV[1].0);
    }

    /// The dashboard isn't a tab, so stepping forward from it enters the ring
    /// at the start rather than falling off it.
    #[test]
    fn stepping_from_the_dashboard_enters_the_ring() {
        assert_eq!(nav_step(Screen::Main, 1), NAV[0].0);
        assert_eq!(nav_step(Screen::Main, -1), NAV[NAV.len() - 1].0);
        assert!(nav_tabs(Screen::Main).iter().all(|(_, active)| !active));
    }

    /// The load-bearing guard: a screen holding a prompt owns every key, or
    /// typing a digit into a path would teleport the user mid-edit.
    #[test]
    fn a_prompt_is_modal_and_global_nav_stays_out() {
        let mut app = App::new(Screen::Aliases, crate::config::settings::Settings::default());
        let mut views = Views::default();
        assert!(!is_modal(&app, &views), "the list is not modal");

        views.aliases.mode = super::super::aliases::AliasMode::Search { query: "3".into() };
        assert!(is_modal(&app, &views), "a search box must keep its keys");

        // The settings popup holds unsaved edits until esc writes them.
        app.screen = Screen::Settings;
        let views = Views::default();
        assert!(is_modal(&app, &views));

        // Blocks has no `NAV` slot, so it must stay modal too — otherwise a
        // stray digit press would silently bounce the user to another screen
        // mid-edit, with nowhere sane on the ring for Blocks itself to go back to.
        app.screen = Screen::Blocks;
        let views = Views::default();
        assert!(is_modal(&app, &views));
    }

    #[test]
    fn every_nav_screen_is_reachable_by_its_digit() {
        for (i, &(screen, _)) in NAV.iter().enumerate() {
            let digit = char::from_digit(i as u32 + 1, 10).unwrap();
            let picked = NAV.get(digit as usize - '1' as usize).map(|&(s, _)| s);
            assert_eq!(picked, Some(screen));
        }
        assert!(NAV.len() <= 9, "digits only reach nine screens");
    }
}
