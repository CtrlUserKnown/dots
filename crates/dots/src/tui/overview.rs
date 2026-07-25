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

use crate::packages::{check_dep, check_plugin, Category, DEPS, PLUGINS};
use crate::plugins::layout::{self, Cell};
use crate::plugins::PluginPaneView;
use crate::symlinks::{self, SymlinkStatus};
use crate::tui::app::{App, Screen};
use crate::tui::theme::{style_dim, style_error, style_header, style_select};

// Directional moves for dashboard focus come from the shared layout engine.
pub use crate::plugins::layout::Dir;

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

/// Default number of columns in the dashboard grid, overridable by a plugin via
/// `ui.layout{ columns = N }` (stored in [`App::grid_cols`]).
pub const DEFAULT_COLS: usize = 2;

/// Grid order, laid out left-to-right, top-to-bottom in the grid.
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
            Pane::Plugins  => " Zsh ",
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
            Pane::Configs  => None,
            Pane::Update   => None,
            Pane::Network  => None,
        }
    }

    /// Screen this pane opens on enter. Network is informational and stays on
    /// the dashboard (it refreshes on its own).
    pub fn target(self) -> Screen {
        match self {
            Pane::Configs => Screen::Configs,
            Pane::Update  => Screen::Update,
            Pane::Network => Screen::Main,
            _             => Screen::Health,
        }
    }
}

// ── layout: built-in panes + plugin panes ───────────────────────────────────

/// The combined layout cells for this dashboard: the six built-in panes (each a
/// uniform 1×1 cell) followed by any plugin panes, which carry their own
/// `span`/`size` so a plugin can make its pane wider or taller.
fn cells(app: &App) -> Vec<Cell> {
    let mut v: Vec<Cell> = PANES.iter().map(|_| Cell::new(1, 1)).collect();
    v.extend(app.plugin_panes.iter().map(|p| Cell::new(p.span, p.weight)));
    v
}

fn cols(app: &App) -> u16 {
    app.grid_cols.max(1) as u16
}

/// Total dashboard panes, built-ins plus plugin panes.
pub fn pane_count(app: &App) -> usize {
    PANES.len() + app.plugin_panes.len()
}

/// On-screen rectangle of every pane within the grid `area`, in grid order.
fn pane_rects(area: Rect, app: &App) -> Vec<Rect> {
    layout::grid(area, cols(app), &cells(app))
}

// ── focus navigation ────────────────────────────────────────────────────────

/// Move dashboard focus geometrically. Adjacency depends only on the relative
/// arrangement of panes, not on pixel size, so a synthetic area is enough and
/// no terminal dimensions are needed at key-handling time.
pub fn move_focus(app: &App, focus: usize, dir: Dir) -> usize {
    let c = cols(app);
    let area = Rect::new(0, 0, c.saturating_mul(40).max(40), 240);
    let rects = layout::grid(area, c, &cells(app));
    layout::move_focus(&rects, focus, dir)
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
    let cfgs = crate::configs::discover();
    let total = cfgs.len();
    let ok = cfgs.iter()
        .filter(|c| c.status == crate::configs::ConfigStatus::Installed)
        .count();
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
            if s.total == 0 {
                "no dotfiles configs found — enter for details".into()
            } else {
                format!("{}/{} configs installed — enter to manage", s.ok, s.total)
            }
        }
        Pane::Update => {
            if let Some(msg) = app.install_source.defer_message() {
                format!("managed externally — {msg}")
            } else if let Some(info) = &app.update_info {
                format!("update available: v{} — enter to update", info.latest)
            } else if app.update_error.is_some() {
                "update check unavailable — enter for details".into()
            } else {
                "up to date — enter for details".into()
            }
        }
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

const VERSION: &str = env!("DOTS_VERSION");

/// Whether `area` is large enough to draw the grid at all.
fn grid_fits(area: Rect) -> bool {
    area.height >= 6 && area.width >= 20
}

/// Index of the pane (built-in or plugin) covering point `(col, row)`, if any.
/// Indices past [`PANES`]`.len()` are plugin panes.
pub fn pane_at(area: Rect, app: &App, col: u16, row: u16) -> Option<usize> {
    if !grid_fits(area) { return None; }
    layout::cell_at(&pane_rects(area, app), col, row)
}

/// The one-line hint for the desc bar for whichever pane is focused — the
/// contextual summary for built-ins, or the pane's own first line for plugins.
pub fn focus_hint(app: &App, focus: usize) -> String {
    if let Some(&pane) = PANES.get(focus) {
        pane_hint(pane, app)
    } else if let Some(view) = app.plugin_panes.get(focus - PANES.len()) {
        view.lines.first().cloned().unwrap_or_else(|| format!("{} — plugin pane", view.title))
    } else {
        String::new()
    }
}

/// Renders the pane grid into `area` (the content region between header/footer).
/// `focus` is the index of the currently selected pane across built-ins and
/// plugin panes.
pub fn render_grid(f: &mut Frame, area: Rect, app: &App, focus: usize) {
    if !grid_fits(area) { return; }

    for (i, rect) in pane_rects(area, app).into_iter().enumerate() {
        let focused = i == focus;
        if let Some(&pane) = PANES.get(i) {
            draw_pane(f, rect, pane, app, focused);
        } else if let Some(view) = app.plugin_panes.get(i - PANES.len()) {
            draw_plugin_pane(f, rect, view, focused);
        }
    }
}

/// Draws a plugin-registered pane: the same chrome as a built-in, with the
/// plugin's cached `render()` lines inside.
fn draw_plugin_pane(f: &mut Frame, area: Rect, view: &PluginPaneView, focused: bool) {
    if area.width < 6 || area.height < 3 { return; }

    let border_style = if focused { style_select() } else { style_dim() };
    let title_style  = if focused { style_select() } else { style_header() };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(format!(" {} ", view.title), title_style.add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    for (i, text) in view.lines.iter().enumerate() {
        if inner.height as usize <= i { break; }
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(text.clone(), style_header()))),
            Rect { x: inner.x + 1, y: inner.y + i as u16, width: inner.width.saturating_sub(1), height: 1 },
        );
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
        Pane::Configs  => count_lines(config_summary(),  "installed", "pending", false),
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
    if app.install_source.defer_message().is_some() {
        return vec![
            Line::from(Span::styled("○ managed externally", style_dim().add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(format!("v{VERSION}"), style_dim())),
        ];
    }
    if let Some(info) = &app.update_info {
        return vec![
            Line::from(Span::styled("✗ update available", style_error().add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(format!("v{VERSION} → v{}", info.latest), style_error())),
        ];
    }
    if app.update_error.is_some() {
        return vec![
            Line::from(Span::styled("• check unavailable", style_dim().add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(format!("v{VERSION}"), style_dim())),
        ];
    }
    vec![
        Line::from(Span::styled("✓ up to date", style_select().add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(format!("v{VERSION}"), style_dim())),
    ]
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
    fn every_pane_maps_to_a_target() {
        for p in PANES {
            let _ = p.target();
            let _ = p.title();
        }
    }

    #[test]
    fn renders_plugin_panes_without_panicking() {
        use crate::config::settings::Settings;
        use crate::plugins::PluginPaneView;
        use crate::tui::app::App;
        use ratatui::{backend::TestBackend, Terminal};

        let mut app = App::new(Screen::Main, Settings::default());
        app.grid_cols = 2;
        app.plugin_panes = vec![
            PluginPaneView { id: "gh".into(), title: "GitHub".into(), span: 2, weight: 1, lines: vec!["3 PRs".into()] },
            PluginPaneView { id: "aws".into(), title: "AWS".into(), span: 1, weight: 2, lines: vec!["account 123".into()] },
        ];

        // Grid + hit-test see the extra panes.
        assert_eq!(pane_count(&app), PANES.len() + 2);
        let grid = Rect::new(0, 0, 80, 30);
        assert!(pane_at(grid, &app, 1, 1).is_some());

        // Focus can reach a plugin pane and drawing it doesn't panic.
        let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term.draw(|f| render_grid(f, grid, &app, PANES.len())).unwrap();
    }
}
