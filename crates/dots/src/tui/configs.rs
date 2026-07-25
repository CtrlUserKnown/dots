//! The Configs screen: a master–detail view over the app configs discovered in
//! the dotfiles repo (see [`crate::configs`]). The left pane lists each config
//! with an install-status badge; the right pane shows the selected config's
//! files and their individual link status. `v` opens a file in a scrollable
//! pager. `space` installs or removes a config by (un)linking it into `$HOME`.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::configs::{self, Config, ConfigStatus};
use crate::symlinks::{self, SymlinkStatus};
use crate::tui::app::{App, Screen};
use crate::tui::theme::{style_dim, style_error, style_header, style_select, style_warn};
use crate::tui::{draw_desc, draw_footer, draw_header, FlashKind};

// ── view state ────────────────────────────────────────────────────────────────

#[derive(PartialEq, Eq, Clone, Copy)]
enum Focus { List, Files }

/// A scrollable read-only view of one file's contents.
struct Pager {
    title:  String,
    lines:  Vec<String>,
    scroll: usize,
}

pub struct ConfigsView {
    configs:     Vec<Config>,
    root:        Option<PathBuf>,
    cursor:      usize,
    scroll:      usize,
    focus:       Focus,
    file_cursor: usize,
    file_scroll: usize,
    pager:       Option<Pager>,
    /// When `Some`, the screen is in add-path input mode; holds the typed buffer.
    adding:      Option<String>,
    pub flash:   Option<(String, FlashKind)>,
}

impl Default for ConfigsView {
    fn default() -> Self { Self::new() }
}

impl ConfigsView {
    pub fn new() -> Self {
        Self {
            configs:     Vec::new(),
            root:        None,
            cursor:      0,
            scroll:      0,
            focus:       Focus::List,
            file_cursor: 0,
            file_scroll: 0,
            pager:       None,
            adding:      None,
            flash:       None,
        }
    }

    /// (Re)discover configs from disk. Called on entry and after mutations.
    pub fn reload(&mut self) {
        // Preserve the highlighted config across a reload where possible.
        let selected = self.configs.get(self.cursor).map(|c| c.name.clone());
        self.configs = configs::discover();
        self.root    = configs::configs_root();
        self.cursor  = selected
            .and_then(|name| self.configs.iter().position(|c| c.name == name))
            .unwrap_or(self.cursor)
            .min(self.configs.len().saturating_sub(1));
        self.file_cursor = 0;
        self.file_scroll = 0;
        self.focus = Focus::List;
        self.pager = None;
    }

    fn selected(&self) -> Option<&Config> {
        self.configs.get(self.cursor)
    }

    /// Number of file rows for the selected config.
    fn file_count(&self) -> usize {
        self.selected().map(|c| c.links.len()).unwrap_or(0)
    }

    fn toggle_install(&mut self) {
        let Some(cfg) = self.configs.get(self.cursor) else { return };
        let name = cfg.name.clone();
        let result = match cfg.status {
            ConfigStatus::Empty => {
                self.flash = Some((format!("{name} has nothing to link"), FlashKind::Info));
                return;
            }
            ConfigStatus::Installed => {
                configs::remove(cfg).map(|n| format!("✓ {name} removed ({n} link(s) unlinked)"))
            }
            ConfigStatus::Partial | ConfigStatus::NotInstalled => {
                configs::install(cfg).map(|r| {
                    format!("✓ {name} installed ({} linked, {} skipped)", r.repaired + r.ok, r.skipped)
                })
            }
        };
        match result {
            Ok(msg) => { self.reload(); self.flash = Some((msg, FlashKind::Success)); }
            Err(e)  => self.flash = Some((format!("✗ {e}"), FlashKind::Error)),
        }
    }

    /// Pull the latest source repo and re-apply installed configs (Phase 5 sync).
    fn sync_now(&mut self) {
        match configs::sync() {
            Ok(msg) => { self.reload(); self.flash = Some((format!("✓ {msg}"), FlashKind::Success)); }
            Err(e)  => self.flash = Some((format!("✗ {e}"), FlashKind::Error)),
        }
    }

    /// Adopt the file at the typed path into the source repo (Phase 3 `add`).
    fn commit_add(&mut self, path: &str) {
        let path = path.trim();
        if path.is_empty() {
            self.flash = Some(("add cancelled".into(), FlashKind::Info));
            return;
        }
        match configs::add(Path::new(path), None) {
            Ok(msg) => { self.reload(); self.flash = Some((first_line(&msg), FlashKind::Success)); }
            Err(e)  => self.flash = Some((format!("✗ {e}"), FlashKind::Error)),
        }
    }

    /// Open the file under the file cursor in the pager (Files focus only).
    fn open_pager(&mut self) {
        let Some(cfg) = self.selected() else { return };
        let Some(link) = cfg.links.get(self.file_cursor) else { return };
        let target = &link.target;

        if target.is_dir() {
            self.flash = Some((
                format!("{} is a folded directory", rel_to(&cfg.dir, target)),
                FlashKind::Info,
            ));
            return;
        }
        match std::fs::read_to_string(target) {
            Ok(text) => {
                self.pager = Some(Pager {
                    title:  rel_to(&cfg.dir, target),
                    lines:  text.lines().map(str::to_string).collect(),
                    scroll: 0,
                });
            }
            Err(_) => self.flash = Some(("cannot preview (binary or unreadable)".into(), FlashKind::Info)),
        }
    }

    fn move_list(&mut self, delta: isize, visible: usize) {
        let n = self.configs.len();
        if n == 0 { return; }
        self.cursor = clamp_add(self.cursor, delta, n);
        self.file_cursor = 0;
        self.file_scroll = 0;
        scroll_into_view(&mut self.scroll, self.cursor, visible);
    }

    fn move_files(&mut self, delta: isize, visible: usize) {
        let n = self.file_count();
        if n == 0 { return; }
        self.file_cursor = clamp_add(self.file_cursor, delta, n);
        scroll_into_view(&mut self.file_scroll, self.file_cursor, visible);
    }
}

// ── summary ───────────────────────────────────────────────────────────────────

struct Counts { installed: usize, partial: usize, missing: usize, total: usize }

fn counts(configs: &[Config]) -> Counts {
    let mut c = Counts { installed: 0, partial: 0, missing: 0, total: configs.len() };
    for cfg in configs {
        match cfg.status {
            ConfigStatus::Installed => c.installed += 1,
            ConfigStatus::Partial   => c.partial += 1,
            _                       => c.missing += 1,
        }
    }
    c
}

fn status_style(s: ConfigStatus) -> Style {
    match s {
        ConfigStatus::Installed    => style_select(),
        ConfigStatus::Partial      => style_warn(),
        ConfigStatus::NotInstalled => style_dim(),
        ConfigStatus::Empty        => style_dim(),
    }
}

// ── rendering ─────────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, area: Rect, _app: &App, view: &ConfigsView) {
    draw_header(f, area, " configs ", "");
    if area.height < 6 { return; }

    // Summary line.
    let c = counts(&view.configs);
    let summary = Line::from(vec![
        Span::styled(format!("{} configs", c.total), style_header()),
        Span::raw("  ·  "),
        Span::styled(format!("{} installed", c.installed), style_select()),
        Span::raw("  ·  "),
        Span::styled(format!("{} partial", c.partial), style_warn()),
        Span::raw("  ·  "),
        Span::styled(format!("{} missing", c.missing), style_dim()),
    ]);
    f.render_widget(
        Paragraph::new(summary),
        Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: 1 },
    );

    let body = Rect {
        x:      area.x + 1,
        y:      area.y + 3,
        width:  area.width.saturating_sub(2),
        height: area.height.saturating_sub(3 + 3), // leave desc+footer room
    };

    if view.configs.is_empty() {
        render_empty(f, body, view);
    } else {
        let left_w  = (body.width as usize * 3 / 5).max(22) as u16;
        let left_w  = left_w.min(body.width.saturating_sub(12));
        let left    = Rect { x: body.x, y: body.y, width: left_w, height: body.height };
        let right   = Rect {
            x: body.x + left_w + 1,
            y: body.y,
            width: body.width.saturating_sub(left_w + 1),
            height: body.height,
        };
        render_list(f, left, view);
        render_detail(f, right, view);
    }

    // Desc bar: the add-path prompt when adding, else where we read configs from.
    if let Some(buf) = &view.adding {
        draw_desc(f, area, &format!("add path › {buf}▏"), view.flash.as_ref());
    } else {
        let where_from = view.root.as_ref()
            .map(|r| format!("configs from {}", home_rel(r)))
            .unwrap_or_else(|| "no dotfiles configs dir found".into());
        draw_desc(f, area, &where_from, view.flash.as_ref());
    }

    let footer = match view.focus {
        _ if view.adding.is_some() => " type a path  enter add  esc cancel ",
        _ if view.pager.is_some()  => " j/k scroll  esc close ",
        Focus::List  => " j/k move  space install/remove  a add  s sync  l/→ files  r rescan  q quit ",
        Focus::Files => " j/k move  v/enter view file  h/← back  esc back  q quit ",
    };
    draw_footer(f, area, footer);

    // Pager overlay sits on top of everything.
    if let Some(pager) = &view.pager {
        render_pager(f, area, pager);
    }
}

fn render_empty(f: &mut Frame, area: Rect, view: &ConfigsView) {
    let msg = match &view.root {
        Some(root) => vec![
            Line::from(Span::styled(format!("No config directories in {}", home_rel(root)), style_dim())),
            Line::from(Span::styled("Add a directory per app (e.g. nvim/, git/) to populate this list.", style_dim())),
        ],
        None => vec![
            Line::from(Span::styled("No dotfiles configs directory found.", style_dim())),
            Line::from(Span::styled("Pull a repo with 'dots gc -u=<user/repo>', or press 'a' to add a config.", style_dim())),
        ],
    };
    f.render_widget(Paragraph::new(msg), Rect { x: area.x + 1, y: area.y, width: area.width.saturating_sub(1), height: 2 });
}

fn render_list(f: &mut Frame, area: Rect, view: &ConfigsView) {
    let active = view.focus == Focus::List && view.pager.is_none();
    let visible = area.height as usize;
    let width   = area.width as usize;

    for (row, i) in (view.scroll..view.configs.len()).take(visible).enumerate() {
        let cfg     = &view.configs[i];
        let sel     = i == view.cursor;
        let badge   = cfg.status.badge();
        let bstyle  = status_style(cfg.status);
        let cursor  = if sel && active { "▶ " } else if sel { "· " } else { "  " };

        // name left, badge right-aligned within the column.
        let name_field = width.saturating_sub(2 + badge.chars().count() + 1);
        let name = truncate(&cfg.name, name_field);
        let pad  = name_field.saturating_sub(name.chars().count());

        let name_style = if sel { Style::default().add_modifier(Modifier::BOLD) } else { Style::default() };
        let line = Line::from(vec![
            Span::styled(cursor, if sel { style_select() } else { style_dim() }),
            Span::styled(name, name_style),
            Span::raw(" ".repeat(pad + 1)),
            Span::styled(badge, bstyle),
        ]);
        f.render_widget(
            Paragraph::new(line),
            Rect { x: area.x, y: area.y + row as u16, width: area.width, height: 1 },
        );
    }
}

fn render_detail(f: &mut Frame, area: Rect, view: &ConfigsView) {
    let active = view.focus == Focus::Files && view.pager.is_none();
    let title_style = if active { style_select() } else { style_header() };
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(style_dim())
        .title(Span::styled(" files ", title_style.add_modifier(Modifier::BOLD)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(cfg) = view.selected() else { return };
    if cfg.links.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled("(no linkable files)", style_dim()))),
            Rect { x: inner.x + 1, y: inner.y, width: inner.width.saturating_sub(1), height: 1 },
        );
        return;
    }

    let visible = inner.height as usize;
    for (row, i) in (view.file_scroll..cfg.links.len()).take(visible).enumerate() {
        let link   = &cfg.links[i];
        let st     = symlinks::check(link);
        let sel    = active && i == view.file_cursor;
        let (mark, mstyle) = match st {
            SymlinkStatus::Ok => ("✓", style_select()),
            SymlinkStatus::Missing => ("·", style_dim()),
            _ => ("✗", style_error()),
        };
        let cursor = if sel { "▶ " } else { "  " };
        let rel    = rel_to(&cfg.dir, &link.target);
        let name   = truncate(&rel, inner.width.saturating_sub(5) as usize);
        let line = Line::from(vec![
            Span::styled(cursor, style_select()),
            Span::styled(format!("{mark} "), mstyle),
            Span::styled(name, if sel { Style::default().add_modifier(Modifier::BOLD) } else { Style::default() }),
        ]);
        f.render_widget(
            Paragraph::new(line),
            Rect { x: inner.x + 1, y: inner.y + row as u16, width: inner.width.saturating_sub(1), height: 1 },
        );
    }
}

fn render_pager(f: &mut Frame, area: Rect, pager: &Pager) {
    // A centred box covering most of the screen.
    let w = area.width.saturating_sub(6).min(100);
    let h = area.height.saturating_sub(4);
    let box_rect = Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + 2,
        width: w,
        height: h,
    };
    let title = format!(" {} ", truncate(&pager.title, w.saturating_sub(4) as usize));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_header())
        .title(Span::styled(title, style_header().add_modifier(Modifier::BOLD)));
    let inner = block.inner(box_rect);
    // Clear behind the box so underlying content doesn't bleed through.
    f.render_widget(ratatui::widgets::Clear, box_rect);
    f.render_widget(block, box_rect);

    let visible = inner.height as usize;
    for (row, i) in (pager.scroll..pager.lines.len()).take(visible).enumerate() {
        let text = truncate(&pager.lines[i], inner.width as usize);
        f.render_widget(
            Paragraph::new(Line::from(Span::raw(text))),
            Rect { x: inner.x, y: inner.y + row as u16, width: inner.width, height: 1 },
        );
    }
    // Scroll indicator.
    if pager.lines.len() > visible {
        let pos   = format!(" {}/{} ", (pager.scroll + visible).min(pager.lines.len()), pager.lines.len());
        let plen  = pos.len() as u16;
        let px    = box_rect.x + box_rect.width.saturating_sub(plen + 2);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(pos, style_dim()))),
            Rect { x: px, y: box_rect.y + box_rect.height - 1, width: plen, height: 1 },
        );
    }
}

// ── key handling ──────────────────────────────────────────────────────────────

pub fn handle_key(app: &mut App, view: &mut ConfigsView, key: KeyEvent) {
    // Add-path input mode captures all input while open.
    if view.adding.is_some() {
        match key.code {
            KeyCode::Esc => view.adding = None,
            KeyCode::Backspace => { if let Some(b) = &mut view.adding { b.pop(); } }
            KeyCode::Char(c)   => { if let Some(b) = &mut view.adding { b.push(c); } }
            KeyCode::Enter => {
                let path = view.adding.take().unwrap_or_default();
                view.commit_add(&path);
            }
            _ => {}
        }
        return;
    }

    // Pager captures all input while open.
    if let Some(pager) = &mut view.pager {
        let page = 10;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => view.pager = None,
            KeyCode::Char('j') | KeyCode::Down => {
                let max = pager.lines.len().saturating_sub(1);
                pager.scroll = (pager.scroll + 1).min(max);
            }
            KeyCode::Char('k') | KeyCode::Up => pager.scroll = pager.scroll.saturating_sub(1),
            KeyCode::Char('d') | KeyCode::PageDown => {
                let max = pager.lines.len().saturating_sub(1);
                pager.scroll = (pager.scroll + page).min(max);
            }
            KeyCode::Char('u') | KeyCode::PageUp => pager.scroll = pager.scroll.saturating_sub(page),
            _ => {}
        }
        return;
    }

    // A generous visible-window estimate; exact height isn't known here, but the
    // list is short and render clamps the scroll anyway.
    let visible = 100;
    match view.focus {
        Focus::List => match key.code {
            KeyCode::Esc => { app.screen = Screen::Main; app.flash = None; }
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => view.move_list(1, visible),
            KeyCode::Char('k') | KeyCode::Up   => view.move_list(-1, visible),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                if view.file_count() > 0 { view.focus = Focus::Files; }
            }
            KeyCode::Char(' ') => view.toggle_install(),
            KeyCode::Char('a') => { view.adding = Some(String::new()); view.flash = None; }
            KeyCode::Char('s') => view.sync_now(),
            KeyCode::Char('r') => { view.reload(); view.flash = Some(("rescanned".into(), FlashKind::Info)); }
            _ => {}
        },
        Focus::Files => match key.code {
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => view.focus = Focus::List,
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => view.move_files(1, visible),
            KeyCode::Char('k') | KeyCode::Up   => view.move_files(-1, visible),
            KeyCode::Char('v') | KeyCode::Enter => view.open_pager(),
            _ => {}
        },
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn clamp_add(cur: usize, delta: isize, len: usize) -> usize {
    let next = cur as isize + delta;
    next.clamp(0, len as isize - 1) as usize
}

fn scroll_into_view(scroll: &mut usize, cursor: usize, visible: usize) {
    if cursor < *scroll {
        *scroll = cursor;
    } else if visible > 0 && cursor >= *scroll + visible {
        *scroll = cursor + 1 - visible;
    }
}

fn truncate(s: &str, max: usize) -> String {
    if max == 0 { return String::new(); }
    if s.chars().count() <= max {
        s.to_string()
    } else if max <= 1 {
        "…".to_string()
    } else {
        let keep: String = s.chars().take(max - 1).collect();
        format!("{keep}…")
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim_start_matches('✓').trim().to_string()
}

fn rel_to(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn home_rel(path: &Path) -> String {
    let home = dirs::home_dir().unwrap_or_default();
    match path.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_adds_ellipsis() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello", 3), "he…");
        assert_eq!(truncate("hello", 0), "");
    }

    #[test]
    fn clamp_add_stays_in_range() {
        assert_eq!(clamp_add(0, -1, 3), 0);
        assert_eq!(clamp_add(2, 1, 3), 2);
        assert_eq!(clamp_add(1, 1, 3), 2);
    }

    #[test]
    fn reload_does_not_panic_without_configs() {
        let mut v = ConfigsView::new();
        v.reload();
        assert!(v.selected().is_none() || v.cursor < v.configs.len());
    }
}
