use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::aliases::{
    add_user_alias, edit_user_alias, load_all_aliases, remove_user_alias, Alias, AliasSource,
};
use crate::config::personal::personal_dir;
use crate::config::settings::dots_dir;
use crate::tui::{draw_desc, draw_key_bar, draw_screen_nav, draw_top_bar, FlashKind};
use crate::tui::app::{App, Screen};
use crate::tui::theme::{style_muted, style_name, style_selected, style_text};

// ── modes ─────────────────────────────────────────────────────────────────────

pub enum AliasMode {
    List,
    AddForm  { name: String, value: String, field: usize },
    EditForm { original: String, name: String, value: String, field: usize },
    ConfirmDelete,
    Search   { query: String },
}

// ── view ──────────────────────────────────────────────────────────────────────

pub struct AliasView {
    pub aliases:  Vec<Alias>,
    pub visible:  Vec<usize>,
    pub cursor:   usize,
    pub scroll:   usize,
    pub flash:    Option<(String, FlashKind)>,
    pub mode:     AliasMode,
}

impl Default for AliasView {
    fn default() -> Self { Self::new() }
}

impl AliasView {
    /// True in any mode that consumes typed characters — the add/edit forms,
    /// the search box, and the delete confirmation.
    pub fn is_capturing(&self) -> bool {
        !matches!(self.mode, AliasMode::List)
    }

    pub fn new() -> Self {
        let mut v = Self {
            aliases: Vec::new(),
            visible: Vec::new(),
            cursor:  0,
            scroll:  0,
            flash:   None,
            mode:    AliasMode::List,
        };
        v.reload();
        v
    }

    pub fn reload(&mut self) {
        let dots    = dots_dir();
        let personal = personal_dir();
        self.aliases = load_all_aliases(&dots, &personal);
        self.rebuild_visible(None);
        self.clamp();
    }

    fn rebuild_visible(&mut self, filter: Option<&str>) {
        if let Some(q) = filter.filter(|q| !q.is_empty()) {
            let q = q.to_lowercase();
            self.visible = self.aliases.iter().enumerate()
                .filter(|(_, a)| {
                    a.name.to_lowercase().contains(&q) || a.value.to_lowercase().contains(&q)
                })
                .map(|(i, _)| i)
                .collect();
        } else {
            self.visible = (0..self.aliases.len()).collect();
        }
    }

    fn clamp(&mut self) {
        let n = self.visible.len();
        if n == 0 { self.cursor = 0; } else { self.cursor = self.cursor.min(n - 1); }
    }

    fn current(&self) -> Option<&Alias> {
        self.visible.get(self.cursor).and_then(|&i| self.aliases.get(i))
    }
}

// ── rendering ─────────────────────────────────────────────────────────────────

pub fn render_aliases(f: &mut Frame, area: Rect, _app: &App, view: &AliasView) {
    match &view.mode {
        AliasMode::AddForm { name, value, field } => {
            render_form(f, area, "add alias", name, value, *field, &view.flash);
        }
        AliasMode::EditForm { name, value, field, .. } => {
            render_form(f, area, "edit alias", name, value, *field, &view.flash);
        }
        _ => render_list(f, area, view),
    }
}

fn render_list(f: &mut Frame, area: Rect, view: &AliasView) {
    draw_screen_nav(f, area, Screen::Aliases);
    if area.height < 5 { return; }

    let visible_rows = (area.height as usize).saturating_sub(5);
    let start        = if view.cursor >= visible_rows { view.cursor - visible_rows + 1 } else { 0 };

    // Column header
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("  {:<18} {:<36} {}", "NAME", "COMMAND", "TYPE"),
            style_muted(),
        ))),
        Rect { x: area.x + 2, y: area.y + 1, width: area.width.saturating_sub(4), height: 1 },
    );

    for (row, &raw_idx) in view.visible.iter().enumerate().skip(start).take(visible_rows) {
        let y      = area.y + 2 + (row - start) as u16;
        if y + 4 >= area.y + area.height { break; }

        let alias  = &view.aliases[raw_idx];
        let is_sel = row == view.cursor;
        let is_user = matches!(alias.source, AliasSource::User);
        let cursor  = if is_sel { "▶ " } else { "  " };
        let badge   = if is_user { "user" } else { "" };

        // Name is the column the eye scans, the command is supporting detail,
        // and the source badge is right-aligned metadata.
        let name_style = if is_sel { style_selected() } else { style_name() };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(cursor, if is_sel { style_selected() } else { style_muted() }),
                Span::styled(format!("{:<18} ", truncate(&alias.name, 18)), name_style),
                Span::styled(format!("{:<36} ", truncate(&alias.value, 36)), style_muted()),
                Span::styled(badge.to_string(), style_muted()),
            ])),
            Rect { x: area.x + 2, y, width: area.width.saturating_sub(4), height: 1 },
        );
    }

    let desc = match &view.mode {
        AliasMode::ConfirmDelete => {
            let name = view.current().map(|a| a.name.as_str()).unwrap_or("?");
            format!("Delete alias '{name}'? y/n")
        }
        AliasMode::Search { query } => format!("/ {query}_"),
        _ => {
            if let Some(a) = view.current() {
                format!("alias {}='{}'", a.name, a.value)
            } else {
                String::new()
            }
        }
    };
    draw_desc(f, area, &desc, view.flash.as_ref());
    draw_key_bar(f, area, &[
        ("j/k", "navigate"), ("a", "add"), ("e", "edit"), ("d", "delete"),
        ("/", "search"), ("esc", "back"), ("q", "quit"),
    ]);
}

fn render_form(
    f:     &mut Frame,
    area:  Rect,
    title: &str,
    name:  &str,
    value: &str,
    field: usize,
    flash: &Option<(String, FlashKind)>,
) {
    draw_top_bar(f, area, title, "", &[("esc", "cancel")]);

    for (i, (label, text)) in [("Name", name), ("Value", value)].iter().enumerate() {
        let y       = area.y + 2 + i as u16 * 2;
        let is_sel  = i == field;
        let l_style = if is_sel { style_selected() } else { style_muted() };
        let v_style = if is_sel { style_name() } else { style_text() };
        let cursor  = if is_sel { "▶ " } else { "  " };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(cursor, l_style),
                Span::styled(format!("{:<10}", label), l_style),
                Span::styled(format!("{text}█"), v_style),
            ])),
            Rect { x: area.x + 2, y, width: area.width.saturating_sub(4), height: 1 },
        );
    }

    draw_desc(f, area, "tab next  enter save", flash.as_ref());
    draw_key_bar(f, area, &[
        ("tab", "next"), ("shift-tab", "prev"), ("enter", "save"), ("esc", "cancel"),
    ]);
}

// ── key handling ──────────────────────────────────────────────────────────────

pub fn handle_alias_key(app: &mut App, view: &mut AliasView, key: KeyEvent) {
    match &view.mode {
        AliasMode::List           => handle_list(app, view, key),
        AliasMode::AddForm { .. } |
        AliasMode::EditForm { .. }=> handle_form(view, key),
        AliasMode::ConfirmDelete  => handle_confirm(view, key),
        AliasMode::Search { .. }  => handle_search(view, key),
    }
}

fn handle_list(app: &mut App, view: &mut AliasView, key: KeyEvent) {
    let n = view.visible.len();
    match key.code {
        KeyCode::Esc => { app.screen = Screen::Main; app.flash = None; }
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => {
            if view.cursor + 1 < n { view.cursor += 1; }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            view.cursor = view.cursor.saturating_sub(1);
        }
        KeyCode::Char('a') => {
            view.mode  = AliasMode::AddForm { name: String::new(), value: String::new(), field: 0 };
            view.flash = None;
        }
        KeyCode::Char('e') => {
            if let Some(a) = view.current() {
                if !matches!(a.source, AliasSource::User) {
                    view.flash = Some(("✗ Cannot modify built-in aliases".to_string(), FlashKind::Error));
                    return;
                }
                let (n, v) = (a.name.clone(), a.value.clone());
                view.mode  = AliasMode::EditForm { original: n.clone(), name: n, value: v, field: 0 };
                view.flash = None;
            }
        }
        KeyCode::Char('d') => {
            if let Some(a) = view.current() {
                if !matches!(a.source, AliasSource::User) {
                    view.flash = Some(("✗ Cannot modify built-in aliases".to_string(), FlashKind::Error));
                    return;
                }
                view.mode  = AliasMode::ConfirmDelete;
                view.flash = None;
            }
        }
        KeyCode::Char('/') => {
            view.mode  = AliasMode::Search { query: String::new() };
            view.flash = None;
        }
        _ => {}
    }
}

fn handle_form(view: &mut AliasView, key: KeyEvent) {
    let shift = key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT);
    match key.code {
        KeyCode::Esc => { view.mode = AliasMode::List; view.flash = None; }
        KeyCode::Tab if !shift => {
            match &mut view.mode {
                AliasMode::AddForm { field, .. } | AliasMode::EditForm { field, .. } => {
                    *field = (*field + 1) % 2;
                }
                _ => {}
            }
        }
        KeyCode::BackTab => {
            match &mut view.mode {
                AliasMode::AddForm { field, .. } | AliasMode::EditForm { field, .. } => {
                    *field = (*field + 1) % 2;
                }
                _ => {}
            }
        }
        KeyCode::Backspace => {
            match &mut view.mode {
                AliasMode::AddForm { name, value, field } => {
                    if *field == 0 { name.pop(); } else { value.pop(); }
                }
                AliasMode::EditForm { name, value, field, .. } => {
                    if *field == 0 { name.pop(); } else { value.pop(); }
                }
                _ => {}
            }
        }
        KeyCode::Char(c) => {
            match &mut view.mode {
                AliasMode::AddForm { name, value, field } => {
                    if *field == 0 { name.push(c); } else { value.push(c); }
                }
                AliasMode::EditForm { name, value, field, .. } => {
                    if *field == 0 { name.push(c); } else { value.push(c); }
                }
                _ => {}
            }
        }
        KeyCode::Enter => {
            let mode = std::mem::replace(&mut view.mode, AliasMode::List);
            match mode {
                AliasMode::AddForm { name, value, .. } => {
                    let personal = personal_dir();
                    // Check collision with built-in
                    let builtin_collision = view.aliases.iter()
                        .any(|a| matches!(a.source, AliasSource::BuiltIn) && a.name == name);
                    if builtin_collision {
                        view.flash = Some((
                            format!("✗ '{}' is already a built-in alias", name),
                            FlashKind::Error,
                        ));
                        return;
                    }
                    match add_user_alias(&personal, &name, &value) {
                        Ok(()) => {
                            view.reload();
                            view.flash = Some((format!("✓ Added '{name}'"), FlashKind::Success));
                        }
                        Err(e) => {
                            view.flash = Some((format!("✗ {e}"), FlashKind::Error));
                        }
                    }
                }
                AliasMode::EditForm { original, value, .. } => {
                    let personal = personal_dir();
                    match edit_user_alias(&personal, &original, &value) {
                        Ok(()) => {
                            view.reload();
                            view.flash = Some((format!("✓ Updated '{original}'"), FlashKind::Success));
                        }
                        Err(e) => {
                            view.flash = Some((format!("✗ {e}"), FlashKind::Error));
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn handle_confirm(view: &mut AliasView, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') => {
            if let Some(a) = view.current().map(|a| a.name.clone()) {
                let personal = personal_dir();
                match remove_user_alias(&personal, &a) {
                    Ok(()) => {
                        view.reload();
                        view.flash = Some((format!("✓ Removed '{a}'"), FlashKind::Success));
                    }
                    Err(e) => view.flash = Some((format!("✗ {e}"), FlashKind::Error)),
                }
            }
            view.mode = AliasMode::List;
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            view.mode  = AliasMode::List;
            view.flash = None;
        }
        _ => {}
    }
}

fn handle_search(view: &mut AliasView, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            view.rebuild_visible(None);
            view.cursor = 0;
            view.mode   = AliasMode::List;
        }
        KeyCode::Enter => {
            view.mode = AliasMode::List;
        }
        KeyCode::Backspace => {
            if let AliasMode::Search { query } = &mut view.mode {
                query.pop();
                let q = query.clone();
                view.rebuild_visible(if q.is_empty() { None } else { Some(&q) });
                view.clamp();
            }
        }
        KeyCode::Char(c) => {
            if let AliasMode::Search { query } = &mut view.mode {
                query.push(c);
                let q = query.clone();
                view.rebuild_visible(Some(&q));
                view.clamp();
            }
        }
        _ => {}
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    format!("{}…", &s[..max.saturating_sub(1)])
}
