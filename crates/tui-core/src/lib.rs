//! Shared TUI primitives for the `dots` and `ssm` tools.
//!
//! Both binaries render with ratatui and share the same chrome (header/footer/
//! description bars), color theme, and flash-message model. Keeping them here
//! means a fix in one place lands in both tools. The lazygit-style panel/focus
//! framework will grow in this crate too.

use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub mod theme;

use theme::{
    style_action, style_block_title, style_block_title_focus, style_border, style_border_focus,
    style_dim, style_error, style_key, style_muted, style_rule, style_select, style_subtitle,
    style_title,
};

#[derive(Debug, Clone, PartialEq)]
pub enum FlashKind {
    Success,
    Error,
    Info,
}

// ── blocks ────────────────────────────────────────────────────────────────────

/// A titled block in the house style: thin rounded border, title inlined at the
/// left of the top border in muted grey, brightening to cyan when focused.
///
/// Every bordered box goes through here so corner style, title placement, and
/// the focused/unfocused contrast stay identical across screens.
pub fn block(title: &str, focused: bool) -> Block<'static> {
    let (border, text) = if focused {
        (style_border_focus(), style_block_title_focus())
    } else {
        (style_border(), style_block_title())
    };

    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Span::styled(format!(" {} ", title.trim()), text))
}

/// An untitled rounded block, for regions that are pure grouping.
pub fn plain_block(focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focused { style_border_focus() } else { style_border() })
}

// ── chrome ────────────────────────────────────────────────────────────────────

/// The top bar: cyan app title, muted subtitle beside it, and right-aligned
/// muted keybinding hints. One row tall, like the rule it replaces, so screen
/// content areas are unaffected.
pub fn draw_top_bar(f: &mut Frame, area: Rect, title: &str, subtitle: &str, hints: &[(&str, &str)]) {
    if area.width == 0 {
        return;
    }
    let row = Rect { x: area.x, y: area.y, width: area.width, height: 1 };

    let mut left = vec![
        Span::styled("  ", style_muted()),
        Span::styled(title.trim().to_string(), style_title()),
    ];
    if !subtitle.is_empty() {
        left.push(Span::styled(format!("  {}", subtitle.trim()), style_subtitle()));
    }
    f.render_widget(Paragraph::new(Line::from(left)), row);

    // Hints are drawn as a second, right-aligned pass rather than padded into
    // the first: measuring styled spans to pad by hand goes wrong the moment a
    // title contains a wide glyph.
    if hints.is_empty() {
        return;
    }
    let spans = key_spans(hints);
    let width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let width = width + 2;
    if width >= area.width as usize {
        return; // no room; the title matters more
    }
    let x = area.x + area.width - width as u16;
    f.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect { x, y: area.y, width: width as u16, height: 1 },
    );
}

/// The top bar for a screen that is one of several siblings: the app name, then
/// every sibling screen as a tab with the active one lit, then right-aligned
/// hints. Seeing the whole set at once is what makes the screens read as one
/// app rather than a handful of dead ends.
///
/// On a terminal too narrow for the full strip, only the active tab is drawn —
/// knowing where you are matters more than knowing where you could go.
pub fn draw_nav_bar(f: &mut Frame, area: Rect, app: &str, tabs: &[(&str, bool)], hints: &[(&str, &str)]) {
    if area.width == 0 {
        return;
    }
    let row = Rect { x: area.x, y: area.y, width: area.width, height: 1 };

    let hint_spans = key_spans(hints);
    let hint_w: usize = hint_spans.iter().map(|s| s.content.chars().count()).sum();
    let hint_w = if hint_spans.is_empty() { 0 } else { hint_w + 2 };

    let mut left = vec![
        Span::styled("  ", style_muted()),
        Span::styled(app.trim().to_string(), style_title()),
        Span::styled("  ", style_muted()),
    ];
    let head_w: usize = left.iter().map(|s| s.content.chars().count()).sum();

    let full_w: usize = tabs.iter().map(|(l, _)| l.chars().count() + 2).sum();
    let show_all = head_w + full_w + hint_w <= area.width as usize;
    if show_all {
        for (label, active) in tabs {
            left.push(Span::styled(
                format!(" {label} "),
                if *active { style_title() } else { style_subtitle() },
            ));
        }
    } else if let Some((label, _)) = tabs.iter().find(|(_, active)| *active) {
        left.push(Span::styled(format!(" {label} "), style_title()));
    }

    // The hints are the first thing to go: they are drawn as a second pass over
    // the same row, so keeping them when they don't fit would paint them across
    // the tab that says where you are.
    let left_w: usize = left.iter().map(|s| s.content.chars().count()).sum();
    f.render_widget(Paragraph::new(Line::from(left)), row);

    if hint_w > 0 && left_w + hint_w <= area.width as usize {
        let x = area.x + area.width - hint_w as u16;
        f.render_widget(
            Paragraph::new(Line::from(hint_spans)),
            Rect { x, y: area.y, width: hint_w as u16, height: 1 },
        );
    }
}

/// `key action   key action` — key glyphs dimmed, actions in lighter text, so a
/// bar of hints reads as actions first and syntax second.
fn key_spans(hints: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(hints.len() * 3);
    for (i, (key, action)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("   ", style_muted()));
        }
        spans.push(Span::styled((*key).to_string(), style_key()));
        if !action.is_empty() {
            spans.push(Span::styled(format!(" {action}"), style_action()));
        }
    }
    spans
}

/// The bottom status bar: a muted rule, then one clean line of keybindings.
/// Occupies the same two rows [`draw_footer`] does.
pub fn draw_key_bar(f: &mut Frame, area: Rect, hints: &[(&str, &str)]) {
    if area.height < 3 {
        return;
    }
    let border_y = area.y + area.height - 3;
    let hint_y = area.y + area.height - 2;

    f.render_widget(
        Paragraph::new(Line::from(Span::styled("─".repeat(area.width as usize), style_rule()))),
        Rect { x: area.x, y: border_y, width: area.width, height: 1 },
    );

    let mut spans = vec![Span::styled("  ", style_muted())];
    spans.extend(key_spans(hints));
    f.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect { x: area.x, y: hint_y, width: area.width, height: 1 },
    );
}

/// Renders the top border line with the title at col 4 and the version near the right edge.
pub fn draw_header(f: &mut Frame, area: Rect, title: &str, version: &str) {
    let right = if version.is_empty() { String::new() } else { format!("v{version}") };
    draw_header_right(f, area, title, &right);
}

/// Renders the top border line with the title at col 4 and arbitrary text
/// right-aligned near the right edge — e.g. quick keybinding hints once the
/// version tag has moved elsewhere. Unlike [`draw_header`], `right` is shown
/// verbatim (no "v" prefix).
pub fn draw_header_right(f: &mut Frame, area: Rect, title: &str, right: &str) {
    let w = area.width as usize;
    if w == 0 { return; }

    let mut chars: Vec<char> = "─".repeat(w).chars().collect();

    for (i, ch) in title.chars().enumerate() {
        let pos = 4 + i;
        if pos < w { chars[pos] = ch; }
    }

    if !right.is_empty() {
        let text = format!(" {right} ");
        let start = w.saturating_sub(text.len() + 2).max(4 + title.len() + 1);
        for (i, ch) in text.chars().enumerate() {
            let pos = start + i;
            if pos < w { chars[pos] = ch; }
        }
    }

    // Drawn in three spans rather than one so the rule recedes to border grey
    // while the title keeps its contrast.
    let chars: Vec<char> = chars;
    let title_start = 4.min(w);
    let title_end = (4 + title.chars().count()).min(w);
    let take = |a: usize, b: usize| -> String { chars[a..b].iter().collect() };

    let rect = Rect { x: area.x, y: area.y, width: area.width, height: 1 };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(take(0, title_start), style_rule()),
            Span::styled(take(title_start, title_end), style_title()),
            Span::styled(take(title_end, w), style_rule()),
        ])),
        rect,
    );
}

/// Renders ───── border at h-3 and hint text at h-2.
pub fn draw_footer(f: &mut Frame, area: Rect, hint: &str) {
    if area.height < 3 { return; }

    let border_y = area.y + area.height - 3;
    let hint_y   = area.y + area.height - 2;

    let border = "─".repeat(area.width as usize);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(border, style_rule()))),
        Rect { x: area.x, y: border_y, width: area.width, height: 1 },
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(hint, style_dim()))),
        Rect { x: area.x, y: hint_y, width: area.width, height: 1 },
    );
}

/// Renders the description / flash bar at h-4.
pub fn draw_desc(f: &mut Frame, area: Rect, text: &str, flash: Option<&(String, FlashKind)>) {
    if area.height < 4 { return; }

    let desc_y    = area.y + area.height - 4;
    let desc_rect = Rect { x: area.x, y: desc_y, width: area.width, height: 1 };

    let widget = match flash {
        Some((msg, kind)) => {
            let style = match kind {
                FlashKind::Success => style_select(),
                FlashKind::Error   => style_error(),
                FlashKind::Info    => style_dim().add_modifier(Modifier::BOLD),
            };
            Paragraph::new(Line::from(Span::styled(format!("  {msg}"), style)))
        }
        None => Paragraph::new(Line::from(vec![
            Span::styled("  › ", style_muted()),
            Span::styled(text.to_string(), style_action()),
        ])),
    };

    f.render_widget(widget, desc_rect);
}

// ── list rows ─────────────────────────────────────────────────────────────────

/// The state a list row reports, which picks both its bullet and its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Fully operational — `●` green.
    Ok,
    /// Present but incomplete — `◐` amber.
    Partial,
    /// Nothing yet, and that is fine — `○` amber.
    Pending,
    /// Missing or broken — `✗` red.
    Bad,
}

impl Status {
    pub fn glyph(self) -> &'static str {
        match self {
            Status::Ok => theme::DOT_OK,
            Status::Partial => theme::DOT_PARTIAL,
            Status::Pending => theme::DOT_PENDING,
            Status::Bad => theme::MARK_BAD,
        }
    }

    pub fn style(self) -> ratatui::style::Style {
        match self {
            Status::Ok => theme::style_ok(),
            Status::Partial | Status::Pending => theme::style_warn_soft(),
            Status::Bad => theme::style_bad(),
        }
    }

    /// Derive a status from a "how many of these are good" count, the shape
    /// nearly every summary in the app already has.
    pub fn from_counts(ok: usize, total: usize) -> Status {
        match (ok, total) {
            (_, 0) => Status::Pending,
            (o, t) if o == t => Status::Ok,
            (0, _) => Status::Bad,
            _ => Status::Partial,
        }
    }
}

/// One list row: `● name              meta`.
///
/// The name is left-aligned and the metadata right-aligned against `width`,
/// with the gap padded so a column of rows lines up. When the two cannot both
/// fit, the name is truncated with an ellipsis — the metadata is the shorter,
/// denser half and the part worth keeping whole.
pub fn status_row(status: Status, name: &str, meta: &str, width: u16) -> Line<'static> {
    let width = width as usize;
    let glyph = status.glyph();
    // bullet + space, then the name, then at least one space before the meta.
    let fixed = glyph.chars().count() + 1;
    let avail = width.saturating_sub(fixed);
    let meta_len = meta.chars().count();

    let (name, pad) = if meta_len + 1 >= avail {
        (truncate(name, avail), 0)
    } else {
        let room = avail - meta_len;
        let name = truncate(name, room.saturating_sub(1));
        (name.clone(), room - name.chars().count())
    };

    let mut spans = vec![
        Span::styled(format!("{glyph} "), status.style()),
        Span::styled(name, theme::style_name()),
    ];
    if pad > 0 {
        spans.push(Span::styled(" ".repeat(pad), theme::style_muted()));
        spans.push(Span::styled(meta.to_string(), theme::style_muted()));
    }
    Line::from(spans)
}

/// The dim one-liner at the bottom of a block body: `4 links · 3 ok · 1 missing`.
pub fn summary_line(parts: &[String]) -> Line<'static> {
    Line::from(Span::styled(parts.join(" · "), style_muted()))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "…".repeat(max);
    }
    s.chars().take(max - 1).chain(std::iter::once('…')).collect()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn header_no_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| {
            let area = f.area();
            draw_header(f, area, " dots ", "1.0.0");
            draw_footer(f, area, " q quit ");
        }).unwrap();
    }

    /// A row must always fit the width it was measured for, or the columns in a
    /// block stop lining up and long names bleed over their neighbours.
    #[test]
    fn status_rows_fit_their_width() {
        for width in [10u16, 20, 40, 80] {
            for (name, meta) in [
                ("short", "ok"),
                ("a-really-long-config-name-that-overflows", "3/4 installed"),
                ("x", ""),
            ] {
                let line = status_row(Status::Ok, name, meta, width);
                let len: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
                assert!(len <= width as usize, "{name:?}/{meta:?} at {width}: {len} > {width}");
            }
        }
    }

    #[test]
    fn status_row_right_aligns_its_meta() {
        let line = status_row(Status::Ok, "git", "2.44", 20);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "● git           2.44");
        assert_eq!(text.chars().count(), 20, "the row fills the width it was given");
    }

    #[test]
    fn status_maps_counts_to_glyphs() {
        assert_eq!(Status::from_counts(3, 3).glyph(), theme::DOT_OK);
        assert_eq!(Status::from_counts(1, 3).glyph(), theme::DOT_PARTIAL);
        assert_eq!(Status::from_counts(0, 3).glyph(), theme::MARK_BAD);
        assert_eq!(Status::from_counts(0, 0).glyph(), theme::DOT_PENDING);
    }

    /// The hints are a second pass over the same row, so a width where they no
    /// longer fit must drop them rather than paint them over the active tab.
    #[test]
    fn a_narrow_nav_bar_keeps_the_active_tab_intact() {
        let tabs = [("symlinks", false), ("tools", true), ("configs", false)];
        for width in 20u16..70 {
            let mut term = Terminal::new(TestBackend::new(width, 3)).unwrap();
            term.draw(|f| {
                draw_nav_bar(f, Rect::new(0, 0, width, 3), "dots", &tabs, &[("[ ]", "screen")])
            })
            .unwrap();

            let b = term.backend().buffer().clone();
            let row: String = (0..width).map(|x| b[(x, 0)].symbol().to_string()).collect();
            assert!(
                row.contains("tools"),
                "width {width}: active tab was overwritten — got {row:?}",
            );
        }
    }

    #[test]
    fn chrome_survives_a_narrow_terminal() {
        let mut terminal = Terminal::new(TestBackend::new(24, 10)).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                draw_top_bar(f, area, "dots", "dotfiles manager", &[("q", "quit"), ("?", "help")]);
                draw_key_bar(f, area, &[("hjkl", "move"), ("enter", "open")]);
            })
            .unwrap();
    }

    #[test]
    fn small_terminal_shows_guard_message() {
        let backend = TestBackend::new(30, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| {
            let area = f.area();
            if area.width < 50 || area.height < 14 {
                let msg = "Terminal too small — need at least 50×14";
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(msg, super::theme::style_error()))),
                    Rect { x: 0, y: area.height / 2, width: area.width, height: 1 },
                );
            }
        }).unwrap();
    }
}
