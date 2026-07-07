use std::io::Stdout;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    backend::CrosstermBackend,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
    Terminal,
};

use crate::config::settings::{dots_dir, Settings};
use crate::tui::theme::{style_dim, style_error, style_select};
use crate::tui::{draw_desc, draw_footer, draw_header, FlashKind};
use crate::update::{
    check_upstream, record_check, should_check, UpdateMode, UpdateStatus,
};

use super::aliases::{handle_alias_key, render_aliases, AliasView};
use super::health::HealthView;
use super::profile::{handle_profile_key, render_profile, ProfileView};
use super::settings::{handle_settings_key, render_settings, render_theme, handle_theme_key, SettingsView, ThemeView};
use super::update::UpdateScreen;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ── screen enum ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main,
    Health,
    Aliases,
    Profile,
    Theme,
    Settings,
    Update,
    Ssm,
}

// ── app state ─────────────────────────────────────────────────────────────────

pub struct App {
    pub should_quit:   bool,
    pub open_ssm:      bool,
    pub screen:        Screen,
    pub flash:         Option<(String, FlashKind)>,
    pub menu_idx:      usize,
    pub update_status: Option<UpdateStatus>,
    pub update_error:  Option<String>,
    pub settings:      Settings,
}

impl App {
    pub fn new(start: Screen, settings: Settings) -> Self {
        Self {
            should_quit:   false,
            open_ssm:      false,
            screen:        start,
            flash:         None,
            menu_idx:      0,
            update_status: None,
            update_error:  None,
            settings,
        }
    }
}

// ── main menu ─────────────────────────────────────────────────────────────────

struct MenuItem {
    label:  &'static str,
    screen: Screen,
    desc:   &'static str,
}

const MAIN_MENU: &[MenuItem] = &[
    MenuItem { label: "Health",   screen: Screen::Health,   desc: "check symlinks, tools & plugins" },
    MenuItem { label: "Aliases",  screen: Screen::Aliases,  desc: "view and manage shell aliases" },
    MenuItem { label: "Profile",  screen: Screen::Profile,  desc: "export / import personal config" },
    MenuItem { label: "Theme",    screen: Screen::Theme,    desc: "pick a terminal color theme" },
    MenuItem { label: "SSM",      screen: Screen::Ssm,      desc: "SSH session manager" },
    MenuItem { label: "Settings", screen: Screen::Settings, desc: "configure dots preferences" },
    MenuItem { label: "Update",   screen: Screen::Update,   desc: "check for updates" },
];

// ── event loop ────────────────────────────────────────────────────────────────

/// Returns true if the SSM TUI should be opened after this returns.
pub fn run(
    terminal:  &mut Terminal<CrosstermBackend<Stdout>>,
    start:     Screen,
    settings:  &Settings,
) -> anyhow::Result<bool> {
    let mut app           = App::new(start, settings.clone());
    let mut health_view   = HealthView::new();
    let mut alias_view    = AliasView::new();
    let mut profile_view  = ProfileView::new();
    let mut update_screen = UpdateScreen::new();
    let mut settings_view = SettingsView::new();
    let mut theme_view    = ThemeView::new();

    if start == Screen::Update   { update_screen.sync_from_app(&app); }
    if start == Screen::Settings { settings_view.load_from(&app.settings); }
    if start == Screen::Theme    { theme_view.load(); }

    // Spawn background update check
    let (tx, update_rx) = std::sync::mpsc::channel::<anyhow::Result<UpdateStatus>>();
    {
        let dots = dots_dir();
        let freq = settings.dots.update_frequency;
        let mode = if settings.dots.developer_mode { UpdateMode::Dev } else { UpdateMode::Normal };
        std::thread::spawn(move || {
            if should_check(&dots, freq) {
                let _ = record_check(&dots);
                let _ = tx.send(check_upstream(&dots, mode));
            }
        });
    }

    loop {
        update_screen.try_complete_pull();
        health_view.try_complete_install();

        if let Ok(result) = update_rx.try_recv() {
            match result {
                Ok(status) => {
                    if status.behind > 0 && app.screen != Screen::Update {
                        app.flash = Some((
                            format!("Update available: v{} — open Update screen", status.label),
                            FlashKind::Info,
                        ));
                    }
                    if app.screen == Screen::Update {
                        if status.behind > 0 {
                            update_screen.state = super::update::UpdateState::Available {
                                behind: status.behind,
                                label:  status.label.clone(),
                            };
                        } else {
                            update_screen.state = super::update::UpdateState::UpToDate;
                        }
                    }
                    app.update_status = Some(status);
                }
                Err(e) => {
                    let msg = e.to_string();
                    if app.screen == Screen::Update {
                        update_screen.state = super::update::UpdateState::Error(msg.clone());
                    }
                    app.update_error = Some(msg);
                }
            }
        }

        terminal.draw(|f| render(f, &app, &health_view, &alias_view, &profile_view, &update_screen, &settings_view, &theme_view))?;

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => handle_key(
                    &mut app,
                    &mut health_view,
                    &mut alias_view,
                    &mut profile_view,
                    &mut update_screen,
                    &mut settings_view,
                    &mut theme_view,
                    key,
                ),
                Event::Resize(..) => {}
                _ => {}
            }
        }

        if app.should_quit { break; }
    }

    Ok(app.open_ssm)
}

// ── rendering dispatch ────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render(
    f:        &mut Frame,
    app:      &App,
    health:   &HealthView,
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
        Screen::Aliases  => render_aliases(f, area, app, aliases),
        Screen::Profile  => render_profile(f, area, app, profile),
        Screen::Update   => super::update::render(f, area, app, update),
        Screen::Settings => render_settings(f, area, app, settings),
        Screen::Theme    => render_theme(f, area, app, theme),
        Screen::Ssm      => render_ssm_redirect(f, area, app),
    }
}

fn render_main(f: &mut Frame, area: Rect, app: &App) {
    draw_header(f, area, " dots ", VERSION);

    let content_y = area.y + 2;
    for (i, item) in MAIN_MENU.iter().enumerate() {
        let y = content_y + i as u16;
        if y + 3 >= area.y + area.height { break; }
        let rect = Rect { x: area.x + 2, y, width: area.width.saturating_sub(4), height: 1 };
        let (num_style, label_style) = if i == app.menu_idx {
            (style_select().add_modifier(Modifier::BOLD), style_select().add_modifier(Modifier::BOLD))
        } else {
            (style_dim(), ratatui::style::Style::default())
        };
        let cursor = if i == app.menu_idx { "▶ " } else { "  " };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(cursor, num_style),
                Span::styled(format!("{:<2}", i + 1), num_style),
                Span::raw("  "),
                Span::styled(item.label, label_style),
            ])),
            rect,
        );
    }

    let desc = MAIN_MENU.get(app.menu_idx).map(|m| m.desc).unwrap_or("");
    draw_desc(f, area, desc, app.flash.as_ref());
    draw_footer(f, area, " j/k navigate  1-9 jump  enter select  q quit ");
}

fn render_ssm_redirect(f: &mut Frame, area: Rect, app: &App) {
    draw_header(f, area, " ssm ", VERSION);
    let rect = Rect { x: area.x + 2, y: area.y + 2, width: area.width.saturating_sub(4), height: 1 };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("Opening SSH session manager…", style_dim()))),
        rect,
    );
    draw_desc(f, area, "", app.flash.as_ref());
    draw_footer(f, area, " q cancel ");
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
        Screen::Main     => handle_main_key(app, health, aliases, profile, update, settings, theme, key),
        Screen::Health   => super::health::handle_key(app, health, key),
        Screen::Aliases  => handle_alias_key(app, aliases, key),
        Screen::Profile  => handle_profile_key(app, profile, key),
        Screen::Update   => super::update::handle_key(app, update, key),
        Screen::Settings => handle_settings_key(app, settings, theme, key),
        Screen::Theme    => handle_theme_key(app, theme, key),
        Screen::Ssm      => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => { app.screen = Screen::Main; app.flash = None; }
            _ => {}
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_main_key(
    app:      &mut App,
    health:   &mut HealthView,
    aliases:  &mut AliasView,
    profile:  &mut ProfileView,
    update:   &mut UpdateScreen,
    settings: &mut SettingsView,
    theme:    &mut ThemeView,
    key:      KeyEvent,
) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => {
            app.menu_idx = (app.menu_idx + 1).min(MAIN_MENU.len() - 1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.menu_idx = app.menu_idx.saturating_sub(1);
        }
        KeyCode::Char(c) if ('1'..='9').contains(&c) => {
            let idx = (c as u8 - b'1') as usize;
            if idx < MAIN_MENU.len() {
                app.menu_idx = idx;
                navigate_to(app, health, aliases, profile, update, settings, theme, MAIN_MENU[idx].screen);
            }
        }
        KeyCode::Enter => {
            let screen = MAIN_MENU[app.menu_idx].screen;
            navigate_to(app, health, aliases, profile, update, settings, theme, screen);
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn navigate_to(
    app:      &mut App,
    health:   &mut HealthView,
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
        Screen::Ssm => {
            app.open_ssm    = true;
            app.should_quit = true;
        }
        other => {
            app.screen = other;
            app.flash  = None;
        }
    }
}
