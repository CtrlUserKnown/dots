use std::sync::mpsc::Receiver;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::tui::{draw_desc, draw_footer, draw_header};
use crate::tui::app::{App, Screen};
use crate::tui::theme::{style_dim, style_error, style_header, style_select};
use crate::update as update_core;

const VERSION: &str = env!("DOTS_VERSION");

// ── state machine ─────────────────────────────────────────────────────────────

pub enum UpdateState {
    Checking,
    UpToDate,
    Available { behind: u32, label: String },
    Pulling,
    Done    { new_ver: String },
    Error   (String),
}

pub struct UpdateScreen {
    pub state:   UpdateState,
    pub pull_rx: Option<Receiver<anyhow::Result<String>>>,
}

impl UpdateScreen {
    pub fn new() -> Self {
        Self { state: UpdateState::Checking, pull_rx: None }
    }

    pub fn sync_from_app(&mut self, app: &App) {
        match &app.update_status {
            Some(status) if status.behind > 0 => {
                self.state = UpdateState::Available {
                    behind: status.behind,
                    label:  status.label.clone(),
                };
            }
            Some(_) => self.state = UpdateState::UpToDate,
            None => match &app.update_error {
                Some(e) => self.state = UpdateState::Error(e.clone()),
                None    => self.state = UpdateState::Checking,
            },
        }
    }

    pub fn try_complete_pull(&mut self) {
        let done = if let Some(rx) = &self.pull_rx {
            rx.try_recv().ok()
        } else {
            None
        };
        if let Some(result) = done {
            self.pull_rx = None;
            self.state = match result {
                Ok(ver) => UpdateState::Done { new_ver: ver },
                Err(e)  => UpdateState::Error(e.to_string()),
            };
        }
    }
}

// ── rendering ─────────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, area: Rect, _app: &App, screen: &UpdateScreen) {
    draw_header(f, area, " check for updates ", VERSION);

    if area.height < 5 { return; }

    let body_y    = area.y + 2;
    let body_rect = Rect { x: area.x + 2, y: body_y, width: area.width.saturating_sub(4), height: 1 };

    let (line, desc) = match &screen.state {
        UpdateState::Checking => (
            Line::from(Span::styled("  Checking for updates…", style_dim())),
            "contacting upstream…",
        ),
        UpdateState::UpToDate => (
            Line::from(vec![
                Span::styled("✓  ", style_select()),
                Span::raw(format!("No updates — you're on v{VERSION}")),
            ]),
            "you're up to date",
        ),
        UpdateState::Available { behind, label } => (
            Line::from(vec![
                Span::styled("📦 ", style_header()),
                Span::raw(format!("Update available: v{VERSION} → v{label}  ({behind} commit(s))")),
            ]),
            "press y to update, any other key to skip",
        ),
        UpdateState::Pulling => (
            Line::from(Span::styled("  Pulling update…", style_dim())),
            "please wait",
        ),
        UpdateState::Done { new_ver } => (
            Line::from(vec![
                Span::styled("✓  ", style_select()),
                Span::raw(format!("Updated to v{new_ver}. Restart your shell to apply.")),
            ]),
            "restart your shell to load the new version",
        ),
        UpdateState::Error(msg) => (
            Line::from(vec![
                Span::styled("✗  ", style_error()),
                Span::raw(format!("Could not reach upstream: {msg}")),
            ]),
            "check your internet connection",
        ),
    };

    f.render_widget(Paragraph::new(line), body_rect);

    let hint_str = match screen.state {
        UpdateState::Available { .. } => " y update  esc back  q quit ",
        _ => " esc back  q quit ",
    };
    draw_desc(f, area, desc, None);
    draw_footer(f, area, hint_str);
}

// ── key handling ─────────────────────────────────────────────────────────────

pub fn handle_key(app: &mut App, screen: &mut UpdateScreen, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.screen = Screen::Main;
            app.flash  = None;
        }
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('y') => {
            if matches!(screen.state, UpdateState::Available { .. }) {
                start_pull(screen);
            }
        }
        _ => {}
    }
}

fn start_pull(screen: &mut UpdateScreen) {
    let dots_dir = crate::config::settings::dots_dir();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(update_core::do_pull(&dots_dir));
    });
    screen.state   = UpdateState::Pulling;
    screen.pull_rx = Some(rx);
}
