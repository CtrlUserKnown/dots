//! The Symlinks and Tools screens.
//!
//! These are two separate screens — separate titles, separate rows, separate
//! bulk actions, and separate cursors — that share one implementation because
//! the row/cursor/install machinery is identical for both. [`Scope`] selects
//! which one a [`HealthView`] is currently showing; nothing else here knows the
//! difference.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::installer::{detect_pm, install_dep};
use crate::packages::{check_dep, check_plugin, Category, Dep, Plugin, DEPS, PLUGINS};
use crate::symlinks::{self, SymlinkStatus};
use crate::tui::{draw_desc, draw_key_bar, draw_screen_nav, FlashKind, Status};
use crate::tui::app::{App, Screen};
use crate::tui::theme::{style_block_title, style_muted, style_name, style_selected};

// ── scope ─────────────────────────────────────────────────────────────────────

/// Which of the two screens this view is currently showing.
///
/// Symlinks and tools are separate screens with separate keys, separate
/// bulk actions, and separate cursors — they only share this implementation
/// because the row/cursor/install machinery is identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Symlinks,
    Tools,
}

impl Scope {
    fn index(self) -> usize {
        match self {
            Scope::Symlinks => 0,
            Scope::Tools => 1,
        }
    }

    /// The [`Screen`] this scope is, so the nav strip lights the right tab.
    pub fn screen(self) -> Screen {
        match self {
            Scope::Symlinks => Screen::Symlinks,
            Scope::Tools => Screen::Tools,
        }
    }

    /// The scope a screen selects, if it is one of these two.
    pub fn from_screen(screen: Screen) -> Option<Scope> {
        match screen {
            Screen::Symlinks => Some(Scope::Symlinks),
            Screen::Tools => Some(Scope::Tools),
            _ => None,
        }
    }
}

// ── display model ─────────────────────────────────────────────────────────────

pub enum DisplayRow {
    /// A nested heading within a screen (e.g. "zsh" under tools).
    SubSection(&'static str),
    SymlinkItem { link: PathBuf, target: PathBuf, status: SymlinkStatus },
    DepItem     { dep: &'static Dep,    installed: bool },
    PluginItem  { plugin: &'static Plugin, installed: bool },
}

fn is_navigable(row: &DisplayRow) -> bool {
    !matches!(row, DisplayRow::SubSection(_))
}

fn row_desc(row: &DisplayRow) -> String {
    match row {
        DisplayRow::SubSection(_)                     => String::new(),
        DisplayRow::SymlinkItem { target, .. }        => format!("→ {}", home_rel(target)),
        DisplayRow::DepItem     { dep, .. }           => dep.desc.to_string(),
        DisplayRow::PluginItem  { plugin, .. }        => plugin.desc.to_string(),
    }
}

/// The rows for one screen. Each scope owns its rows entirely — nothing from
/// the other screen appears here, so the cursor can never wander across.
pub fn build_display_rows(scope: Scope) -> Vec<DisplayRow> {
    let mut rows: Vec<DisplayRow> = Vec::new();

    match scope {
        Scope::Symlinks => {
            for s in symlinks::get_symlinks() {
                let status = symlinks::check(&s);
                rows.push(DisplayRow::SymlinkItem { link: s.link, target: s.target, status });
            }
        }
        Scope::Tools => {
            for dep in DEPS {
                if dep.category == Category::Required || dep.category == Category::Optional {
                    rows.push(DisplayRow::DepItem { dep, installed: check_dep(dep) });
                }
            }

            rows.push(DisplayRow::SubSection("zsh"));
            for plugin in PLUGINS {
                rows.push(DisplayRow::PluginItem { plugin, installed: check_plugin(plugin) });
            }
        }
    }

    rows
}

// ── confirm / install state ───────────────────────────────────────────────────

#[derive(Debug)]
enum Pending {
    ConfirmInstall(String),
    Installing,
}

// ── view state ────────────────────────────────────────────────────────────────

pub struct HealthView {
    pub scope:       Scope,
    pub cursor:      usize,
    pub scroll:      usize,
    pub flash:       Option<(String, FlashKind)>,
    pub rows:        Vec<DisplayRow>,
    pub nav_indices: Vec<Option<usize>>,
    pub nav_count:   usize,
    pending:         Option<Pending>,
    install_rx:      Option<Receiver<anyhow::Result<()>>>,
    /// `(cursor, scroll)` parked per scope, so leaving a screen and coming back
    /// lands where you left it rather than at the top — the two screens keep
    /// their own place the way genuinely separate screens would.
    saved:           [(usize, usize); 2],
}

impl Default for HealthView {
    fn default() -> Self { Self::new() }
}

impl HealthView {
    pub fn new() -> Self {
        let mut v = Self {
            scope:       Scope::Symlinks,
            cursor:      0,
            scroll:      0,
            flash:       None,
            rows:        Vec::new(),
            nav_indices: Vec::new(),
            nav_count:   0,
            pending:     None,
            install_rx:  None,
            saved:       [(0, 0); 2],
        };
        v.rebuild();
        v
    }

    pub fn rebuild(&mut self) {
        self.rows = build_display_rows(self.scope);
        let mut idx = 0usize;
        self.nav_indices = self.rows.iter().map(|row| {
            if is_navigable(row) { let i = idx; idx += 1; Some(i) }
            else { None }
        }).collect();
        self.nav_count = idx;
        self.cursor = self.cursor.min(self.nav_count.saturating_sub(1));
        self.flash  = None;
        self.pending = None;
    }

    /// Switch which screen this view is showing, parking the outgoing screen's
    /// position and restoring the incoming one's. Used by the dashboard's
    /// drill-in, which targets one screen or the other.
    pub fn show(&mut self, scope: Scope) {
        if scope != self.scope {
            self.saved[self.scope.index()] = (self.cursor, self.scroll);
            let (cursor, scroll) = self.saved[scope.index()];
            self.scope = scope;
            self.cursor = cursor;
            self.scroll = scroll;
        }
        self.rebuild();
    }

    fn cursor_display_row(&self) -> usize {
        self.nav_indices.iter()
            .position(|&n| n == Some(self.cursor))
            .unwrap_or(0)
    }

    fn update_scroll(&mut self, visible: usize) {
        let dr = self.cursor_display_row();
        if dr < self.scroll {
            self.scroll = dr;
        } else if dr >= self.scroll + visible {
            self.scroll = dr.saturating_sub(visible - 1);
        }
    }

    /// True while a confirmation prompt is up or an install is running — the
    /// screen is answering a question and must keep every key.
    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Call from the main event loop to poll the background install thread.
    pub fn try_complete_install(&mut self) {
        let result = if let Some(rx) = &self.install_rx {
            rx.try_recv().ok()
        } else {
            None
        };
        if let Some(r) = result {
            self.install_rx = None;
            self.pending    = None;
            match r {
                Ok(()) => {
                    self.rebuild();
                    self.flash = Some(("✓ Installed".to_string(), FlashKind::Success));
                }
                Err(e) => {
                    self.flash = Some((format!("✗ {e}"), FlashKind::Error));
                }
            }
        }
    }
}

// ── rendering ─────────────────────────────────────────────────────────────────

/// Width of the name column, so names and their descriptions line up down the
/// screen regardless of which section a row belongs to.
const NAME_COL: usize = 22;

/// One health row: `▶ ● name          description                        meta`.
///
/// The cursor and bullet carry the state, the name is the crisp column the eye
/// scans, the description is muted supporting text, and `meta` is right-aligned
/// against the row width.
fn detail_row(cursor: bool, status: Status, name: &str, desc: &str, meta: &str, width: u16) -> Line<'static> {
    let name = clip(name, NAME_COL);
    let name_pad = NAME_COL.saturating_sub(name.chars().count()) + 1;

    // 2 cursor + 2 bullet + name column + its padding, then the description
    // shares what's left with the right-aligned meta.
    let used = 4 + NAME_COL + 1;
    let rest = (width as usize).saturating_sub(used);
    let meta_len = meta.chars().count();
    let desc_room = rest.saturating_sub(if meta_len > 0 { meta_len + 2 } else { 0 });
    let desc = clip(desc, desc_room);
    let gap = rest.saturating_sub(desc.chars().count() + meta_len);

    let mut spans = vec![
        Span::styled(if cursor { "▶ " } else { "  " }, style_selected()),
        Span::styled(format!("{} ", status.glyph()), status.style()),
        Span::styled(name, if cursor { style_selected() } else { style_name() }),
        Span::styled(" ".repeat(name_pad), style_muted()),
        Span::styled(desc, style_muted()),
    ];
    if meta_len > 0 {
        spans.push(Span::styled(" ".repeat(gap), style_muted()));
        spans.push(Span::styled(meta.to_string(), style_muted()));
    }
    Line::from(spans)
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max <= 1 {
        return String::new();
    }
    s.chars().take(max - 1).chain(std::iter::once('…')).collect()
}

pub fn render(f: &mut Frame, area: Rect, _app: &App, view: &HealthView) {
    draw_screen_nav(f, area, view.scope.screen());

    if area.height < 6 { return; }
    // A blank row under the title bar, then rows down to just above the desc
    // bar at height-4 — hence one fewer visible row than the gap costs.
    let visible   = (area.height as usize).saturating_sub(6);
    let content_y = area.y + 2;

    // Each screen can now be empty on its own (a machine with no links.toml,
    // say), where before the other screen's rows always filled the space.
    if view.rows.is_empty() {
        let (what, how) = match view.scope {
            Scope::Symlinks => (
                "No symlinks declared.",
                "Adopt a file with 'dots link add <source> <target>'.",
            ),
            Scope::Tools => (
                "No tools to check.",
                "Tools are declared in the built-in package list.",
            ),
        };
        f.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(format!("  {what}"), style_name())),
                Line::from(Span::styled(format!("  {how}"), style_muted())),
            ]),
            Rect { x: area.x, y: content_y, width: area.width, height: 2 },
        );
    }

    for (di, row) in view.rows.iter().enumerate().skip(view.scroll).take(visible) {
        let ry       = content_y + (di - view.scroll) as u16;
        let row_rect = Rect { x: area.x, y: ry, width: area.width, height: 1 };
        let nav_idx  = view.nav_indices[di];
        let is_cursor = nav_idx == Some(view.cursor);

        match row {
            DisplayRow::SubSection(label) => {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        format!("  {label}"),
                        style_block_title(),
                    ))),
                    row_rect,
                );
            }
            DisplayRow::SymlinkItem { link, target, status } => {
                let ok = *status == SymlinkStatus::Ok;
                let (desc, meta) = if ok {
                    (home_rel(target), String::new())
                } else {
                    (String::new(), status_label(status).to_string())
                };
                f.render_widget(
                    Paragraph::new(detail_row(
                        is_cursor,
                        if ok { Status::Ok } else { Status::Bad },
                        &home_rel(link),
                        &desc,
                        &meta,
                        area.width,
                    )),
                    row_rect,
                );
            }
            DisplayRow::DepItem { dep, installed } => {
                let tag = match dep.category {
                    Category::Required => "req",
                    Category::Optional => "opt",
                    Category::Dev      => "dev",
                };
                f.render_widget(
                    Paragraph::new(detail_row(
                        is_cursor,
                        if *installed { Status::Ok } else { Status::Bad },
                        dep.bin,
                        dep.desc,
                        tag,
                        area.width,
                    )),
                    row_rect,
                );
            }
            DisplayRow::PluginItem { plugin, installed } => {
                f.render_widget(
                    Paragraph::new(detail_row(
                        is_cursor,
                        if *installed { Status::Ok } else { Status::Bad },
                        plugin.name,
                        plugin.desc,
                        if *installed { "" } else { "missing" },
                        area.width,
                    )),
                    row_rect,
                );
            }
        }
    }

    // Desc bar — show confirmation prompt if pending
    let desc_text = match &view.pending {
        Some(Pending::ConfirmInstall(name)) => format!("Install {name}? [y/N]"),
        Some(Pending::Installing) => "  Installing… please wait".to_string(),
        None => {
            let dr = view.cursor_display_row();
            if dr < view.rows.len() { row_desc(&view.rows[dr]) } else { String::new() }
        }
    };
    draw_desc(f, area, &desc_text, view.flash.as_ref());

    // Bulk actions are per-screen: `r` repairs links, `i` installs tools, and
    // neither is advertised on the screen it doesn't belong to.
    let any_broken  = view.rows.iter().any(|r| matches!(r, DisplayRow::SymlinkItem { status, .. } if *status != SymlinkStatus::Ok));
    let any_missing = view.rows.iter().any(|r| matches!(r, DisplayRow::DepItem { installed: false, .. }));
    let mut hints   = vec![("j/k", "navigate")];
    if can_fix_cursor(view) { hints.push(("enter", "fix/apply")); }
    match view.scope {
        Scope::Symlinks if any_broken  => hints.push(("r", "repair all")),
        Scope::Tools    if any_missing => hints.push(("i", "install all")),
        _ => {}
    }
    hints.push(("esc", "back"));
    hints.push(("q", "quit"));
    draw_key_bar(f, area, &hints);
}

// ── key handling ──────────────────────────────────────────────────────────────

pub fn handle_key(app: &mut App, view: &mut HealthView, key: KeyEvent) {
    // If a confirmation prompt is active, only handle y/n/esc
    if let Some(pending) = view.pending.take() {
        match pending {
            Pending::Installing => {
                view.pending = Some(Pending::Installing);
                return;
            }
            Pending::ConfirmInstall(name) => {
                if key.code == KeyCode::Char('y') {
                    start_install(view, name);
                } else {
                    view.flash = None;
                }
                return;
            }
        }
    }

    let visible = view.nav_count;

    match key.code {
        KeyCode::Esc => {
            app.screen = Screen::Main;
            app.flash  = None;
        }
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => {
            if view.cursor + 1 < view.nav_count {
                view.cursor += 1;
                view.update_scroll(visible);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if view.cursor > 0 {
                view.cursor -= 1;
                view.update_scroll(visible);
            }
        }
        KeyCode::Enter => activate_cursor(view),
        // Each bulk action belongs to one screen only, so the same keystroke
        // can't reach across and act on rows the user isn't looking at.
        KeyCode::Char('r') if view.scope == Scope::Symlinks => repair_all_symlinks(view),
        KeyCode::Char('i') if view.scope == Scope::Tools    => install_all_missing(view),
        _ => {}
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn can_fix_cursor(view: &HealthView) -> bool {
    let dr = view.cursor_display_row();
    if dr >= view.rows.len() { return false; }
    match &view.rows[dr] {
        DisplayRow::SymlinkItem { status, .. } => *status != SymlinkStatus::Ok,
        DisplayRow::DepItem { installed, .. }  => !installed,
        _ => false,
    }
}

fn activate_cursor(view: &mut HealthView) {
    let dr = view.cursor_display_row();
    if dr >= view.rows.len() { return; }
    match &view.rows[dr] {
        DisplayRow::SymlinkItem { link, target, .. } => {
            let s = symlinks::Symlink { link: link.clone(), target: target.clone() };
            match symlinks::repair(&s) {
                Ok(())  => { view.rebuild(); view.flash = Some(("✓ Repaired".into(), FlashKind::Success)); }
                Err(e)  => view.flash = Some((format!("✗ {e}"), FlashKind::Error)),
            }
        }
        DisplayRow::DepItem { dep, installed: false } => {
            view.pending = Some(Pending::ConfirmInstall(dep.bin.to_string()));
            view.flash   = None;
        }
        DisplayRow::PluginItem { .. } => {
            view.flash = Some(("install plugin manually — see docs".into(), FlashKind::Info));
        }
        _ => {}
    }
}

fn start_install(view: &mut HealthView, bin: String) {
    // Find the dep by bin name
    let dep = DEPS.iter().find(|d| d.bin == bin);
    let Some(dep) = dep else {
        view.flash = Some((format!("✗ unknown dep '{bin}'"), FlashKind::Error));
        return;
    };
    let pm = detect_pm();
    let (tx, rx) = std::sync::mpsc::channel();
    let dep_copy = dep.bin; // &'static str
    std::thread::spawn(move || {
        let result = DEPS.iter().find(|d| d.bin == dep_copy)
            .map(install_dep)
            .unwrap_or_else(|| anyhow::bail!("dep not found"));
        let _ = tx.send(result);
    });
    view.pending    = Some(Pending::Installing);
    view.install_rx = Some(rx);
    view.flash      = Some((format!("  Installing {bin}…"), FlashKind::Info));
    let _ = pm; // used for detection above
}

fn repair_all_symlinks(view: &mut HealthView) {
    match symlinks::repair_all() {
        Ok(r) => {
            let mut parts = Vec::new();
            if r.repaired > 0 { parts.push(format!("{} repaired", r.repaired)); }
            if r.skipped  > 0 { parts.push(format!("{} skipped",  r.skipped)); }
            if parts.is_empty() { parts.push(format!("{} already OK", r.ok)); }
            view.flash = Some((parts.join(", "), FlashKind::Success));
        }
        Err(e) => view.flash = Some((format!("✗ {e}"), FlashKind::Error)),
    }
    view.rebuild();
}

fn install_all_missing(view: &mut HealthView) {
    let missing: Vec<&Dep> = DEPS.iter()
        .filter(|d| {
            (d.category == Category::Required || d.category == Category::Optional)
                && !check_dep(d)
        })
        .collect();

    if missing.is_empty() {
        view.flash = Some(("✓ All tools are installed".into(), FlashKind::Success));
        return;
    }

    let pm = detect_pm();
    let names: Vec<String> = missing.iter().map(|d| d.bin.to_string()).collect();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut last_err: Option<anyhow::Error> = None;
        for name in &names {
            if let Some(dep) = DEPS.iter().find(|d| d.bin == name.as_str()) {
                if let Err(e) = install_dep(dep) {
                    last_err = Some(e);
                }
            }
        }
        let _ = tx.send(last_err.map(Err).unwrap_or(Ok(())));
    });
    view.pending    = Some(Pending::Installing);
    view.install_rx = Some(rx);
    view.flash      = Some(("  Installing missing tools…".into(), FlashKind::Info));
    let _ = pm;
}

fn home_rel(path: &std::path::Path) -> String {
    let home = dirs::home_dir().unwrap_or_default();
    if let Ok(rel) = path.strip_prefix(&home) {
        format!("~/{}", rel.display())
    } else {
        path.display().to_string()
    }
}

fn status_label(s: &SymlinkStatus) -> &'static str {
    match s {
        SymlinkStatus::Ok          => "OK",
        SymlinkStatus::Missing     => "MISSING",
        SymlinkStatus::Broken      => "BROKEN",
        SymlinkStatus::NotALink    => "NOT A LINK",
        SymlinkStatus::WrongTarget => "WRONG TARGET",
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// The point of the split: neither screen may contain a row belonging to
    /// the other. If this ever fails, the two screens have merged again.
    #[test]
    fn the_two_screens_share_no_rows() {
        let links = build_display_rows(Scope::Symlinks);
        assert!(
            links.iter().all(|r| matches!(r, DisplayRow::SymlinkItem { .. })),
            "the symlinks screen must contain only symlink rows",
        );

        let tools = build_display_rows(Scope::Tools);
        assert!(
            tools.iter().all(|r| matches!(
                r,
                DisplayRow::DepItem { .. } | DisplayRow::PluginItem { .. } | DisplayRow::SubSection(_)
            )),
            "the tools screen must contain no symlink rows",
        );
    }

    #[test]
    fn each_pane_drills_into_its_own_screen() {
        use crate::tui::overview::Pane;
        assert_eq!(Pane::Symlinks.target(), Screen::Symlinks);
        assert_eq!(Pane::Tools.target(), Screen::Tools);
        assert_ne!(
            Pane::Symlinks.target(),
            Pane::Tools.target(),
            "symlinks and tools must not open the same screen",
        );
    }

    #[test]
    fn zsh_is_a_subsection_within_tools() {
        let rows = build_display_rows(Scope::Tools);
        let zsh_idx = rows.iter().position(
            |r| matches!(r, DisplayRow::SubSection(l) if *l == "zsh"),
        ).expect("zsh subsection present");
        let first_dep = rows.iter().position(|r| matches!(r, DisplayRow::DepItem { .. }))
            .expect("at least one dep");
        assert!(zsh_idx > first_dep, "zsh should come after the plain tools");
        assert!(
            rows.iter().skip(zsh_idx + 1).all(|r| matches!(r, DisplayRow::PluginItem { .. })),
            "only zsh plugins follow the zsh subsection",
        );
    }

    #[test]
    fn switching_scope_keeps_each_screens_place() {
        let mut v = HealthView::new();
        v.show(Scope::Tools);
        assert!(v.nav_count > 2, "tools screen should have rows to move through");
        v.cursor = 2;

        v.show(Scope::Symlinks);
        assert_eq!(v.scope, Scope::Symlinks);

        v.show(Scope::Tools);
        assert_eq!(v.cursor, 2, "returning to tools restores where the cursor was");
    }

    #[test]
    fn dep_items_include_git_as_installed() {
        let rows = build_display_rows(Scope::Tools);
        let git_row = rows.iter().find(|r| {
            matches!(r, DisplayRow::DepItem { dep, .. } if dep.bin == "git")
        });
        assert!(git_row.is_some(), "git dep should be in display rows");
        if let Some(DisplayRow::DepItem { installed, .. }) = git_row {
            assert!(*installed, "git should be installed (required to build this binary)");
        }
    }

    #[test]
    fn nav_cursor_never_lands_on_a_heading() {
        for scope in [Scope::Symlinks, Scope::Tools] {
            for row in build_display_rows(scope).iter().filter(|r| is_navigable(r)) {
                assert!(
                    !matches!(row, DisplayRow::SubSection(_)),
                    "a heading was marked navigable in {scope:?}",
                );
            }
        }
        assert!(
            build_display_rows(Scope::Tools).iter().any(is_navigable),
            "tools should have navigable items",
        );
    }
}
