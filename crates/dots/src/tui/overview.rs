//! The dashboard shown on the main screen: a grid of summary panes giving an
//! at-a-glance view of symlinks, tools, plugins, premade configs, updates and
//! live network status. Each pane is a summary; pressing enter drills into the
//! matching detail view where one exists.

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
    Network,
}

/// Number of columns in the dashboard grid.
const COLS: usize = 2;

/// Grid order, laid out left-to-right, top-to-bottom in a 2-column grid.
pub const PANES: [Pane; 6] = [
    Pane::Symlinks,
    Pane::Tools,
    Pane::Plugins,
    Pane::Configs,
    Pane::Update,
    Pane::Network,
];

impl Pane {
    fn title(self) -> &'static str {
        match self {
            Pane::Symlinks => " Symlinks ",
            Pane::Tools    => " Tools ",
            Pane::Plugins  => " Plugins ",
            Pane::Configs  => " Configs ",
            Pane::Update   => " Update ",
            Pane::Network  => " Network ",
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
            Pane::Network  => None,
        }
    }

    /// Screen this pane opens on enter. Network is informational and stays on
    /// the dashboard (it refreshes on its own).
    pub fn target(self) -> Screen {
        match self {
            Pane::Update  => Screen::Update,
            Pane::Network => Screen::Main,
            _             => Screen::Health,
        }
    }
}

// ── focus navigation ────────────────────────────────────────────────────────

pub enum Dir { Up, Down, Left, Right }

/// Move focus within the 2-column grid.
pub fn move_focus(focus: usize, dir: Dir) -> usize {
    let last = PANES.len() - 1;
    let in_left_col = focus.is_multiple_of(COLS);
    match dir {
        Dir::Right => if in_left_col { (focus + 1).min(last) } else { focus },
        Dir::Left  => if in_left_col { focus } else { focus - 1 },
        Dir::Down  => if focus + COLS <= last { focus + COLS } else { focus },
        Dir::Up    => if focus >= COLS { focus - COLS } else { focus },
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
        Pane::Network => match &app.network {
            None => "probing network…".into(),
            Some(n) if !n.online => "offline — no connectivity".into(),
            Some(n) => {
                let net = n.name.as_deref().unwrap_or("network");
                let lat = n.latency_ms.map(|ms| format!("{ms}ms")).unwrap_or_else(|| "—".into());
                let dns = n.dns.first().map(String::as_str).unwrap_or("unknown");
                let vpn = if n.vpn { "VPN on" } else { "VPN off" };
                format!("{net} · {lat} · {vpn} · DNS {dns} — live")
            }
        },
    }
}

// ── rendering ───────────────────────────────────────────────────────────────

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Whether `area` is large enough to draw the grid at all.
fn grid_fits(area: Rect) -> bool {
    area.height >= 6 && area.width >= 20
}

/// The on-screen rectangle of pane `i` within the grid `area`. Shared by both
/// rendering and mouse hit-testing so the two never drift apart.
fn pane_cell(area: Rect, i: usize) -> Rect {
    // A uniform 2-column grid; one row per pair of panes.
    let rows = PANES.len().div_ceil(COLS) as u16;
    let band_h = area.height / rows;
    let mid = area.width / 2;
    let row = (i / COLS) as u16;
    let right = i % COLS == 1;
    // The final row absorbs any rounding remainder so it reaches the bottom.
    let y = area.y + row * band_h;
    let h = if row == rows - 1 { area.height - band_h * row } else { band_h };
    Rect {
        x:      if right { area.x + mid } else { area.x },
        y,
        width:  if right { area.width - mid } else { mid },
        height: h,
    }
}

/// Index into [`PANES`] of the pane covering point `(col, row)`, if any.
pub fn pane_at(area: Rect, col: u16, row: u16) -> Option<usize> {
    if !grid_fits(area) { return None; }
    (0..PANES.len()).find(|&i| {
        let r = pane_cell(area, i);
        col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
    })
}

/// Renders the pane grid into `area` (the content region between header/footer).
/// `focus` is the index into [`PANES`] of the currently selected pane.
pub fn render_grid(f: &mut Frame, area: Rect, app: &App, focus: usize) {
    if !grid_fits(area) { return; }

    for (i, pane) in PANES.iter().enumerate() {
        draw_pane(f, pane_cell(area, i), *pane, app, i == focus);
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
        Pane::Network  => network_lines(app),
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

/// Live network snapshot: reachability + latency, the network name, and DNS.
fn network_lines(app: &App) -> Vec<Line<'static>> {
    let Some(n) = &app.network else {
        return vec![Line::from(Span::styled("probing…", style_dim()))];
    };

    if !n.online {
        return vec![
            Line::from(Span::styled("✗ offline", style_error().add_modifier(Modifier::BOLD))),
            Line::from(Span::styled("no connectivity", style_dim())),
        ];
    }

    let head = match (n.latency_ms, n.quality()) {
        (Some(ms), Some(q)) => format!("✓ online · {ms}ms ({q})"),
        _ => "✓ online".to_string(),
    };

    let name = n.name.clone().unwrap_or_else(|| "unknown network".into());
    let dns = if n.dns.is_empty() { "DNS: unknown".to_string() } else { format!("DNS: {}", n.dns.join(", ")) };
    let vpn = if n.vpn {
        Line::from(Span::styled("🔒 VPN on", style_select()))
    } else {
        Line::from(Span::styled("○ VPN off", style_dim()))
    };

    // VPN comes before DNS so it survives first when a short pane clips lines.
    vec![
        Line::from(Span::styled(head, style_select().add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(format!("⌂ {name}"), style_header())),
        vpn,
        Line::from(Span::styled(dns, style_dim())),
    ]
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_moves_within_grid() {
        // 2-column grid: [0 1 / 2 3 / 4 5]
        assert_eq!(move_focus(0, Dir::Right), 1);
        assert_eq!(move_focus(1, Dir::Left), 0);
        assert_eq!(move_focus(0, Dir::Down), 2);
        assert_eq!(move_focus(2, Dir::Down), 4);
        assert_eq!(move_focus(4, Dir::Up), 2);
        // right on an already-right cell stays put
        assert_eq!(move_focus(1, Dir::Right), 1);
        // Update (4) and Network (5) are horizontal neighbours on the last row
        assert_eq!(move_focus(4, Dir::Right), 5);
        assert_eq!(move_focus(5, Dir::Left), 4);
        // clamped at edges
        assert_eq!(move_focus(0, Dir::Up), 0);
        assert_eq!(move_focus(4, Dir::Down), 4);
        assert_eq!(move_focus(5, Dir::Down), 5);
        assert_eq!(move_focus(5, Dir::Up), 3);
    }

    #[test]
    fn every_pane_maps_to_a_target() {
        for p in PANES {
            let _ = p.target();
            let _ = p.title();
        }
    }

    #[test]
    fn pane_at_hit_tests_the_grid() {
        // 80×18 grid → 2 cols × 3 rows, each cell 40×6.
        let area = Rect::new(0, 0, 80, 18);
        assert_eq!(pane_at(area, 5, 3), Some(0)); // top-left
        assert_eq!(pane_at(area, 45, 3), Some(1)); // top-right
        assert_eq!(pane_at(area, 5, 8), Some(2)); // mid-left
        assert_eq!(pane_at(area, 45, 15), Some(5)); // bottom-right (Network)
        assert_eq!(pane_at(area, 5, 12), Some(4)); // bottom-left (Update)
        // Every cell maps back to itself.
        for i in 0..PANES.len() {
            let r = pane_cell(area, i);
            assert_eq!(pane_at(area, r.x, r.y), Some(i));
        }
        // Too-small areas hit nothing.
        assert_eq!(pane_at(Rect::new(0, 0, 10, 4), 1, 1), None);
    }
}
