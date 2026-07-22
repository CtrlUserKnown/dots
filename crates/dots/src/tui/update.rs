//! The update screen: shows current vs. latest version and offers to apply a
//! release in place. It reads the check result held on [`App`] (populated by the
//! background check in the event loop) and owns the apply/re-check background
//! work. Mirrors the self-update UX in `ssm`.

use std::sync::mpsc::Receiver;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::tui::app::{App, Screen};
use crate::tui::theme::{style_dim, style_header, style_select};
use crate::tui::{draw_desc, draw_footer, draw_header};
use crate::update::{self as update_core, UpdateInfo};

const VERSION: &str = env!("DOTS_VERSION");

// ── view state ────────────────────────────────────────────────────────────────

pub struct UpdateScreen {
    /// Background re-check kicked off from this screen (separate from the initial
    /// check the event loop runs on launch).
    recheck_rx: Option<Receiver<anyhow::Result<Option<UpdateInfo>>>>,
    apply_rx:   Option<Receiver<anyhow::Result<()>>>,
    applying:   bool,
    /// True once the swap succeeded — the view then asks for a restart.
    done:       bool,
}

impl UpdateScreen {
    pub fn new() -> Self {
        Self { recheck_rx: None, apply_rx: None, applying: false, done: false }
    }

    /// Reset transient state when (re)entering the screen. Kept for parity with
    /// the navigation flow; the underlying check result lives on `App`.
    pub fn sync_from_app(&mut self, _app: &App) {
        // A completed update stays "done" until the process restarts, so we only
        // clear the in-flight flags when nothing is running.
        if !self.applying {
            self.recheck_rx = None;
        }
    }

    /// Drain the apply/re-check channels. Called each tick from the event loop.
    /// `app` is updated in place so the dashboard summary reflects a re-check.
    pub fn pump(&mut self, app: &mut App) {
        if let Some(rx) = &self.recheck_rx {
            if let Ok(result) = rx.try_recv() {
                self.recheck_rx = None;
                match result {
                    Ok(info)  => { app.update_info = info; app.update_error = None; }
                    Err(e)    => app.update_error = Some(e.to_string()),
                }
            }
        }

        if let Some(rx) = &self.apply_rx {
            if let Ok(result) = rx.try_recv() {
                self.applying = false;
                self.apply_rx = None;
                match result {
                    Ok(())  => self.done = true,
                    Err(e)  => app.update_error = Some(format!("update failed: {e}")),
                }
            }
        }
    }

    fn start_apply(&mut self, app: &App) {
        if self.applying || self.done { return; }
        if let Some(info) = app.update_info.clone() {
            self.applying = true;
            self.apply_rx = Some(update_core::spawn_apply(info));
        }
    }

    fn start_recheck(&mut self, app: &mut App) {
        if self.applying || self.recheck_rx.is_some() { return; }
        app.update_error = None;
        // Force a check regardless of the throttle by passing a zero interval.
        self.recheck_rx = Some(update_core::spawn_check(0));
    }
}

// ── rendering ─────────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, area: Rect, app: &App, screen: &UpdateScreen) {
    draw_header(f, area, " check for updates ", VERSION);

    if area.height < 5 { return; }

    let bold = style_header().add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(format!("{:<12}", "current"), bold),
        Span::styled(format!("v{}", update_core::CURRENT), style_dim()),
    ]));

    // What we show — and which actions are live — depends on install source and
    // how far the background work has gotten.
    let footer = if screen.done {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "✓ Updated — restart dots to run the new version.",
            style_select(),
        )));
        " esc back  q quit "
    } else if screen.applying {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Downloading and installing… this can take a moment.",
            bold,
        )));
        " esc back  q quit "
    } else if let Some(msg) = app.install_source.defer_message() {
        // Package-manager install: never self-replace, just point at the manager.
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Installed via a package manager — {msg}."),
            style_dim(),
        )));
        " esc back  q quit "
    } else if let Some(info) = &app.update_info {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<12}", "latest"), bold),
            Span::styled(format!("v{}", info.latest), style_select()),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("A newer release is available.", style_header())));
        " a apply update  r re-check  esc back  q quit "
    } else if screen.recheck_rx.is_some() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("Checking for updates…", style_dim())));
        " esc back  q quit "
    } else if let Some(err) = &app.update_error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(format!("✗ {err}"), style_dim())));
        " r re-check  esc back  q quit "
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("You're on the latest version.", style_dim())));
        " r re-check  esc back  q quit "
    };

    for (i, line) in lines.into_iter().enumerate() {
        let y = area.y + 2 + i as u16;
        if y + 4 >= area.y + area.height { break; }
        f.render_widget(
            Paragraph::new(line),
            Rect { x: area.x + 2, y, width: area.width.saturating_sub(4), height: 1 },
        );
    }

    draw_desc(f, area, "", app.flash.as_ref());
    draw_footer(f, area, footer);
}

// ── key handling ─────────────────────────────────────────────────────────────

pub fn handle_key(app: &mut App, screen: &mut UpdateScreen, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.screen = Screen::Main;
            app.flash  = None;
        }
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('a') => {
            if app.update_info.is_some() && app.install_source.defer_message().is_none() {
                screen.start_apply(app);
            }
        }
        KeyCode::Char('r') => screen.start_recheck(app),
        _ => {}
    }
}
