use std::io::Stdout;
use std::time::Duration;

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
use crate::tui::theme::style_error;
use crate::tui::{draw_desc, draw_footer, draw_header, FlashKind};
use crate::update::{self, InstallSource, UpdateInfo};

use super::aliases::{handle_alias_key, render_aliases, AliasView};
use super::configs::ConfigsView;
use super::health::HealthView;
use super::profile::{handle_profile_key, render_profile, ProfileView};
use super::settings::{handle_settings_key, render_settings, render_theme, handle_theme_key, SettingsView, ThemeView};
use super::update::UpdateScreen;

const VERSION: &str = env!("DOTS_VERSION");

// ── screen enum ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main,
    Health,
    Aliases,
    Configs,
    Profile,
    Theme,
    Settings,
    Update,
}

// ── app state ─────────────────────────────────────────────────────────────────

pub struct App {
    pub should_quit:    bool,
    pub screen:         Screen,
    pub flash:          Option<(String, FlashKind)>,
    pub menu_idx:       usize,
    pub dash_focus:     usize,
    /// The newest release, if a background check found one newer than us.
    pub update_info:    Option<UpdateInfo>,
    pub update_error:   Option<String>,
    /// How this binary was installed — gates whether self-update is offered.
    pub install_source: InstallSource,
    pub network:        Option<crate::network::NetworkStatus>,
    pub settings:       Settings,
}

impl App {
    pub fn new(start: Screen, settings: Settings) -> Self {
        Self {
            should_quit:    false,
            screen:         start,
            flash:          None,
            menu_idx:       0,
            dash_focus:     0,
            update_info:    None,
            update_error:   None,
            install_source: update::install_source(),
            network:        None,
            settings,
        }
    }
}

// ── main menu ─────────────────────────────────────────────────────────────────

/// Screens reachable by number key from the dashboard (1 = first). Health and
/// Update are omitted here because they are opened by drilling into their panes.
const MAIN_MENU: &[Screen] = &[
    Screen::Aliases,
    Screen::Profile,
    Screen::Theme,
    Screen::Settings,
];

// ── event loop ────────────────────────────────────────────────────────────────

pub fn run(
    terminal:  &mut Terminal<CrosstermBackend<Stdout>>,
    start:     Screen,
    settings:  &Settings,
) -> anyhow::Result<()> {
    let mut app           = App::new(start, settings.clone());
    let mut health_view   = HealthView::new();
    let mut configs_view  = ConfigsView::new();
    let mut alias_view    = AliasView::new();
    let mut profile_view  = ProfileView::new();
    let mut update_screen = UpdateScreen::new();
    let mut settings_view = SettingsView::new();
    let mut theme_view    = ThemeView::new();

    if start == Screen::Update   { update_screen.sync_from_app(&app); }
    if start == Screen::Settings { settings_view.load_from(&app.settings); }
    if start == Screen::Theme    { theme_view.load(); }

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

    loop {
        update_screen.pump(&mut app);
        health_view.try_complete_install();

        // Drain the latest network snapshot (keep only the freshest).
        while let Ok(status) = net_rx.try_recv() {
            app.network = Some(status);
        }

        // Fold in the initial release check the moment it lands.
        if let Some(rx) = &update_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(Some(info)) => {
                        if app.screen != Screen::Update {
                            app.flash = Some((
                                format!("Update available: v{} — open Update screen", info.latest),
                                FlashKind::Info,
                            ));
                        }
                        app.update_info = Some(info);
                    }
                    Ok(None) => {}
                    Err(e)   => app.update_error = Some(e.to_string()),
                }
            }
        }

        terminal.draw(|f| render(f, &app, &health_view, &configs_view, &alias_view, &profile_view, &update_screen, &settings_view, &theme_view))?;

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => handle_key(
                    &mut app,
                    &mut health_view,
                    &mut configs_view,
                    &mut alias_view,
                    &mut profile_view,
                    &mut update_screen,
                    &mut settings_view,
                    &mut theme_view,
                    key,
                ),
                Event::Mouse(me) => {
                    let area = terminal.size().map(|s| Rect::new(0, 0, s.width, s.height)).unwrap_or_default();
                    handle_mouse(
                        &mut app,
                        &mut health_view,
                        &mut configs_view,
                        &mut alias_view,
                        &mut profile_view,
                        &mut update_screen,
                        &mut settings_view,
                        &mut theme_view,
                        me,
                        area,
                    );
                }
                Event::Resize(..) => {}
                _ => {}
            }
        }

        if app.should_quit { break; }
    }

    Ok(())
}

// ── rendering dispatch ────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render(
    f:        &mut Frame,
    app:      &App,
    health:   &HealthView,
    configs:  &ConfigsView,
    aliases:  &AliasView,
    profile:  &ProfileView,
    update:   &UpdateScreen,
    settings: &SettingsView,
    theme:    &ThemeView,
) {
    let area = f.area();
    if area.width < 50 || area.height < 14 {
        render_too_small(f, area);
        return;
    }
    match app.screen {
        Screen::Main     => render_main(f, area, app),
        Screen::Health   => super::health::render(f, area, app, health),
        Screen::Configs  => super::configs::render(f, area, app, configs),
        Screen::Aliases  => render_aliases(f, area, app, aliases),
        Screen::Profile  => render_profile(f, area, app, profile),
        Screen::Update   => super::update::render(f, area, app, update),
        Screen::Settings => render_settings(f, area, app, settings),
        Screen::Theme    => render_theme(f, area, app, theme),
    }
}

/// The dashboard pane-grid region within the full terminal `area`. Shared by
/// rendering and mouse hit-testing so clicks land on what's drawn.
fn dashboard_grid(area: Rect) -> Rect {
    // Content region sits below the header line and above the desc/footer bars.
    let top    = area.y + 1;
    let bottom = area.y + area.height - 4; // desc bar lives at height-4
    Rect {
        x:      area.x + 1,
        y:      top,
        width:  area.width.saturating_sub(2),
        height: bottom.saturating_sub(top),
    }
}

fn render_main(f: &mut Frame, area: Rect, app: &App) {
    use super::overview::{self, PANES};

    draw_header(f, area, " dots ", VERSION);

    let grid = dashboard_grid(area);
    overview::render_grid(f, grid, app, app.dash_focus);

    let hint = PANES
        .get(app.dash_focus)
        .map(|p| overview::pane_hint(*p, app))
        .unwrap_or_default();
    draw_desc(f, area, &hint, app.flash.as_ref());
    draw_footer(f, area, " hjkl/↑↓←→ move  enter open  1 aliases 2 profile 3 theme 4 settings  q quit ");
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

#[allow(clippy::too_many_arguments)]
fn handle_key(
    app:      &mut App,
    health:   &mut HealthView,
    configs:  &mut ConfigsView,
    aliases:  &mut AliasView,
    profile:  &mut ProfileView,
    update:   &mut UpdateScreen,
    settings: &mut SettingsView,
    theme:    &mut ThemeView,
    key:      KeyEvent,
) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }
    match app.screen {
        Screen::Main     => handle_main_key(app, health, configs, aliases, profile, update, settings, theme, key),
        Screen::Health   => super::health::handle_key(app, health, key),
        Screen::Configs  => super::configs::handle_key(app, configs, key),
        Screen::Aliases  => handle_alias_key(app, aliases, key),
        Screen::Profile  => handle_profile_key(app, profile, key),
        Screen::Update   => super::update::handle_key(app, update, key),
        Screen::Settings => handle_settings_key(app, settings, theme, key),
        Screen::Theme    => handle_theme_key(app, theme, key),
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_main_key(
    app:      &mut App,
    health:   &mut HealthView,
    configs:  &mut ConfigsView,
    aliases:  &mut AliasView,
    profile:  &mut ProfileView,
    update:   &mut UpdateScreen,
    settings: &mut SettingsView,
    theme:    &mut ThemeView,
    key:      KeyEvent,
) {
    use super::overview::{self, Dir, PANES};

    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('h') | KeyCode::Left  => app.dash_focus = overview::move_focus(app.dash_focus, Dir::Left),
        KeyCode::Char('l') | KeyCode::Right => app.dash_focus = overview::move_focus(app.dash_focus, Dir::Right),
        KeyCode::Char('j') | KeyCode::Down  => app.dash_focus = overview::move_focus(app.dash_focus, Dir::Down),
        KeyCode::Char('k') | KeyCode::Up    => app.dash_focus = overview::move_focus(app.dash_focus, Dir::Up),
        KeyCode::Char(c) if ('1'..='9').contains(&c) => {
            let idx = (c as u8 - b'1') as usize;
            if idx < MAIN_MENU.len() {
                app.menu_idx = idx;
                navigate_to(app, health, configs, aliases, profile, update, settings, theme, MAIN_MENU[idx]);
            }
        }
        KeyCode::Enter => {
            if let Some(&pane) = PANES.get(app.dash_focus) {
                navigate_to(app, health, configs, aliases, profile, update, settings, theme, pane.target());
                if let Some(section) = pane.section() {
                    health.focus_section(section);
                }
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_mouse(
    app:      &mut App,
    health:   &mut HealthView,
    configs:  &mut ConfigsView,
    aliases:  &mut AliasView,
    profile:  &mut ProfileView,
    update:   &mut UpdateScreen,
    settings: &mut SettingsView,
    theme:    &mut ThemeView,
    me:       MouseEvent,
    area:     Rect,
) {
    use super::overview::{self, PANES};

    match me.kind {
        // Wheel scroll drives vertical navigation on whatever screen is up,
        // by reusing each screen's existing up/down key handling.
        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
            let code = if matches!(me.kind, MouseEventKind::ScrollDown) { KeyCode::Down } else { KeyCode::Up };
            let key = KeyEvent::new(code, KeyModifiers::NONE);
            handle_key(app, health, configs, aliases, profile, update, settings, theme, key);
        }
        // Left-click on a dashboard pane focuses and opens it. The 50×14 guard
        // matches the threshold below which the dashboard isn't drawn at all.
        MouseEventKind::Down(MouseButton::Left)
            if app.screen == Screen::Main && area.width >= 50 && area.height >= 14 =>
        {
            let grid = dashboard_grid(area);
            if let Some(i) = overview::pane_at(grid, me.column, me.row) {
                app.dash_focus = i;
                if let Some(&pane) = PANES.get(i) {
                    navigate_to(app, health, configs, aliases, profile, update, settings, theme, pane.target());
                    if let Some(section) = pane.section() {
                        health.focus_section(section);
                    }
                }
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn navigate_to(
    app:      &mut App,
    health:   &mut HealthView,
    configs:  &mut ConfigsView,
    aliases:  &mut AliasView,
    profile:  &mut ProfileView,
    update:   &mut UpdateScreen,
    settings: &mut SettingsView,
    theme:    &mut ThemeView,
    screen:   Screen,
) {
    match screen {
        Screen::Health => {
            health.rebuild();
            app.screen = Screen::Health;
            app.flash  = None;
        }
        Screen::Configs => {
            configs.reload();
            app.screen = Screen::Configs;
            app.flash  = None;
        }
        Screen::Aliases => {
            aliases.reload();
            app.screen = Screen::Aliases;
            app.flash  = None;
        }
        Screen::Profile => {
            profile.reset();
            app.screen = Screen::Profile;
            app.flash  = None;
        }
        Screen::Update => {
            update.sync_from_app(app);
            app.screen = Screen::Update;
            app.flash  = None;
        }
        Screen::Settings => {
            settings.load_from(&app.settings);
            app.screen = Screen::Settings;
            app.flash  = None;
        }
        Screen::Theme => {
            theme.load();
            app.screen = Screen::Theme;
            app.flash  = None;
        }
        other => {
            app.screen = other;
            app.flash  = None;
        }
    }
}
