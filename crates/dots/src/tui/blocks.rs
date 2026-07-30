//! The "Blocks" screen: lets the user swap, add, remove, and reorder the
//! widgets inside each dashboard zone, without hand-editing
//! `~/.dots/layout.toml`.
//!
//! Reached from the [`super::settings`] popup, this mirrors `ThemeView`'s
//! shape (a scrollable list, `enter` applies immediately, `esc` goes back)
//! but has two levels: a flattened zone/widget list, and a small overlay
//! picker for choosing a replacement or new widget from the catalog of
//! built-ins and live plugin panes.
//!
//! Every mutation touches only `Zone::widgets` — never a zone's geometry —
//! and is written to disk immediately via [`crate::zones::Layout::save`],
//! the same apply-on-`enter` model `ThemeView` uses for the terminal theme.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use crate::tui::app::{App, Screen};
use crate::tui::overview::Pane;
use crate::tui::{draw_desc, draw_key_bar, draw_top_bar, FlashKind};
use crate::tui::theme::{style_key, style_muted, style_name, style_selected, style_text};
use crate::zones::{Layout, BUILTIN_WIDGETS};

// ── rows ─────────────────────────────────────────────────────────────────────

/// One line in the flattened zone/widget list. A header is a cursor stop
/// too, not just a label — that's what lets `enter`/`a` append to a zone
/// that has no widgets yet.
enum Row {
    ZoneHeader { zone_idx: usize },
    Slot { zone_idx: usize, slot_idx: usize, label: String },
}

/// Rebuilt fresh on every render and every keypress from `app.layout` +
/// `app.plugin_panes` — never cached on [`BlocksView`], so a background
/// plugin tick updating `app.plugin_panes` mid-edit can't leave it stale.
fn rows(app: &App) -> Vec<Row> {
    let mut out = Vec::new();
    for (zone_idx, zone) in app.layout.zones.iter().enumerate() {
        out.push(Row::ZoneHeader { zone_idx });
        for (slot_idx, id) in zone.widgets.iter().enumerate() {
            out.push(Row::Slot { zone_idx, slot_idx, label: row_label(app, id) });
        }
    }
    out
}

/// A widget id's display label: a built-in's title, a live plugin pane's
/// title, or (for an id `zones::resolve` would warn about) the raw id, so a
/// dangling reference is still visible rather than silently blank.
fn row_label(app: &App, id: &str) -> String {
    if let Some(pane) = Pane::from_id(id) {
        return pane.title().trim().to_string();
    }
    if let Some(p) = app.plugin_panes.iter().find(|p| p.id == id) {
        return p.title.clone();
    }
    id.to_string()
}

fn zone_label(zone: &crate::zones::Zone) -> String {
    zone.title.clone().unwrap_or_else(|| zone.id.clone())
}

// ── catalog (what the picker offers) ────────────────────────────────────────

struct CatalogEntry {
    id:    String,
    label: String,
}

/// Built-ins first, in [`BUILTIN_WIDGETS`] order, then every currently live
/// plugin pane. No "already placed" filtering: `zones::resolve` already
/// tolerates the same id in two zones (drawn twice, no warning), so hiding
/// used entries would just make it harder to put a widget in a second zone.
fn catalog(app: &App) -> Vec<CatalogEntry> {
    let mut out: Vec<CatalogEntry> = BUILTIN_WIDGETS
        .iter()
        .filter_map(|id| Pane::from_id(id).map(|p| CatalogEntry {
            id:    (*id).to_string(),
            label: p.title().trim().to_string(),
        }))
        .collect();
    out.extend(app.plugin_panes.iter().map(|p| CatalogEntry {
        id:    p.id.clone(),
        label: p.title.clone(),
    }));
    out
}

// ── picker state ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum PickerTarget {
    Replace { zone_idx: usize, slot_idx: usize },
    Append { zone_idx: usize },
}

struct PickerState {
    target:  PickerTarget,
    entries: Vec<CatalogEntry>,
    cursor:  usize,
    scroll:  usize,
}

// ── view ─────────────────────────────────────────────────────────────────────

pub struct BlocksView {
    cursor: usize,
    scroll: usize,
    picker: Option<PickerState>,
    flash:  Option<(String, FlashKind)>,
}

impl Default for BlocksView {
    fn default() -> Self { Self::new() }
}

impl BlocksView {
    pub fn new() -> Self {
        Self { cursor: 0, scroll: 0, picker: None, flash: None }
    }

    /// Reset cursor/scroll/picker/flash. Called each time the screen is
    /// entered, mirroring `ThemeView::load`.
    pub fn load(&mut self) {
        self.cursor = 0;
        self.scroll = 0;
        self.picker = None;
        self.flash  = None;
    }

    pub fn is_picking(&self) -> bool {
        self.picker.is_some()
    }
}

fn update_scroll(cursor: usize, scroll: &mut usize) {
    if cursor < *scroll {
        *scroll = cursor;
    } else if *scroll + 20 <= cursor {
        *scroll = cursor.saturating_sub(19);
    }
}

// ── pure mutation helpers (no I/O — safe to unit test directly) ────────────

fn apply_pick_to(layout: &mut Layout, target: PickerTarget, id: String) {
    match target {
        PickerTarget::Replace { zone_idx, slot_idx } => {
            if let Some(zone) = layout.zones.get_mut(zone_idx) {
                if let Some(slot) = zone.widgets.get_mut(slot_idx) {
                    *slot = id;
                }
            }
        }
        PickerTarget::Append { zone_idx } => {
            if let Some(zone) = layout.zones.get_mut(zone_idx) {
                zone.widgets.push(id);
            }
        }
    }
}

fn remove_slot_from(layout: &mut Layout, zone_idx: usize, slot_idx: usize) {
    if let Some(zone) = layout.zones.get_mut(zone_idx) {
        if slot_idx < zone.widgets.len() {
            zone.widgets.remove(slot_idx);
        }
    }
}

/// Swaps the widget at `slot_idx` with its neighbor `delta` away, within the
/// same zone only. Returns whether a swap happened, so the caller can skip
/// persisting (and moving the cursor) on a no-op move at either end.
fn reorder_in(layout: &mut Layout, zone_idx: usize, slot_idx: usize, delta: isize) -> bool {
    let Some(zone) = layout.zones.get_mut(zone_idx) else { return false };
    let len = zone.widgets.len();
    let new_idx = slot_idx as isize + delta;
    if new_idx < 0 || new_idx as usize >= len {
        return false;
    }
    zone.widgets.swap(slot_idx, new_idx as usize);
    true
}

// ── app-level mutation (mutate + persist + rebuild) ─────────────────────────

/// Write `app.layout` to disk and re-resolve the dashboard so the change is
/// visible immediately — the same apply-on-`enter` model `ThemeView` uses.
fn persist(app: &mut App, view: &mut BlocksView) {
    match app.layout.save() {
        Ok(()) => {
            app.rebuild_dashboard();
            view.flash = None;
        }
        Err(e) => {
            view.flash = Some((format!("✗ Save failed: {e}"), FlashKind::Error));
        }
    }
}

fn open_picker(app: &App, view: &mut BlocksView, target: PickerTarget) {
    view.picker = Some(PickerState { target, entries: catalog(app), cursor: 0, scroll: 0 });
}

fn apply_pick(app: &mut App, view: &mut BlocksView) {
    let Some(picker) = view.picker.take() else { return };
    let Some(entry) = picker.entries.get(picker.cursor) else { return };
    let id = entry.id.clone();
    apply_pick_to(&mut app.layout, picker.target, id);
    persist(app, view);
}

fn remove_slot(app: &mut App, view: &mut BlocksView, zone_idx: usize, slot_idx: usize) {
    remove_slot_from(&mut app.layout, zone_idx, slot_idx);
    persist(app, view);
    let n = rows(app).len();
    view.cursor = view.cursor.min(n.saturating_sub(1));
}

fn reorder(app: &mut App, view: &mut BlocksView, zone_idx: usize, slot_idx: usize, delta: isize) {
    if !reorder_in(&mut app.layout, zone_idx, slot_idx, delta) {
        return;
    }
    view.cursor = (view.cursor as isize + delta).max(0) as usize;
    persist(app, view);
}

// ── rendering ────────────────────────────────────────────────────────────────

pub fn render_blocks(f: &mut Frame, area: Rect, app: &App, view: &BlocksView) {
    draw_top_bar(f, area, "dots", "dashboard blocks", &[("esc", "back"), ("q", "quit")]);

    if area.height < 5 { return; }
    let list_rows = rows(app);
    let visible   = (area.height as usize).saturating_sub(5);

    for (i, row) in list_rows.iter().enumerate().skip(view.scroll).take(visible) {
        let y      = area.y + 2 + (i - view.scroll) as u16;
        let is_sel = i == view.cursor;
        let rect   = Rect { x: area.x, y, width: area.width, height: 1 };
        let cursor_style = if is_sel { style_selected() } else { style_muted() };

        match row {
            Row::ZoneHeader { zone_idx } => {
                let label = zone_label(&app.layout.zones[*zone_idx]);
                let style = if is_sel { style_selected() } else { style_name() };
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(if is_sel { "▶ " } else { "  " }, cursor_style),
                        Span::styled(label, style),
                    ])),
                    rect,
                );
            }
            Row::Slot { label, .. } => {
                let style = if is_sel { style_selected() } else { style_text() };
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(if is_sel { "   ▶ " } else { "     " }, cursor_style),
                        Span::styled(label.clone(), style),
                    ])),
                    rect,
                );
            }
        }
    }

    let desc = match list_rows.get(view.cursor) {
        Some(Row::ZoneHeader { .. }) => "enter/a: add a widget to this zone".to_string(),
        Some(Row::Slot { .. })       => "enter: swap   d: remove   J/K: reorder".to_string(),
        None => String::new(),
    };
    draw_desc(f, area, &desc, view.flash.as_ref());
    draw_key_bar(f, area, &[
        ("j/k", "navigate"), ("enter", "swap/add"), ("a", "add"),
        ("d", "remove"), ("J/K", "reorder"), ("esc", "back"), ("q", "quit"),
    ]);

    if let Some(picker) = &view.picker {
        render_picker(f, area, picker);
    }
}

const PICKER_HINT: &str = "enter choose   esc cancel";

fn render_picker(f: &mut Frame, area: Rect, picker: &PickerState) {
    let title = match picker.target {
        PickerTarget::Replace { .. } => "swap widget",
        PickerTarget::Append { .. }  => "add widget",
    };

    let visible = 10.min(picker.entries.len().max(1));
    let content_w = picker.entries.iter()
        .map(|e| e.label.chars().count() + 4)
        .chain([title.len(), PICKER_HINT.len()])
        .max()
        .unwrap_or(20);
    let box_w = (content_w as u16 + 4).min(area.width.saturating_sub(2));
    let box_h = (visible as u16 + 3).min(area.height.saturating_sub(2));

    let x = area.x + area.width.saturating_sub(box_w) / 2;
    let y = area.y + area.height.saturating_sub(box_h) / 2;
    let rect = Rect { x, y, width: box_w, height: box_h };

    f.render_widget(Clear, rect);
    f.render_widget(tui_core::block(title, true), rect);

    let inner_x = rect.x + 2;
    let inner_w = rect.width.saturating_sub(3);

    if picker.entries.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled("no widgets available", style_muted()))),
            Rect { x: inner_x, y: rect.y + 1, width: inner_w, height: 1 },
        );
    }

    for (i, entry) in picker.entries.iter().enumerate().skip(picker.scroll).take(visible) {
        let ry     = rect.y + 1 + (i - picker.scroll) as u16;
        let is_sel = i == picker.cursor;
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(if is_sel { "▶ " } else { "  " }, if is_sel { style_selected() } else { style_muted() }),
                Span::styled(entry.label.clone(), if is_sel { style_selected() } else { style_text() }),
            ])),
            Rect { x: inner_x, y: ry, width: inner_w, height: 1 },
        );
    }

    let hint_y = rect.y + 1 + visible as u16;
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(PICKER_HINT, style_key()))),
        Rect { x: inner_x, y: hint_y, width: inner_w, height: 1 },
    );
}

// ── key handling ─────────────────────────────────────────────────────────────

pub fn handle_blocks_key(app: &mut App, view: &mut BlocksView, key: KeyEvent) {
    if view.picker.is_some() {
        handle_picker_key(app, view, key);
        return;
    }

    let list_rows = rows(app);
    let n = list_rows.len();

    match key.code {
        KeyCode::Esc => {
            app.screen = Screen::Settings;
            app.flash  = None;
        }
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => {
            if n > 0 {
                view.cursor = (view.cursor + 1).min(n - 1);
                update_scroll(view.cursor, &mut view.scroll);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            view.cursor = view.cursor.saturating_sub(1);
            update_scroll(view.cursor, &mut view.scroll);
        }
        KeyCode::Enter => {
            if let Some(row) = list_rows.get(view.cursor) {
                let target = match *row {
                    Row::ZoneHeader { zone_idx } => PickerTarget::Append { zone_idx },
                    Row::Slot { zone_idx, slot_idx, .. } => PickerTarget::Replace { zone_idx, slot_idx },
                };
                open_picker(app, view, target);
            }
        }
        KeyCode::Char('a') => {
            if let Some(row) = list_rows.get(view.cursor) {
                let zone_idx = match *row {
                    Row::ZoneHeader { zone_idx } | Row::Slot { zone_idx, .. } => zone_idx,
                };
                open_picker(app, view, PickerTarget::Append { zone_idx });
            }
        }
        KeyCode::Char('d') | KeyCode::Char('x') => {
            if let Some(Row::Slot { zone_idx, slot_idx, .. }) = list_rows.get(view.cursor) {
                remove_slot(app, view, *zone_idx, *slot_idx);
            }
        }
        KeyCode::Char('J') => {
            if let Some(Row::Slot { zone_idx, slot_idx, .. }) = list_rows.get(view.cursor) {
                reorder(app, view, *zone_idx, *slot_idx, 1);
            }
        }
        KeyCode::Char('K') => {
            if let Some(Row::Slot { zone_idx, slot_idx, .. }) = list_rows.get(view.cursor) {
                reorder(app, view, *zone_idx, *slot_idx, -1);
            }
        }
        _ => {}
    }
}

fn handle_picker_key(app: &mut App, view: &mut BlocksView, key: KeyEvent) {
    let Some(picker) = &mut view.picker else { return };
    let n = picker.entries.len();

    match key.code {
        KeyCode::Esc => view.picker = None,
        KeyCode::Char('j') | KeyCode::Down => {
            if n > 0 {
                picker.cursor = (picker.cursor + 1).min(n - 1);
                update_scroll(picker.cursor, &mut picker.scroll);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            picker.cursor = picker.cursor.saturating_sub(1);
            update_scroll(picker.cursor, &mut picker.scroll);
        }
        KeyCode::Enter => apply_pick(app, view),
        _ => {}
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::Settings;
    use crate::plugins::PluginPaneView;
    use crate::zones::{Zone, BUILTIN_WIDGETS};

    fn app_with(layout: Layout, panes: Vec<PluginPaneView>) -> App {
        let mut app = App::new(Screen::Main, Settings::default());
        app.layout = layout;
        app.plugin_panes = panes;
        app.rebuild_dashboard();
        app
    }

    fn plugin(id: &str) -> PluginPaneView {
        PluginPaneView {
            id: id.into(),
            title: format!("{id} pane"),
            zone: None,
            span: 1,
            weight: 1,
            lines: vec![],
        }
    }

    #[test]
    fn rows_interleave_headers_and_slots_in_zone_order() {
        let layout = Layout {
            columns: 1,
            zones: vec![
                Zone { widgets: vec!["symlinks".into(), "tools".into()], ..Zone::new("a") },
                Zone::new("empty"),
            ],
        };
        let app = app_with(layout, vec![]);
        let rows = rows(&app);
        assert!(matches!(rows[0], Row::ZoneHeader { zone_idx: 0 }));
        assert!(matches!(rows[1], Row::Slot { zone_idx: 0, slot_idx: 0, .. }));
        assert!(matches!(rows[2], Row::Slot { zone_idx: 0, slot_idx: 1, .. }));
        assert!(matches!(rows[3], Row::ZoneHeader { zone_idx: 1 }), "an empty zone still gets a header row");
        assert_eq!(rows.len(), 4);
    }

    #[test]
    fn catalog_lists_builtins_then_live_plugin_panes_in_order() {
        let app = app_with(Layout::default(), vec![plugin("gh"), plugin("aws")]);
        let cat = catalog(&app);
        assert_eq!(cat.len(), BUILTIN_WIDGETS.len() + 2);
        for (i, id) in BUILTIN_WIDGETS.iter().enumerate() {
            assert_eq!(cat[i].id, *id);
        }
        assert_eq!(cat[BUILTIN_WIDGETS.len()].id, "gh");
        assert_eq!(cat[BUILTIN_WIDGETS.len() + 1].id, "aws");
    }

    #[test]
    fn row_label_falls_back_to_the_raw_id_for_a_dangling_widget() {
        let app = app_with(Layout::default(), vec![]);
        assert_eq!(row_label(&app, "symlinks"), "Symlinks");
        assert_eq!(row_label(&app, "nope-not-real"), "nope-not-real");
    }

    #[test]
    fn apply_pick_replaces_or_appends() {
        let mut layout = Layout {
            columns: 1,
            zones: vec![Zone { widgets: vec!["symlinks".into()], ..Zone::new("a") }],
        };
        apply_pick_to(&mut layout, PickerTarget::Replace { zone_idx: 0, slot_idx: 0 }, "tools".into());
        assert_eq!(layout.zones[0].widgets, vec!["tools".to_string()]);

        apply_pick_to(&mut layout, PickerTarget::Append { zone_idx: 0 }, "network".into());
        assert_eq!(layout.zones[0].widgets, vec!["tools".to_string(), "network".to_string()]);
    }

    #[test]
    fn remove_slot_drops_only_the_targeted_widget() {
        let mut layout = Layout {
            columns: 1,
            zones: vec![Zone { widgets: vec!["symlinks".into(), "tools".into()], ..Zone::new("a") }],
        };
        remove_slot_from(&mut layout, 0, 0);
        assert_eq!(layout.zones[0].widgets, vec!["tools".to_string()]);
    }

    #[test]
    fn reorder_swaps_within_the_zone_and_refuses_to_cross_its_edges() {
        let mut layout = Layout {
            columns: 1,
            zones: vec![Zone { widgets: vec!["symlinks".into(), "tools".into(), "network".into()], ..Zone::new("a") }],
        };
        assert!(reorder_in(&mut layout, 0, 0, 1));
        assert_eq!(layout.zones[0].widgets, vec!["tools".to_string(), "symlinks".to_string(), "network".to_string()]);

        assert!(!reorder_in(&mut layout, 0, 0, -1), "can't move left past the first slot");
        assert!(!reorder_in(&mut layout, 0, 2, 1), "can't move right past the last slot");
    }

    #[test]
    fn removing_a_widget_never_leaves_focus_out_of_bounds() {
        // Mirrors App::rebuild_dashboard's own focus-reclamping test coverage,
        // but exercised through the mutation this screen actually performs.
        let mut layout = Layout {
            columns: 1,
            zones: vec![Zone { widgets: vec!["symlinks".into()], ..Zone::new("a") }],
        };
        let mut app = app_with(layout.clone(), vec![]);
        app.dash_focus = 0;
        remove_slot_from(&mut layout, 0, 0);
        app.layout = layout;
        app.rebuild_dashboard();
        assert_eq!(app.dash_focus, 0);
        assert!(app.dash.is_empty());
    }
}
