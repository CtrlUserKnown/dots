//! The dashboard shown on the main screen: a grid of summary panes giving an
//! at-a-glance view of symlinks, tools, plugins, premade configs and updates.
//! Each pane is a summary; pressing enter drills into the matching detail view.

use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::installer::PREMADE_CONFIGS;
use crate::packages::{check_dep, check_plugin, Category, DEPS, PLUGINS};
use crate::symlinks::{self, SymlinkStatus};
use crate::tui::app::{App, Screen};
use crate::tui::theme::{style_dim, style_error, style_header, style_select};

// ── pane model ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Symlinks,
    Tools,
    Plugins,
    Configs,
    Update,
}

/// Grid order. Rows of two, with Update spanning the final row.
pub const PANES: [Pane; 5] = [
    Pane::Symlinks,
    Pane::Tools,
    Pane::Plugins,
    Pane::Configs,
    Pane::Update,
];

impl Pane {
    fn title(self) -> &'static str {
        match self {
            Pane::Symlinks => " Symlinks ",
            Pane::Tools    => " Tools ",
            Pane::Plugins  => " Plugins ",
            Pane::Configs  => " Configs ",
            Pane::Update   => " Update ",
        }
    }

    /// The Health-screen section this pane drills into, if any.
    pub fn section(self) -> Option<&'static str> {
        match self {
            Pane::Symlinks => Some("symlinks"),
            Pane::Tools    => Some("tools"),
            Pane::Plugins  => Some("plugins"),
            Pane::Configs  => Some("premade configs"),
            Pane::Update   => None,
        }
    }

    /// Screen this pane opens on enter.
    pub fn target(self) -> Screen {
        match self {
            Pane::Update => Screen::Update,
            _            => Screen::Health,
        }
    }
}

// ── focus navigation ────────────────────────────────────────────────────────

pub enum Dir { Up, Down, Left, Right }

/// Move focus within the 2-column grid. Update (index 4) spans the last row.
pub fn move_focus(focus: usize, dir: Dir) -> usize {
    match dir {
        Dir::Right => if focus < 4 && focus.is_multiple_of(2) { focus + 1 } else { focus },
        Dir::Left  => if focus < 4 && !focus.is_multiple_of(2) { focus - 1 } else { focus },
        Dir::Down  => (focus + 2).min(PANES.len() - 1),
        Dir::Up    => focus.saturating_sub(2),
    }
}

// ── summaries ───────────────────────────────────────────────────────────────

struct Summary {
    ok:    usize,
    total: usize,
}

impl Summary {
    fn bad(&self) -> usize { self.total.saturating_sub(self.ok) }
    fn all_ok(&self) -> bool { self.total > 0 && self.ok == self.total }
}

fn symlink_summary() -> Summary {
    let links = symlinks::get_symlinks();
    let total = links.len();
    let ok = links.iter().filter(|s| symlinks::check(s) == SymlinkStatus::Ok).count();
    Summary { ok, total }
}

fn tool_summary() -> Summary {
    let tools: Vec<_> = DEPS.iter()
        .filter(|d| d.category == Category::Required || d.category == Category::Optional)
        .collect();
    let total = tools.len();
    let ok = tools.iter().filter(|d| check_dep(d)).count();
    Summary { ok, total }
}

fn plugin_summary() -> Summary {
    let total = PLUGINS.len();
    let ok = PLUGINS.iter().filter(|p| check_plugin(p)).count();
    Summary { ok, total }
}

fn config_summary() -> Summary {
    let total = PREMADE_CONFIGS.len();
    let ok = PREMADE_CONFIGS.iter().filter(|c| (c.dest)().exists()).count();
    Summary { ok, total }
}

// ── one-line contextual hint for the desc bar ───────────────────────────────

pub fn pane_hint(pane: Pane, app: &App) -> String {
    match pane {
        Pane::Symlinks => {
            let s = symlink_summary();
            if s.all_ok() { "all symlinks healthy — enter to view".into() }
            else { format!("{} broken symlink(s) — enter to repair", s.bad()) }
        }
        Pane::Tools => {
            let s = tool_summary();
            if s.all_ok() { "all tools installed — enter to view".into() }
            else { format!("{} tool(s) missing — enter to install", s.bad()) }
        }
        Pane::Plugins => {
            let s = plugin_summary();
            if s.all_ok() { "all zsh plugins present — enter to view".into() }
            else { format!("{} plugin(s) missing — enter to view", s.bad()) }
        }
        Pane::Configs => {
            let s = config_summary();
            format!("{}/{} premade configs applied — enter to manage", s.ok, s.total)
        }
        Pane::Update => match &app.update_status {
            None => "checking for updates…".into(),
            Some(st) if st.behind > 0 => format!("update available: v{} — enter to update", st.label),
            Some(_) => "up to date — enter for details".into(),
        },
    }
}

// ── rendering ───────────────────────────────────────────────────────────────

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Renders the pane grid into `area` (the content region between header/footer).
/// `focus` is the index into [`PANES`] of the currently selected pane.
pub fn render_grid(f: &mut Frame, area: Rect, app: &App, focus: usize) {
    if area.height < 6 || area.width < 20 { return; }

    // Three row-bands: two 2-column rows, then a full-width Update row.
    let band_h = area.height / 3;
    let rows = [
        Rect { x: area.x, y: area.y,                 width: area.width, height: band_h },
        Rect { x: area.x, y: area.y + band_h,        width: area.width, height: band_h },
        Rect { x: area.x, y: area.y + band_h * 2,    width: area.width, height: area.height - band_h * 2 },
    ];
    let mid = area.width / 2;
    let cell = |r: Rect, right: bool| Rect {
        x: if right { r.x + mid } else { r.x },
        y: r.y,
        width: if right { r.width - mid } else { mid },
        height: r.height,
    };

    let rects = [
        cell(rows[0], false), // Symlinks
        cell(rows[0], true),  // Tools
        cell(rows[1], false), // Plugins
        cell(rows[1], true),  // Configs
        rows[2],              // Update (full width)
    ];

    for (i, pane) in PANES.iter().enumerate() {
        draw_pane(f, rects[i], *pane, app, i == focus);
    }
}

fn draw_pane(f: &mut Frame, area: Rect, pane: Pane, app: &App, focused: bool) {
    if area.width < 6 || area.height < 3 { return; }

    let border_style = if focused {
        style_select()
    } else {
        style_dim()
    };
    let title_style = if focused { style_select() } else { style_header() };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(pane.title(), title_style.add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = pane_lines(pane, app);
    for (i, line) in lines.into_iter().enumerate() {
        if inner.height as usize <= i { break; }
        f.render_widget(
            Paragraph::new(line),
            Rect { x: inner.x + 1, y: inner.y + i as u16, width: inner.width.saturating_sub(1), height: 1 },
        );
    }
}

/// The 1-2 text lines shown inside a pane.
fn pane_lines(pane: Pane, app: &App) -> Vec<Line<'static>> {
    match pane {
        Pane::Symlinks => count_lines(symlink_summary(), "linked", "broken", true),
        Pane::Tools    => count_lines(tool_summary(),    "installed", "missing", true),
        Pane::Plugins  => count_lines(plugin_summary(),  "present", "missing", true),
        Pane::Configs  => count_lines(config_summary(),  "applied", "pending", false),
        Pane::Update   => update_lines(app),
    }
}

/// Big "n/total ok_word" line plus a status line. When `bad_is_error` the
/// shortfall is shown red; otherwise dim (e.g. unapplied configs aren't errors).
fn count_lines(s: Summary, ok_word: &str, bad_word: &str, bad_is_error: bool) -> Vec<Line<'static>> {
    let head_style = if s.all_ok() { style_select() } else if bad_is_error { style_error() } else { style_header() };
    let head = Line::from(Span::styled(
        format!("{}/{} {}", s.ok, s.total, ok_word),
        head_style.add_modifier(Modifier::BOLD),
    ));

    let status = if s.all_ok() {
        Line::from(Span::styled("✓ all good", style_select()))
    } else {
        let sty = if bad_is_error { style_error() } else { style_dim() };
        let mark = if bad_is_error { "✗" } else { "•" };
        Line::from(Span::styled(format!("{mark} {} {bad_word}", s.bad()), sty))
    };

    vec![head, status]
}

fn update_lines(app: &App) -> Vec<Line<'static>> {
    match &app.update_status {
        None => vec![
            Line::from(Span::styled("checking…", style_dim())),
            Line::from(Span::styled(format!("v{VERSION}"), style_dim())),
        ],
        Some(st) if st.behind > 0 => vec![
            Line::from(Span::styled("✗ update available", style_error().add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(format!("v{VERSION} → v{}", st.label), style_error())),
        ],
        Some(_) => vec![
            Line::from(Span::styled("✓ up to date", style_select().add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(format!("v{VERSION}"), style_dim())),
        ],
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_moves_within_grid() {
        assert_eq!(move_focus(0, Dir::Right), 1);
        assert_eq!(move_focus(1, Dir::Left), 0);
        assert_eq!(move_focus(0, Dir::Down), 2);
        assert_eq!(move_focus(2, Dir::Down), 4);
        assert_eq!(move_focus(4, Dir::Up), 2);
        // right on an already-right cell stays put
        assert_eq!(move_focus(1, Dir::Right), 1);
        // update pane has no horizontal neighbour
        assert_eq!(move_focus(4, Dir::Right), 4);
        assert_eq!(move_focus(4, Dir::Left), 4);
        // clamped at edges
        assert_eq!(move_focus(0, Dir::Up), 0);
        assert_eq!(move_focus(4, Dir::Down), 4);
    }

    #[test]
    fn every_pane_maps_to_a_target() {
        for p in PANES {
            let _ = p.target();
            let _ = p.title();
        }
    }
}
