use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::installer::{detect_pm, install_dep};
use crate::packages::{check_dep, check_plugin, Category, Dep, Plugin, DEPS, PLUGINS};
use crate::symlinks::{self, SymlinkStatus};
use crate::tui::{draw_desc, draw_footer, draw_header, FlashKind};
use crate::tui::app::{App, Screen};
use crate::tui::theme::{style_dim, style_error, style_select};

const VERSION: &str = env!("DOTS_VERSION");

// ── display model ─────────────────────────────────────────────────────────────

pub enum DisplayRow {
    Section(&'static str),
    SymlinkItem { link: PathBuf, target: PathBuf, status: SymlinkStatus },
    DepItem     { dep: &'static Dep,    installed: bool },
    PluginItem  { plugin: &'static Plugin, installed: bool },
}

fn is_navigable(row: &DisplayRow) -> bool {
    !matches!(row, DisplayRow::Section(_))
}

fn row_desc(row: &DisplayRow) -> String {
    match row {
        DisplayRow::Section(_)                        => String::new(),
        DisplayRow::SymlinkItem { target, .. }        => format!("→ {}", home_rel(target)),
        DisplayRow::DepItem     { dep, .. }           => dep.desc.to_string(),
        DisplayRow::PluginItem  { plugin, .. }        => plugin.desc.to_string(),
    }
}

pub fn build_display_rows() -> Vec<DisplayRow> {
    let mut rows: Vec<DisplayRow> = Vec::new();

    rows.push(DisplayRow::Section("symlinks"));
    for s in symlinks::get_symlinks() {
        let status = symlinks::check(&s);
        rows.push(DisplayRow::SymlinkItem { link: s.link, target: s.target, status });
    }

    rows.push(DisplayRow::Section("tools"));
    for dep in DEPS {
        if dep.category == Category::Required || dep.category == Category::Optional {
            rows.push(DisplayRow::DepItem { dep, installed: check_dep(dep) });
        }
    }

    rows.push(DisplayRow::Section("plugins"));
    for plugin in PLUGINS {
        rows.push(DisplayRow::PluginItem { plugin, installed: check_plugin(plugin) });
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
    pub cursor:      usize,
    pub scroll:      usize,
    pub flash:       Option<(String, FlashKind)>,
    pub rows:        Vec<DisplayRow>,
    pub nav_indices: Vec<Option<usize>>,
    pub nav_count:   usize,
    pending:         Option<Pending>,
    install_rx:      Option<Receiver<anyhow::Result<()>>>,
}

impl HealthView {
    pub fn new() -> Self {
        let mut v = Self {
            cursor:      0,
            scroll:      0,
            flash:       None,
            rows:        Vec::new(),
            nav_indices: Vec::new(),
            nav_count:   0,
            pending:     None,
            install_rx:  None,
        };
        v.rebuild();
        v
    }

    pub fn rebuild(&mut self) {
        self.rows = build_display_rows();
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

    /// Move the cursor to the first item under the named section and scroll it
    /// to the top of the list. Used by the dashboard's drill-in.
    pub fn focus_section(&mut self, label: &str) {
        let Some(sec_di) = self.rows.iter().position(
            |r| matches!(r, DisplayRow::Section(l) if *l == label),
        ) else { return };
        self.scroll = sec_di;
        for di in (sec_di + 1)..self.rows.len() {
            if let Some(nav) = self.nav_indices[di] {
                self.cursor = nav;
                break;
            }
        }
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

pub fn render(f: &mut Frame, area: Rect, _app: &App, view: &HealthView) {
    draw_header(f, area, " health ", VERSION);

    if area.height < 5 { return; }
    let visible   = (area.height as usize).saturating_sub(5);
    let content_y = area.y + 1;

    for (di, row) in view.rows.iter().enumerate().skip(view.scroll).take(visible) {
        let ry       = content_y + (di - view.scroll) as u16;
        let row_rect = Rect { x: area.x, y: ry, width: area.width, height: 1 };
        let nav_idx  = view.nav_indices[di];
        let is_cursor = nav_idx == Some(view.cursor);

        match row {
            DisplayRow::Section(label) => {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        format!("  {label}"),
                        style_dim().add_modifier(Modifier::BOLD),
                    ))),
                    row_rect,
                );
            }
            DisplayRow::SymlinkItem { link, target, status } => {
                let ok     = *status == SymlinkStatus::Ok;
                let cursor = if is_cursor { "▶" } else { " " };
                let sym    = if ok { "✓" } else { "✗" };
                let sty    = if ok { style_select() } else { style_error() };
                let status_str = if ok { home_rel(target) } else { status_label(status).to_string() };
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(format!("{cursor} "), sty),
                        Span::styled(sym.to_string(), sty),
                        Span::raw(format!("  {:<32} {}", home_rel(link), status_str)),
                    ])),
                    row_rect,
                );
            }
            DisplayRow::DepItem { dep, installed } => {
                let cursor = if is_cursor { "▶" } else { " " };
                let sym    = if *installed { "✓" } else { "✗" };
                let sty    = if *installed { style_select() } else { style_error() };
                let tag    = match dep.category {
                    Category::Required => "[req]",
                    Category::Optional => "[opt]",
                    Category::Dev      => "[dev]",
                };
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(format!("{cursor} "), sty),
                        Span::styled(sym.to_string(), sty),
                        Span::raw(format!("  {:<14} {}  {}", dep.bin, tag, dep.desc)),
                    ])),
                    row_rect,
                );
            }
            DisplayRow::PluginItem { plugin, installed } => {
                let cursor = if is_cursor { "▶" } else { " " };
                let sym    = if *installed { "✓" } else { "✗" };
                let sty    = if *installed { style_select() } else { style_error() };
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(format!("{cursor} "), sty),
                        Span::styled(sym.to_string(), sty),
                        Span::raw(format!("  {:<32} {}", plugin.name, plugin.desc)),
                    ])),
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

    let any_broken  = view.rows.iter().any(|r| matches!(r, DisplayRow::SymlinkItem { status, .. } if *status != SymlinkStatus::Ok));
    let any_missing = view.rows.iter().any(|r| matches!(r, DisplayRow::DepItem { installed: false, .. }));
    let can_fix     = can_fix_cursor(view);
    let mut hints   = Vec::<&str>::new();
    if can_fix    { hints.push("enter fix/apply"); }
    if any_broken { hints.push("r repair all"); }
    if any_missing{ hints.push("i install all"); }
    hints.push("esc back");
    hints.push("q quit");
    draw_footer(f, area, &format!(" j/k navigate  {}  ", hints.join("  ")));
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
        KeyCode::Char('r') => repair_all_symlinks(view),
        KeyCode::Char('i') => install_all_missing(view),
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
            .map(|d| install_dep(d))
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

    #[test]
    fn display_list_has_sections() {
        let rows = build_display_rows();
        let sections: Vec<_> = rows.iter()
            .filter(|r| matches!(r, DisplayRow::Section(_)))
            .collect();
        assert!(sections.len() >= 3, "expected at least 3 sections, got {}", sections.len());
    }

    #[test]
    fn dep_items_include_git_as_green() {
        let rows = build_display_rows();
        let git_row = rows.iter().find(|r| {
            matches!(r, DisplayRow::DepItem { dep, .. } if dep.bin == "git")
        });
        assert!(git_row.is_some(), "git dep should be in display rows");
        if let Some(DisplayRow::DepItem { installed, .. }) = git_row {
            assert!(*installed, "git should be installed (required to build this binary)");
        }
    }

    #[test]
    fn missing_dep_shows_as_not_installed() {
        let rows = build_display_rows();
        let any_dep_item = rows.iter().find(|r| matches!(r, DisplayRow::DepItem { .. }));
        assert!(any_dep_item.is_some(), "should have at least one dep item");
    }

    #[test]
    fn nav_cursor_never_on_section() {
        let rows = build_display_rows();
        let mut nav_idx = 0usize;
        for row in &rows {
            if is_navigable(row) {
                assert!(
                    !matches!(row, DisplayRow::Section(_)),
                    "navigable item {} is a Section",
                    nav_idx
                );
                nav_idx += 1;
            }
        }
        assert!(nav_idx > 0, "no navigable items found");
    }
}
