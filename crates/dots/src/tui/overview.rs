//! The dashboard shown on the main screen.
//!
//! The dashboard is a set of **zones** (see [`crate::zones`]), each holding an
//! ordered list of **widgets**. A widget is either a built-in summary tile —
//! symlinks, tools, configs, Lua plugins, live network — or a pane registered
//! by a plugin. Zones come from `~/.dots/layout.toml`, so the arrangement is the
//! user's to change; with no file the default layout is a single untitled zone
//! holding the five built-ins in two columns, which is exactly the fixed grid
//! this screen had before zones existed.
//!
//! Geometry is two nested passes of the same [`layout::grid`] engine: zones into
//! the screen, then each zone's widgets into the zone. Focus and hit-testing
//! work on the flattened widget list, so one index addresses any tile in any
//! zone regardless of how the layout is arranged.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};
use tui_core::{status_row, summary_line, Status};

use crate::packages::{check_dep, check_plugin, Category, Dep, Plugin, DEPS, PLUGINS};
use crate::plugins::layout::{self, Cell};
use crate::plugins::PluginPaneView;
use crate::symlinks::{self, Symlink, SymlinkStatus};
use crate::tui::app::{App, Screen};
use crate::tui::theme::style_text;
use crate::zones::{Resolved, ResolvedZone, Widget};

// Directional moves for dashboard focus come from the shared layout engine.
pub use crate::plugins::layout::Dir;

// ── the built-in widgets ────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pane {
    Symlinks,
    Tools,
    Configs,
    Plugins,
    Network,
}

impl Pane {
    /// Map a layout widget id onto a built-in. Ids are the strings in
    /// [`crate::zones::BUILTIN_WIDGETS`]; anything else is a plugin pane.
    pub fn from_id(id: &str) -> Option<Pane> {
        match id {
            "symlinks" => Some(Pane::Symlinks),
            "tools"    => Some(Pane::Tools),
            "configs"  => Some(Pane::Configs),
            "plugins"  => Some(Pane::Plugins),
            "network"  => Some(Pane::Network),
            _          => None,
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Pane::Symlinks => " Symlinks ",
            Pane::Tools    => " Tools ",
            Pane::Configs  => " Configs ",
            Pane::Plugins  => " Plugins ",
            Pane::Network  => " Network ",
        }
    }

    /// Screen this pane opens on enter. Symlinks and Tools each drill into
    /// their own screen — they are separate concerns and never share one.
    /// Network and Plugins are informational and stay on the dashboard;
    /// Plugins has no drill-down screen, since `dots plugins list` covers the
    /// detail this tile only summarizes.
    pub fn target(self) -> Screen {
        match self {
            Pane::Symlinks => Screen::Symlinks,
            Pane::Tools    => Screen::Tools,
            Pane::Configs  => Screen::Configs,
            Pane::Plugins | Pane::Network => Screen::Main,
        }
    }
}

/// What the focused dashboard slot actually is, for the key handler.
pub enum Focus {
    Builtin(Pane),
    /// Index into [`App::plugin_panes`].
    Plugin(usize),
}

/// Resolve focus index `i` to the thing it points at.
pub fn focused(app: &App, i: usize) -> Option<Focus> {
    match app.dash.flat().get(i)? {
        Widget::Builtin(id) => Pane::from_id(id).map(Focus::Builtin),
        Widget::Plugin(p)   => Some(Focus::Plugin(*p)),
    }
}

// ── geometry ────────────────────────────────────────────────────────────────

/// A zone draws a labelled border only when it is titled and there is room for
/// one; this must agree between measuring and drawing or clicks land on the
/// wrong tile, so both go through here.
fn zone_is_boxed(zone: &ResolvedZone, area: Rect) -> bool {
    zone.title.is_some() && area.width >= 3 && area.height >= 3
}

/// The region inside a zone that its widgets are laid out in.
fn zone_inner(zone: &ResolvedZone, area: Rect) -> Rect {
    if zone_is_boxed(zone, area) {
        Rect { x: area.x + 1, y: area.y + 1, width: area.width - 2, height: area.height - 2 }
    } else {
        area
    }
}

/// One layout cell per widget: built-ins are uniform, plugin panes carry the
/// `span`/`size` they registered with.
fn widget_cell(w: &Widget, panes: &[PluginPaneView]) -> Cell {
    match w {
        Widget::Builtin(_) => Cell::new(1, 1),
        Widget::Plugin(i)  => panes
            .get(*i)
            .map(|p| Cell::new(p.span, p.weight))
            .unwrap_or_else(|| Cell::new(1, 1)),
    }
}

/// On-screen rectangle of every zone within `area`, in layout order.
fn zone_rects(area: Rect, dash: &Resolved) -> Vec<Rect> {
    let cells: Vec<Cell> = dash.zones.iter().map(|z| Cell::new(z.span, z.weight)).collect();
    layout::grid(area, dash.columns, &cells)
}

/// On-screen rectangle of every widget, in [`Resolved::flat`] order — the order
/// focus indices address.
fn widget_rects(area: Rect, dash: &Resolved, panes: &[PluginPaneView]) -> Vec<Rect> {
    let mut out = Vec::with_capacity(dash.len());
    for (zone, rect) in dash.zones.iter().zip(zone_rects(area, dash)) {
        let cells: Vec<Cell> = zone.widgets.iter().map(|w| widget_cell(w, panes)).collect();
        out.extend(layout::grid(zone_inner(zone, rect), zone.columns, &cells));
    }
    out
}

/// Total dashboard widgets across every zone.
pub fn pane_count(app: &App) -> usize {
    app.dash.len()
}

// ── focus navigation ────────────────────────────────────────────────────────

/// Move dashboard focus geometrically. Adjacency depends only on the relative
/// arrangement of widgets, not on pixel size, so a synthetic area is enough and
/// no terminal dimensions are needed at key-handling time.
pub fn move_focus(app: &App, focus: usize, dir: Dir) -> usize {
    let w = (app.dash.columns.max(1)).saturating_mul(40).max(40);
    let area = Rect::new(0, 0, w, 240);
    layout::move_focus(&widget_rects(area, &app.dash, &app.plugin_panes), focus, dir)
}

// ── dashboard data cache ────────────────────────────────────────────────────

/// A snapshot of everything the Symlinks/Tools/Configs tiles show, recomputed
/// on a timer rather than every frame.
///
/// Each render used to re-read the symlink manifest and the configs
/// directory from disk and re-spawn a `which` subprocess per dependency —
/// unnoticeable once, ruinous at a 60Hz redraw. [`App::run`] refreshes this
/// on a background thread instead (mirroring the network probe below it) and
/// the render path only ever reads the last snapshot.
pub struct DashCache {
    symlinks:    Vec<(Symlink, SymlinkStatus)>,
    tools:       Vec<(&'static Dep, bool)>,
    zsh_plugins: Vec<(&'static Plugin, bool)>,
    /// Each config alongside its precomputed `(ok, total)` link counts, so
    /// [`crate::configs::Config::link_counts`] — itself a symlink stat per
    /// link — also runs only once per refresh, not once per frame.
    configs:     Vec<(crate::configs::Config, usize, usize)>,
}

impl DashCache {
    /// Does the actual I/O: reads the symlink manifest, spawns a `which` per
    /// dependency, and scans the configs directory. Call off the render
    /// thread — see [`App::run`].
    pub fn compute() -> Self {
        let symlinks = symlinks::get_symlinks()
            .into_iter()
            .map(|s| { let status = symlinks::check(&s); (s, status) })
            .collect();
        let tools = DEPS.iter()
            .filter(|d| d.category == Category::Required || d.category == Category::Optional)
            .map(|d| (d, check_dep(d)))
            .collect();
        let zsh_plugins = PLUGINS.iter().map(|p| (p, check_plugin(p))).collect();
        let configs = crate::configs::discover()
            .into_iter()
            .map(|c| { let (ok, total) = c.link_counts(); (c, ok, total) })
            .collect();
        Self { symlinks, tools, zsh_plugins, configs }
    }
}

impl Default for DashCache {
    /// Empty, not computed — so building an `App` in a test never touches
    /// the filesystem or shells out. [`App::run`] fills it in for real before
    /// the event loop starts.
    fn default() -> Self {
        Self { symlinks: Vec::new(), tools: Vec::new(), zsh_plugins: Vec::new(), configs: Vec::new() }
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

fn symlink_summary(cache: &DashCache) -> Summary {
    let total = cache.symlinks.len();
    let ok = cache.symlinks.iter().filter(|(_, status)| *status == SymlinkStatus::Ok).count();
    Summary { ok, total }
}

/// Tools covers both installed CLI deps and zsh plugins — the dashboard tile
/// and the Health screen's "tools" section (with zsh as a subheading) agree.
fn tool_summary(cache: &DashCache) -> Summary {
    let total = cache.tools.len() + cache.zsh_plugins.len();
    let ok = cache.tools.iter().filter(|(_, ok)| *ok).count()
        + cache.zsh_plugins.iter().filter(|(_, ok)| *ok).count();
    Summary { ok, total }
}

fn config_summary(cache: &DashCache) -> Summary {
    let total = cache.configs.len();
    let ok = cache.configs.iter()
        .filter(|(c, ..)| c.status == crate::configs::ConfigStatus::Installed)
        .count();
    Summary { ok, total }
}

/// Lua plugins in `~/.dots/plugins/` — distinct from the zsh plugins folded
/// into the Tools tile. `ok` counts files that loaded without error.
fn plugin_summary(app: &App) -> Summary {
    let total = app.plugin_infos.len();
    let ok = app.plugin_infos.iter().filter(|p| p.error.is_none()).count();
    Summary { ok, total }
}

// ── one-line contextual hint for the desc bar ───────────────────────────────

pub fn pane_hint(pane: Pane, app: &App) -> String {
    match pane {
        Pane::Symlinks => {
            let s = symlink_summary(&app.dash_cache);
            if s.all_ok() { "all symlinks healthy — enter to view".into() }
            else { format!("{} broken symlink(s) — enter to repair", s.bad()) }
        }
        Pane::Tools => {
            let s = tool_summary(&app.dash_cache);
            if s.all_ok() { "all tools installed — enter to view".into() }
            else { format!("{} tool(s) missing — enter to install", s.bad()) }
        }
        Pane::Configs => {
            let s = config_summary(&app.dash_cache);
            if s.total == 0 {
                "no dotfiles configs found — enter for details".into()
            } else {
                format!("{}/{} configs installed — enter to manage", s.ok, s.total)
            }
        }
        Pane::Plugins => {
            let s = plugin_summary(app);
            if s.total == 0 {
                "no lua plugins in ~/.dots/plugins".into()
            } else if s.all_ok() {
                "all plugins loaded".into()
            } else {
                format!("{} plugin(s) failed to load — see `dots plugins list`", s.bad())
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

/// Whether `area` is large enough to draw the grid at all.
fn grid_fits(area: Rect) -> bool {
    area.height >= 6 && area.width >= 20
}

/// Index of the widget covering point `(col, row)`, if any.
pub fn pane_at(area: Rect, app: &App, col: u16, row: u16) -> Option<usize> {
    if !grid_fits(area) { return None; }
    layout::cell_at(&widget_rects(area, &app.dash, &app.plugin_panes), col, row)
}

/// The one-line hint for the desc bar for whichever widget is focused — the
/// contextual summary for built-ins, or the pane's own first line for plugins.
pub fn focus_hint(app: &App, focus: usize) -> String {
    match focused(app, focus) {
        Some(Focus::Builtin(pane)) => pane_hint(pane, app),
        Some(Focus::Plugin(i)) => app
            .plugin_panes
            .get(i)
            .map(|v| v.lines.first().cloned().unwrap_or_else(|| format!("{} — plugin pane", v.title)))
            .unwrap_or_default(),
        None => String::new(),
    }
}

/// Renders the dashboard into `area` (the content region between header and
/// desc/footer bars). `focus` indexes the flattened widget list.
pub fn render_grid(f: &mut Frame, area: Rect, app: &App, focus: usize) {
    if !grid_fits(area) { return; }

    let mut i = 0;
    for (zone, rect) in app.dash.zones.iter().zip(zone_rects(area, &app.dash)) {
        if zone_is_boxed(zone, rect) {
            let title = zone.title.clone().unwrap_or_default();
            f.render_widget(tui_core::block(&title, false), rect);
        }

        let cells: Vec<Cell> = zone.widgets.iter().map(|w| widget_cell(w, &app.plugin_panes)).collect();
        for (w, r) in zone.widgets.iter().zip(layout::grid(zone_inner(zone, rect), zone.columns, &cells)) {
            let is_focused = i == focus;
            match w {
                Widget::Builtin(id) => {
                    if let Some(pane) = Pane::from_id(id) {
                        draw_pane(f, r, pane, app, is_focused);
                    }
                }
                Widget::Plugin(p) => {
                    if let Some(view) = app.plugin_panes.get(*p) {
                        draw_plugin_pane(f, r, view, is_focused);
                    }
                }
            }
            i += 1;
        }
    }
}

/// Draws a plugin-registered pane: the same chrome as a built-in, with the
/// plugin's cached `render()` lines inside.
fn draw_plugin_pane(f: &mut Frame, area: Rect, view: &PluginPaneView, focused: bool) {
    if area.width < 6 || area.height < 3 { return; }

    let block = tui_core::block(&view.title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    for (i, text) in view.lines.iter().enumerate() {
        if inner.height as usize <= i { break; }
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(text.clone(), style_text()))),
            body_row(inner, i as u16),
        );
    }
}

/// One text row inside a block body, inset by a column so content doesn't sit
/// flush against the border.
fn body_row(inner: Rect, i: u16) -> Rect {
    Rect {
        x: inner.x + 1,
        y: inner.y + i,
        width: inner.width.saturating_sub(2),
        height: 1,
    }
}

fn draw_pane(f: &mut Frame, area: Rect, pane: Pane, app: &App, focused: bool) {
    if area.width < 6 || area.height < 3 { return; }

    let block = tui_core::block(pane.title(), focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height == 0 || inner.width < 4 {
        return;
    }

    let body_width = inner.width.saturating_sub(2);
    let summary = pane_summary(pane, app);

    // The summary is pinned to the last body row and the item rows fill what is
    // left above it, so a tile keeps its footer no matter how short it gets.
    let has_summary = !summary.is_empty() && inner.height >= 2;
    let room = if has_summary { inner.height - 1 } else { inner.height } as usize;

    for (i, row) in pane_rows(pane, app).into_iter().take(room).enumerate() {
        f.render_widget(
            Paragraph::new(row.render(body_width)),
            body_row(inner, i as u16),
        );
    }

    if has_summary {
        f.render_widget(
            Paragraph::new(summary_line(&summary)),
            body_row(inner, inner.height - 1),
        );
    }
}

// ── tile contents ───────────────────────────────────────────────────────────

/// One line inside a tile: a status bullet, a left-aligned name, and
/// right-aligned metadata.
struct Row {
    status: Status,
    name:   String,
    meta:   String,
}

impl Row {
    fn new(status: Status, name: impl Into<String>, meta: impl Into<String>) -> Self {
        Self { status, name: name.into(), meta: meta.into() }
    }

    fn render(&self, width: u16) -> Line<'static> {
        status_row(self.status, &self.name, &self.meta, width)
    }
}

/// Sort problems to the front. A tile shows only the first few rows that fit,
/// so what is wrong must be what survives the truncation.
fn problems_first(mut rows: Vec<Row>) -> Vec<Row> {
    fn rank(s: Status) -> u8 {
        match s {
            Status::Bad => 0,
            Status::Partial => 1,
            Status::Pending => 2,
            Status::Ok => 3,
        }
    }
    rows.sort_by_key(|r| rank(r.status));
    rows
}

/// The item rows for a tile, most important first.
fn pane_rows(pane: Pane, app: &App) -> Vec<Row> {
    match pane {
        Pane::Symlinks => problems_first(
            app.dash_cache.symlinks
                .iter()
                .map(|(s, status)| {
                    let (status, label) = match status {
                        SymlinkStatus::Ok          => (Status::Ok, "linked"),
                        SymlinkStatus::Missing     => (Status::Bad, "missing"),
                        SymlinkStatus::Broken      => (Status::Bad, "broken"),
                        SymlinkStatus::NotALink    => (Status::Partial, "not a link"),
                        SymlinkStatus::WrongTarget => (Status::Partial, "wrong target"),
                    };
                    Row::new(status, file_label(&s.link), label)
                })
                .collect(),
        ),
        Pane::Tools => problems_first(
            app.dash_cache.tools
                .iter()
                .map(|(d, ok)| Row::new(
                    if *ok { Status::Ok } else { Status::Bad },
                    d.bin,
                    if *ok { "installed" } else { "missing" },
                ))
                .chain(app.dash_cache.zsh_plugins.iter().map(|(p, ok)| Row::new(
                    if *ok { Status::Ok } else { Status::Bad },
                    p.name,
                    if *ok { "zsh" } else { "missing" },
                )))
                .collect(),
        ),
        Pane::Configs => problems_first(
            app.dash_cache.configs
                .iter()
                .map(|(c, ok, total)| {
                    // An unapplied config is pending, not broken — it is a
                    // choice the user has not made yet.
                    let status = match c.status {
                        crate::configs::ConfigStatus::Installed => Status::Ok,
                        _ if *ok > 0 => Status::Partial,
                        _ => Status::Pending,
                    };
                    Row::new(status, c.name.clone(), format!("{ok}/{total}"))
                })
                .collect(),
        ),
        Pane::Plugins => problems_first(
            app.plugin_infos
                .iter()
                .map(|p| match &p.error {
                    Some(_) => Row::new(Status::Bad, p.name.clone(), "failed"),
                    None => Row::new(
                        Status::Ok,
                        p.name.clone(),
                        match p.pane_ids.len() {
                            0 => "loaded".to_string(),
                            1 => "1 pane".to_string(),
                            n => format!("{n} panes"),
                        },
                    ),
                })
                .collect(),
        ),
        Pane::Network => network_rows(app),
    }
}

/// The network tile is a status readout rather than a list of things that can
/// break, so its rows are fixed facts about the current connection.
fn network_rows(app: &App) -> Vec<Row> {
    let Some(n) = &app.network else {
        return vec![Row::new(Status::Pending, "probing…", "")];
    };
    if !n.online {
        return vec![Row::new(Status::Bad, "offline", "no connectivity")];
    }

    let latency = match (n.latency_ms, n.quality()) {
        (Some(ms), Some(q)) => format!("{ms}ms · {q}"),
        (Some(ms), None) => format!("{ms}ms"),
        _ => String::new(),
    };

    let mut rows = vec![Row::new(
        Status::Ok,
        n.name.clone().unwrap_or_else(|| "unknown network".into()),
        latency,
    )];
    rows.push(if n.vpn {
        Row::new(Status::Ok, "VPN", "on")
    } else {
        Row::new(Status::Pending, "VPN", "off")
    });
    if !n.dns.is_empty() {
        rows.push(Row::new(Status::Ok, "DNS", n.dns.join(", ")));
    }
    rows
}

/// The dim footer inside a tile: `4 links · 3 ok · 1 missing`.
fn pane_summary(pane: Pane, app: &App) -> Vec<String> {
    fn counts(s: &Summary, noun: &str, bad_word: &str) -> Vec<String> {
        let mut parts = vec![format!("{} {noun}", s.total)];
        if s.total == 0 {
            return parts;
        }
        parts.push(format!("{} ok", s.ok));
        if s.bad() > 0 {
            parts.push(format!("{} {bad_word}", s.bad()));
        }
        parts
    }

    match pane {
        Pane::Symlinks => counts(&symlink_summary(&app.dash_cache), "links", "missing"),
        Pane::Tools    => counts(&tool_summary(&app.dash_cache), "tools", "missing"),
        Pane::Configs  => {
            let s = config_summary(&app.dash_cache);
            let mut parts = vec![format!("{} configs", s.total)];
            if s.total > 0 {
                parts.push(format!("{} installed", s.ok));
                if s.bad() > 0 {
                    parts.push(format!("{} pending", s.bad()));
                }
            }
            parts
        }
        Pane::Plugins => {
            let s = plugin_summary(app);
            if s.total == 0 {
                return vec!["no lua plugins".into()];
            }
            counts(&s, "plugins", "failed")
        }
        Pane::Network => match &app.network {
            Some(n) if n.online => vec!["live · refreshed every 5s".into()],
            Some(_) => vec!["offline".into()],
            None => vec![],
        },
    }
}

/// The display name for a symlink: its file name, which is what the user
/// recognises, falling back to the full path when there isn't one.
fn file_label(path: &std::path::Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::Settings;
    use crate::zones::{Layout, Zone, BUILTIN_WIDGETS};
    use ratatui::{backend::TestBackend, Terminal};

    fn app_with(layout: Layout, panes: Vec<PluginPaneView>) -> App {
        let mut app = App::new(Screen::Main, Settings::default());
        app.layout = layout;
        app.plugin_panes = panes;
        app.rebuild_dashboard();
        app
    }

    fn plugin(id: &str, zone: Option<&str>, span: u16, weight: u16) -> PluginPaneView {
        PluginPaneView {
            id: id.into(),
            title: id.into(),
            zone: zone.map(str::to_string),
            span,
            weight,
            lines: vec![format!("{id} line")],
        }
    }

    #[test]
    fn every_builtin_id_maps_to_a_pane() {
        for id in BUILTIN_WIDGETS {
            let p = Pane::from_id(id).expect("built-in widget id must resolve to a pane");
            let _ = p.target();
            let _ = p.title();
        }
        assert!(Pane::from_id("not-a-pane").is_none());
    }

    /// The default layout must lay out exactly like the pre-zones fixed grid:
    /// five 1×1 tiles, two columns, no zone chrome eating a border.
    #[test]
    fn default_layout_reproduces_the_legacy_grid() {
        let app = app_with(Layout::default(), vec![]);
        let area = Rect::new(0, 0, 80, 18);
        let rects = widget_rects(area, &app.dash, &app.plugin_panes);

        assert_eq!(rects.len(), 5);
        assert_eq!(rects[0], Rect::new(0, 0, 40, 6));
        assert_eq!(rects[1], Rect::new(40, 0, 40, 6));
        assert_eq!(rects[2], Rect::new(0, 6, 40, 6));
        assert_eq!(rects[4], Rect::new(0, 12, 80, 6)); // last row's lone tile
    }

    #[test]
    fn renders_plugin_panes_without_panicking() {
        let app = app_with(
            Layout::default(),
            vec![plugin("gh", None, 2, 1), plugin("aws", None, 1, 2)],
        );

        assert_eq!(pane_count(&app), BUILTIN_WIDGETS.len() + 2);
        let grid = Rect::new(0, 0, 80, 30);
        assert!(pane_at(grid, &app, 1, 1).is_some());

        let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term.draw(|f| render_grid(f, grid, &app, BUILTIN_WIDGETS.len())).unwrap();
    }

    #[test]
    fn a_titled_zone_insets_its_widgets_by_its_border() {
        let boxed = Layout {
            columns: 1,
            zones: vec![Zone {
                title: Some("Status".into()),
                widgets: vec!["symlinks".into()],
                ..Zone::new("status")
            }],
        };
        let app = app_with(boxed, vec![]);
        let area = Rect::new(0, 0, 40, 10);
        assert_eq!(widget_rects(area, &app.dash, &app.plugin_panes)[0], Rect::new(1, 1, 38, 8));

        // Untitled, the same zone hands its full area to the widget.
        let bare = Layout {
            columns: 1,
            zones: vec![Zone { widgets: vec!["symlinks".into()], ..Zone::new("status") }],
        };
        let app = app_with(bare, vec![]);
        assert_eq!(widget_rects(area, &app.dash, &app.plugin_panes)[0], area);
    }

    #[test]
    fn focus_walks_across_zone_boundaries() {
        // Two stacked single-widget zones: down from the top lands in the next.
        let layout = Layout {
            columns: 1,
            zones: vec![
                Zone { widgets: vec!["symlinks".into()], ..Zone::new("top") },
                Zone { widgets: vec!["tools".into()], ..Zone::new("bottom") },
            ],
        };
        let app = app_with(layout, vec![]);
        assert_eq!(move_focus(&app, 0, Dir::Down), 1);
        assert_eq!(move_focus(&app, 1, Dir::Up), 0);
        assert_eq!(move_focus(&app, 1, Dir::Down), 1, "clamped at the bottom");
    }

    #[test]
    fn focus_resolves_to_the_right_widget_kind() {
        let layout = Layout {
            columns: 1,
            zones: vec![Zone {
                widgets: vec!["network".into(), "gh".into()],
                ..Zone::new("main")
            }],
        };
        let app = app_with(layout, vec![plugin("gh", None, 1, 1)]);

        assert!(matches!(focused(&app, 0), Some(Focus::Builtin(Pane::Network))));
        assert!(matches!(focused(&app, 1), Some(Focus::Plugin(0))));
        assert!(focused(&app, 2).is_none());
    }

    #[test]
    fn a_pane_placed_in_a_zone_is_drawn_inside_it() {
        let layout = Layout {
            columns: 2,
            zones: vec![
                Zone { span: 1, widgets: vec!["symlinks".into()], ..Zone::new("left") },
                Zone { span: 1, catch_all: true, ..Zone::new("right") },
            ],
        };
        let app = app_with(layout, vec![plugin("gh", Some("right"), 1, 1)]);
        let area = Rect::new(0, 0, 80, 12);
        let rects = widget_rects(area, &app.dash, &app.plugin_panes);

        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].x, 0, "the built-in stays in the left zone");
        assert_eq!(rects[1].x, 40, "the plugin pane lands in the right zone");
    }

    #[test]
    fn hit_testing_agrees_with_what_is_drawn() {
        let layout = Layout {
            columns: 2,
            zones: vec![
                Zone { title: Some("Boxed".into()), span: 2, widgets: vec!["symlinks".into(), "tools".into()], columns: 2, ..Zone::new("a") },
                Zone { span: 2, widgets: vec!["network".into()], ..Zone::new("b") },
            ],
        };
        let app = app_with(layout, vec![]);
        let area = Rect::new(0, 0, 80, 20);
        let rects = widget_rects(area, &app.dash, &app.plugin_panes);
        for (i, r) in rects.iter().enumerate() {
            assert_eq!(pane_at(area, &app, r.x, r.y), Some(i), "widget {i} not hit at its own origin");
        }
    }

    #[test]
    fn an_empty_zone_contributes_no_widgets() {
        let layout = Layout {
            columns: 1,
            zones: vec![
                Zone { widgets: vec!["symlinks".into()], ..Zone::new("a") },
                Zone::new("empty"),
            ],
        };
        let app = app_with(layout, vec![]);
        assert_eq!(pane_count(&app), 1);
        let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
        term.draw(|f| render_grid(f, Rect::new(0, 0, 80, 20), &app, 0)).unwrap();
    }
}
