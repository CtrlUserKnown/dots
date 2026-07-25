use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};
use serde_json::Value;

use crate::config::profile::{
    apply_personal_config, fetch_github_raw, generate_personal_config,
    load_from_value, personal_config_path, validate_personal_config,
};
use crate::tui::{draw_desc, draw_footer, draw_header, FlashKind};
use crate::tui::app::{App, Screen};
use crate::tui::theme::{style_dim, style_error, style_select};

// ── state ─────────────────────────────────────────────────────────────────────

pub enum ProfileMode {
    Normal,
    ImportFile    { input: String },
    ImportGitHub  { input: String },
}

pub struct ProfileView {
    pub mode:  ProfileMode,
    pub flash: Option<(String, FlashKind)>,
}

impl Default for ProfileView {
    fn default() -> Self { Self::new() }
}

impl ProfileView {
    pub fn new() -> Self {
        Self { mode: ProfileMode::Normal, flash: None }
    }

    pub fn reset(&mut self) {
        self.mode  = ProfileMode::Normal;
        self.flash = None;
    }
}

// ── rendering ─────────────────────────────────────────────────────────────────

pub fn render_profile(f: &mut Frame, area: Rect, _app: &App, view: &ProfileView) {
    draw_header(f, area, " profile ", "");
    if area.height < 6 { return; }

    let path    = personal_config_path();
    let exists  = path.exists();
    let sym     = if exists { "✓" } else { "✗" };
    let sty     = if exists { style_select() } else { style_error() };
    let path_label = home_rel(&path);

    // Row 2: status + path
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{sym}  "), sty.add_modifier(Modifier::BOLD)),
            Span::styled(path_label, style_dim()),
        ])),
        Rect { x: area.x + 2, y: area.y + 2, width: area.width.saturating_sub(4), height: 1 },
    );

    // Row 3: metadata if file exists
    if exists {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                let gen  = v["generated"].as_str().unwrap_or("")
                    .chars().take(19).collect::<String>();
                let dver = v["dots_version"].as_str().unwrap_or("?");
                let pkgs = &v["packages"];
                let n = pkgs["optional"].as_array().map(|a| a.len()).unwrap_or(0)
                      + pkgs["dev"].as_array().map(|a| a.len()).unwrap_or(0);
                let meta = format!("   generated {}  ·  {} packages  ·  dots {}", gen, n, dver);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(meta, style_dim()))),
                    Rect { x: area.x + 2, y: area.y + 3, width: area.width.saturating_sub(4), height: 1 },
                );
            }
        }
    }

    // Rows 5-7: action items
    let actions: &[(&str, &str)] = &[
        ("g", "generate / update config from current system"),
        ("i", "import from a local file"),
        ("G", "import from GitHub  (user/repo/path/to/file.json)"),
    ];
    for (i, (key, desc)) in actions.iter().enumerate() {
        let y = area.y + 5 + i as u16;
        if y + 4 >= area.y + area.height { break; }
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("{key}  "), style_select()),
                Span::styled(*desc, style_dim()),
            ])),
            Rect { x: area.x + 2, y, width: area.width.saturating_sub(4), height: 1 },
        );
    }

    // Inline input prompt (overrides desc area)
    match &view.mode {
        ProfileMode::ImportFile { input } => {
            render_input(f, area, "Path to personal.json:", input, &view.flash);
        }
        ProfileMode::ImportGitHub { input } => {
            render_input(f, area, "Format: user/repo/path/to/file.json:", input, &view.flash);
        }
        ProfileMode::Normal => {
            draw_desc(f, area, "", view.flash.as_ref());
            draw_footer(f, area, " g generate  i import file  G import GitHub  esc back  q quit ");
        }
    }
}

fn render_input(
    f: &mut Frame, area: Rect,
    prompt: &str, input: &str,
    flash: &Option<(String, FlashKind)>,
) {
    let desc_y = area.y + area.height.saturating_sub(4);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("  {prompt} "), style_dim()),
            Span::raw(format!("{input}█")),
        ])),
        Rect { x: area.x, y: desc_y, width: area.width, height: 1 },
    );
    // Reuse flash for errors, footer for hint
    if let Some((msg, kind)) = flash {
        let sty = match kind {
            FlashKind::Error   => style_error(),
            FlashKind::Success => style_select(),
            FlashKind::Info    => style_dim().add_modifier(Modifier::BOLD),
        };
        let flash_y = desc_y.saturating_sub(1);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(format!("  {msg}"), sty))),
            Rect { x: area.x, y: flash_y, width: area.width, height: 1 },
        );
    }
    draw_footer(f, area, " enter confirm  esc cancel ");
}

// ── key handling ──────────────────────────────────────────────────────────────

pub fn handle_profile_key(app: &mut App, view: &mut ProfileView, key: KeyEvent) {
    match &view.mode {
        ProfileMode::Normal => handle_normal(app, view, key),
        ProfileMode::ImportFile { .. } |
        ProfileMode::ImportGitHub { .. } => handle_input(view, key),
    }
}

fn handle_normal(app: &mut App, view: &mut ProfileView, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => { app.screen = Screen::Main; app.flash = None; }
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('g') => {
            let path = personal_config_path();
            match generate_personal_config(&path) {
                Ok(()) => view.flash = Some((
                    format!("✓ Saved to {}", home_rel(&path)), FlashKind::Success,
                )),
                Err(e) => view.flash = Some((format!("✗ {e}"), FlashKind::Error)),
            }
        }
        KeyCode::Char('i') => {
            let default = home_rel(&personal_config_path());
            view.mode  = ProfileMode::ImportFile { input: default };
            view.flash = None;
        }
        KeyCode::Char('G') => {
            view.mode  = ProfileMode::ImportGitHub { input: String::new() };
            view.flash = None;
        }
        _ => {}
    }
}

fn handle_input(view: &mut ProfileView, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => { view.mode = ProfileMode::Normal; view.flash = None; }
        KeyCode::Backspace => {
            match &mut view.mode {
                ProfileMode::ImportFile { input } |
                ProfileMode::ImportGitHub { input } => { input.pop(); }
                _ => {}
            }
        }
        KeyCode::Char(c) => {
            match &mut view.mode {
                ProfileMode::ImportFile { input } |
                ProfileMode::ImportGitHub { input } => { input.push(c); }
                _ => {}
            }
        }
        KeyCode::Enter => {
            let mode = std::mem::replace(&mut view.mode, ProfileMode::Normal);
            match mode {
                ProfileMode::ImportFile { input } => {
                    let expanded = input.replacen('~', &home_str(), 1);
                    process_file_import(view, std::path::Path::new(&expanded));
                }
                ProfileMode::ImportGitHub { input } => {
                    process_github_import(view, &input);
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn process_file_import(view: &mut ProfileView, path: &std::path::Path) {
    if !path.exists() {
        view.flash = Some((
            format!("✗ File not found: {}", path.display()),
            FlashKind::Error,
        ));
        return;
    }
    let text = match std::fs::read_to_string(path) {
        Err(e) => { view.flash = Some((format!("✗ Read error: {e}"), FlashKind::Error)); return; }
        Ok(t) => t,
    };
    import_from_json(view, &text);
}

fn process_github_import(view: &mut ProfileView, spec: &str) {
    if spec.is_empty() { view.mode = ProfileMode::Normal; return; }
    match fetch_github_raw(spec) {
        Err(e) => view.flash = Some((format!("✗ {e}"), FlashKind::Error)),
        Ok(text) => import_from_json(view, &text),
    }
}

fn import_from_json(view: &mut ProfileView, text: &str) {
    let v: Value = match serde_json::from_str(text) {
        Err(e) => { view.flash = Some((format!("✗ JSON error: {e}"), FlashKind::Error)); return; }
        Ok(v) => v,
    };
    if let Err(e) = validate_personal_config(&v) {
        view.flash = Some((format!("✗ {e}"), FlashKind::Error));
        return;
    }
    match load_from_value(&v).and_then(|cfg| apply_personal_config(&cfg)) {
        Err(e) => view.flash = Some((format!("✗ Apply failed: {e}"), FlashKind::Error)),
        Ok(_)  => view.flash = Some(("✓ Config applied".to_string(), FlashKind::Success)),
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn home_rel(path: &std::path::Path) -> String {
    let home = dirs::home_dir().unwrap_or_default();
    path.strip_prefix(&home)
        .map(|p| format!("~/{}", p.display()))
        .unwrap_or_else(|_| path.display().to_string())
}

fn home_str() -> String {
    dirs::home_dir().unwrap_or_default().to_string_lossy().to_string()
}
