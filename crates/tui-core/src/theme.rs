//! The shared color palette.
//!
//! A minimal, low-contrast terminal theme: the background is whatever the
//! terminal already is, chrome recedes into muted grey, and color is spent only
//! where it carries meaning — cyan for the thing you are looking at, green /
//! amber / red for status.
//!
//! Borders use explicit Nord-ish greys rather than `DarkGray` so they stay
//! subtly *below* muted text on terminals that render `DarkGray` brightly;
//! everything else stays on the 16-color ANSI palette so it inherits whatever
//! scheme the user's terminal already uses.

use ratatui::style::{Color, Modifier, Style};

// ── palette ───────────────────────────────────────────────────────────────────

/// `#3B4252` — the quietest line on screen: borders of unfocused blocks.
pub const BORDER: Color = Color::Rgb(0x3B, 0x42, 0x52);
/// `#4C566A` — one step up: separators and rules that should still read as structure.
pub const RULE: Color = Color::Rgb(0x4C, 0x56, 0x6A);

// ── status glyphs ─────────────────────────────────────────────────────────────
//
// Kept beside the palette because glyph and color are chosen together: a reader
// scanning a column of bullets gets the state from the shape even in a terminal
// with no color at all.

/// Fully operational / installed.
pub const DOT_OK: &str = "●";
/// Partially there — some of a group present, work pending.
pub const DOT_PARTIAL: &str = "◐";
/// Nothing yet, but nothing wrong either.
pub const DOT_PENDING: &str = "○";
/// Missing or broken.
pub const MARK_BAD: &str = "✗";

// ── roles ─────────────────────────────────────────────────────────────────────

/// Screen and app titles — the highest-contrast thing on screen.
pub fn style_title() -> Style {
    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

/// The subtitle trailing a title, and other secondary labels.
pub fn style_subtitle() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Block borders at rest.
pub fn style_border() -> Style {
    Style::default().fg(BORDER)
}

/// The border of the focused block — the only cyan in the grid, so focus is
/// unambiguous at a glance.
pub fn style_border_focus() -> Style {
    Style::default().fg(Color::Cyan)
}

/// Horizontal rules separating the chrome from the content.
pub fn style_rule() -> Style {
    Style::default().fg(RULE)
}

/// Block titles, inlined into the top border. Deliberately muted: the title
/// labels the box, it is not the content.
pub fn style_block_title() -> Style {
    Style::default().fg(Color::Gray)
}

/// The title of a focused block, bright enough to track without hunting.
pub fn style_block_title_focus() -> Style {
    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

/// Primary body text — crisp, but not bold.
pub fn style_text() -> Style {
    Style::default().fg(Color::Gray)
}

/// The primary name in a list row, the thing being named.
pub fn style_name() -> Style {
    Style::default().fg(Color::White)
}

/// Secondary text: paths, versions, counts, summaries, hints.
pub fn style_muted() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// The key glyph in a keybinding hint (`enter`, `hjkl`, `space`).
pub fn style_key() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// The action a keybinding performs — lighter than the key, so the eye lands on
/// what it does before how to do it.
pub fn style_action() -> Style {
    Style::default().fg(Color::Gray)
}

/// Good: installed, linked, online.
pub fn style_ok() -> Style {
    Style::default().fg(Color::Green)
}

/// Partial or pending — present but incomplete.
pub fn style_warn_soft() -> Style {
    Style::default().fg(Color::Yellow)
}

/// Missing or broken. A soft red, not the alarm-bell bright one.
pub fn style_bad() -> Style {
    Style::default().fg(Color::LightRed)
}

/// The currently selected row in a list.
pub fn style_selected() -> Style {
    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

// ── back-compat aliases ───────────────────────────────────────────────────────
//
// The original five roles, kept so existing call sites (and `ssm`, which shares
// this crate) keep compiling and land on the retuned palette for free.

/// Titles and highlights. Alias of [`style_title`].
pub fn style_header() -> Style { style_title() }
/// Success / selection. Bold green.
pub fn style_select() -> Style { style_ok().add_modifier(Modifier::BOLD) }
/// Errors. Bold soft red.
pub fn style_error() -> Style { style_bad().add_modifier(Modifier::BOLD) }
/// Warnings. Bold amber.
pub fn style_warn() -> Style { style_warn_soft().add_modifier(Modifier::BOLD) }
/// Muted secondary text. Alias of [`style_muted`].
pub fn style_dim() -> Style { style_muted() }
