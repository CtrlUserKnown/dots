use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};
use std::path::Path;

use crate::config::settings::{self, dots_dir, Settings};
use crate::tui::{draw_desc, draw_footer, draw_header, FlashKind};
use crate::tui::app::{App, Screen};
use crate::tui::theme::{style_dim, style_error, style_header, style_select};

const VERSION: &str = env!("DOTS_VERSION");

// ── frequency cycle ───────────────────────────────────────────────────────────

const FREQ_OPTIONS: &[(u64, &str)] = &[
    (60,    "Every hour"),
    (360,   "Every 6 hours"),
    (720,   "Every 12 hours"),
    (1440,  "Daily"),
    (4320,  "Every 3 days"),
    (10080, "Weekly"),
];

fn freq_label(minutes: u64) -> &'static str {
    FREQ_OPTIONS.iter().find(|&&(m, _)| m == minutes).map(|&(_, l)| l).unwrap_or("Custom")
}

fn next_freq(current: u64) -> u64 {
    let idx = FREQ_OPTIONS.iter().position(|&(m, _)| m == current).unwrap_or(3);
    FREQ_OPTIONS[(idx + 1) % FREQ_OPTIONS.len()].0
}

// ── field definitions ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum FieldKey {
    CheckUpdates,
    AutoUpdates,
    CheckInterval,
    Greeting,
    Theme,
    DeveloperMode,
}

fn field_label(k: FieldKey) -> &'static str {
    match k {
        FieldKey::CheckUpdates  => "Check for updates",
        FieldKey::AutoUpdates   => "Auto-updates",
        FieldKey::CheckInterval => "Check interval",
        FieldKey::Greeting      => "Shell greeting",
        FieldKey::Theme         => "Theme",
        FieldKey::DeveloperMode => "Developer mode",
    }
}

fn field_desc(k: FieldKey) -> &'static str {
    match k {
        FieldKey::CheckUpdates  => "manually check for dotfiles updates right now",
        FieldKey::AutoUpdates   => "automatically check for updates in the background",
        FieldKey::CheckInterval => "how often to check for updates",
        FieldKey::Greeting      => "show fastfetch system info when opening a terminal",
        FieldKey::Theme         => "pick a terminal color theme",
        FieldKey::DeveloperMode => "track commits instead of releases",
    }
}

// ── settings view ─────────────────────────────────────────────────────────────

pub struct SettingsView {
    pub settings: Settings,
    pub cursor:   usize,
    pub flash:    Option<(String, FlashKind)>,
    fields:       Vec<FieldKey>,
}

impl SettingsView {
    pub fn new() -> Self {
        Self {
            settings: Settings::default(),
            cursor:   0,
            flash:    None,
            fields:   Vec::new(),
        }
    }

    pub fn load_from(&mut self, s: &Settings) {
        self.settings = s.clone();
        self.cursor   = 0;
        self.flash    = None;
        self.fields   = vec![
            FieldKey::CheckUpdates,
            FieldKey::AutoUpdates,
            FieldKey::CheckInterval,
            FieldKey::Greeting,
            FieldKey::Theme,
            FieldKey::DeveloperMode,
        ];
    }

    fn value_str(&self, k: FieldKey) -> (String, bool) {
        // (display text, is_positive)
        match k {
            FieldKey::AutoUpdates   => bool_display(self.settings.dots.update_check),
            FieldKey::CheckInterval => (freq_label(self.settings.dots.update_frequency).to_string(), true),
            FieldKey::Greeting      => bool_display(self.settings.dots.greeting),
            FieldKey::DeveloperMode => bool_display(self.settings.dots.developer_mode),
            FieldKey::CheckUpdates | FieldKey::Theme => (String::new(), true),
        }
    }

    fn activate(&mut self, k: FieldKey) -> SettingsAction {
        match k {
            FieldKey::AutoUpdates   => { self.settings.dots.update_check    ^= true; SettingsAction::None }
            FieldKey::Greeting      => { self.settings.dots.greeting         ^= true; SettingsAction::None }
            FieldKey::DeveloperMode => { self.settings.dots.developer_mode   ^= true; SettingsAction::None }
            FieldKey::CheckInterval => {
                self.settings.dots.update_frequency = next_freq(self.settings.dots.update_frequency);
                SettingsAction::None
            }
            FieldKey::CheckUpdates  => SettingsAction::OpenUpdate,
            FieldKey::Theme         => SettingsAction::OpenTheme,
        }
    }
}

#[must_use]
enum SettingsAction { None, OpenUpdate, OpenTheme }

fn bool_display(v: bool) -> (String, bool) {
    (if v { "ON".to_string() } else { "OFF".to_string() }, v)
}

pub fn render_settings(f: &mut Frame, area: Rect, _app: &App, view: &SettingsView) {
    draw_header(f, area, " settings ", VERSION);

    for (i, &k) in view.fields.iter().enumerate() {
        let y = area.y + 2 + i as u16;
        if y + 4 >= area.y + area.height { break; }

        let is_sel = i == view.cursor;
        let cursor = if is_sel { "▶" } else { " " };
        let label  = field_label(k);
        let (val, pos) = view.value_str(k);

        let cursor_style = if is_sel { style_select().add_modifier(Modifier::BOLD) } else { style_dim() };
        let label_style  = if is_sel { ratatui::style::Style::default().add_modifier(Modifier::BOLD) } else { ratatui::style::Style::default() };
        let val_style    = if val.is_empty() {
            style_header()
        } else if pos {
            style_select()
        } else {
            style_error()
        };

        let rect = Rect { x: area.x, y, width: area.width, height: 1 };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("{cursor} "), cursor_style),
                Span::styled(format!("{:<22}", label), label_style),
                Span::styled(val, val_style),
            ])),
            rect,
        );
    }

    let desc = view.fields.get(view.cursor).copied().map(field_desc).unwrap_or("");
    draw_desc(f, area, desc, view.flash.as_ref());
    draw_footer(f, area, " j/k navigate  space/enter activate  esc save & back  q save & quit ");
}

pub fn handle_settings_key(app: &mut App, view: &mut SettingsView, theme: &mut ThemeView, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => save_and_exit(app, view),
        KeyCode::Char('q') => save_and_quit(app, view),
        KeyCode::Char('j') | KeyCode::Down => {
            if !view.fields.is_empty() {
                view.cursor = (view.cursor + 1) % view.fields.len();
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if !view.fields.is_empty() {
                view.cursor = (view.cursor + view.fields.len() - 1) % view.fields.len();
            }
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            if let Some(&k) = view.fields.get(view.cursor) {
                match view.activate(k) {
                    SettingsAction::None => {}
                    SettingsAction::OpenUpdate => {
                        app.screen = Screen::Update;
                    }
                    SettingsAction::OpenTheme => {
                        theme.load();
                        app.screen = Screen::Theme;
                    }
                }
            }
        }
        _ => {}
    }
}

fn save_and_exit(app: &mut App, view: &SettingsView) {
    match settings::save(&view.settings) {
        Ok(()) => {
            app.settings = view.settings.clone();
            app.screen   = Screen::Main;
            app.flash    = None;
        }
        Err(e) => {
            // Flash shown next render via view.flash — but we can't mutate view here easily
            // Signal the error via app.flash and stay on Settings
            app.flash = Some((format!("✗ Save failed: {e}"), FlashKind::Error));
        }
    }
}

/// Persist settings then quit the whole app (q from the settings screen).
fn save_and_quit(app: &mut App, view: &SettingsView) {
    match settings::save(&view.settings) {
        Ok(()) => {
            app.settings     = view.settings.clone();
            app.should_quit  = true;
        }
        Err(e) => {
            app.flash = Some((format!("✗ Save failed: {e}"), FlashKind::Error));
        }
    }
}

// ── theme picker ──────────────────────────────────────────────────────────────

pub struct ThemeView {
    pub themes:  Vec<String>,
    pub cursor:  usize,
    pub scroll:  usize,
    pub current: String,
    pub flash:   Option<(String, FlashKind)>,
}

impl ThemeView {
    pub fn new() -> Self {
        Self { themes: Vec::new(), cursor: 0, scroll: 0, current: String::new(), flash: None }
    }

    pub fn load(&mut self) {
        let dots = dots_dir();
        self.current = get_current_theme(&dots);
        self.themes  = list_ghostty_themes();
        self.cursor  = self.themes.iter().position(|t| t == &self.current).unwrap_or(0);
        self.scroll  = 0;
        self.flash   = None;
    }
}

pub fn render_theme(f: &mut Frame, area: Rect, _app: &App, view: &ThemeView) {
    draw_header(f, area, " theme ", VERSION);

    if view.themes.is_empty() {
        let rect = Rect { x: area.x + 2, y: area.y + 2, width: area.width.saturating_sub(4), height: 2 };
        f.render_widget(
            Paragraph::new(vec![
                Line::from("Ghostty is not installed — theme picker requires Ghostty."),
                Line::from(Span::styled("Press esc to go back.", style_dim())),
            ]),
            rect,
        );
        draw_footer(f, area, " esc back  q quit ");
        return;
    }

    if area.height < 5 { return; }
    let visible = (area.height as usize).saturating_sub(5);

    for (i, theme) in view.themes.iter().enumerate().skip(view.scroll).take(visible) {
        let y       = area.y + 2 + (i - view.scroll) as u16;
        let is_sel  = i == view.cursor;
        let is_cur  = *theme == view.current;
        let cursor  = if is_sel { "▶" } else { "  " };
        let suffix  = if is_cur { "  (active)" } else { "" };
        let rect    = Rect { x: area.x, y, width: area.width, height: 1 };

        if is_sel {
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(cursor, style_select().add_modifier(Modifier::BOLD)),
                    Span::styled(format!(" {theme}{suffix}"), ratatui::style::Style::default().add_modifier(Modifier::BOLD)),
                ])),
                rect,
            );
        } else {
            let style = if is_cur { style_dim() } else { ratatui::style::Style::default() };
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(format!("   {theme}{suffix}"), style))),
                rect,
            );
        }
    }

    let desc = format!("enter to apply  current: {}", view.current);
    draw_desc(f, area, &desc, view.flash.as_ref());
    draw_footer(f, area, " j/k navigate  enter apply  esc back  q quit ");
}

pub fn handle_theme_key(app: &mut App, view: &mut ThemeView, key: KeyEvent) {
    let n = view.themes.len();
    match key.code {
        KeyCode::Esc => {
            app.screen = Screen::Settings;
            app.flash  = None;
        }
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => {
            if n > 0 {
                view.cursor = (view.cursor + 1).min(n - 1);
                update_theme_scroll(view);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            view.cursor = view.cursor.saturating_sub(1);
            update_theme_scroll(view);
        }
        KeyCode::Enter => {
            if let Some(name) = view.themes.get(view.cursor).cloned() {
                let dots = dots_dir();
                match set_ghostty_theme(&dots, &name) {
                    Ok(()) => {
                        view.current = name.clone();
                        view.flash = Some((
                            format!("✓ Theme set to {name} — restart Ghostty to apply"),
                            FlashKind::Success,
                        ));
                        // Also persist theme name in settings
                        if let Ok(mut s) = settings::load() {
                            s.dots.theme = name;
                            settings::save(&s).ok();
                            app.settings.dots.theme = s.dots.theme;
                        }
                    }
                    Err(e) => {
                        view.flash = Some((format!("✗ {e}"), FlashKind::Error));
                    }
                }
            }
        }
        _ => {}
    }
}

fn update_theme_scroll(view: &mut ThemeView) {
    if view.cursor < view.scroll {
        view.scroll = view.cursor;
    } else if view.scroll + 20 <= view.cursor {
        view.scroll = view.cursor.saturating_sub(19);
    }
}

// ── ghostty theme helpers ─────────────────────────────────────────────────────

fn ghostty_config_path(dots_dir: &Path) -> std::path::PathBuf {
    dots_dir.join("src/ghostty/config")
}

pub fn list_ghostty_themes() -> Vec<String> {
    let out = std::process::Command::new("ghostty")
        .arg("+list-themes")
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() { return Vec::new(); }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| {
            let s = l.trim();
            if let Some(idx) = s.rfind(" (") { s[..idx].trim().to_string() }
            else { s.to_string() }
        })
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn get_current_theme(dots_dir: &Path) -> String {
    let path = ghostty_config_path(dots_dir);
    let Ok(text) = std::fs::read_to_string(&path) else { return String::new() };
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("theme") {
            let rest = rest.trim_start_matches(|c: char| c == ' ' || c == '=').trim();
            let rest = rest.trim_matches('"');
            if !rest.is_empty() { return rest.to_string(); }
        }
    }
    String::new()
}

pub fn set_ghostty_theme(dots_dir: &Path, name: &str) -> Result<()> {
    let path = ghostty_config_path(dots_dir);
    if !path.exists() {
        anyhow::bail!("ghostty config not found at {}", path.display());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;

    let new_line = format!("theme = \"{name}\"");
    let mut found = false;
    let new_text: String = text.lines().map(|l| {
        if l.trim_start().starts_with("theme") && l.contains('=') && !found {
            found = true;
            new_line.clone()
        } else {
            l.to_string()
        }
    }).collect::<Vec<_>>().join("\n");

    let final_text = if found {
        if text.ends_with('\n') { new_text + "\n" } else { new_text }
    } else {
        // Append
        if text.ends_with('\n') {
            text + &new_line + "\n"
        } else {
            text + "\n" + &new_line + "\n"
        }
    };

    std::fs::write(&path, final_text)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn make_ghostty_config(tmp: &Path, content: &str) {
        let dir = tmp.join("src/ghostty");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config"), content).unwrap();
    }

    #[test]
    fn set_and_get_theme() {
        let tmp = tempdir().unwrap();
        make_ghostty_config(tmp.path(), "font-size = 16\ntheme = \"old\"\n");
        set_ghostty_theme(tmp.path(), "new-theme").unwrap();
        assert_eq!(get_current_theme(tmp.path()), "new-theme");
    }

    #[test]
    fn set_theme_no_existing_line() {
        let tmp = tempdir().unwrap();
        make_ghostty_config(tmp.path(), "font-size = 16\n");
        set_ghostty_theme(tmp.path(), "Nord").unwrap();
        let content = fs::read_to_string(ghostty_config_path(tmp.path())).unwrap();
        assert!(content.contains("theme = \"Nord\""), "theme line not found in: {content}");
    }
}
