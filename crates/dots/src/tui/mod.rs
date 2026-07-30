pub mod aliases;
pub mod app;
pub mod blocks;
pub mod configs;
pub mod health;
pub mod overview;
pub mod profile;
pub mod settings;
pub mod update;

// The shared chrome (header/footer/description bars), color theme, and flash
// model now live in the `tui-core` crate, shared with `ssm`. Re-export them so
// existing `crate::tui::…` paths across the dots screens keep resolving.
pub use tui_core::{
    block, draw_banner, draw_desc, draw_footer, draw_header, draw_header_right, draw_key_bar,
    draw_nav_bar, draw_top_bar, plain_block, status_row, summary_line, theme, FlashKind, Status,
};

/// The nav strip every sibling screen draws at the top. Screens call this
/// rather than `draw_nav_bar` directly so the app name, tab order, and the
/// hints beside them stay identical across all of them.
pub fn draw_screen_nav(f: &mut ratatui::Frame, area: ratatui::layout::Rect, screen: app::Screen) {
    draw_nav_bar(
        f,
        area,
        "dots",
        &app::nav_tabs(screen),
        &[("[ ]", "screen"), ("esc", "home")],
    );
}
