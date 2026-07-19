pub mod aliases;
pub mod app;
pub mod health;
pub mod overview;
pub mod profile;
pub mod settings;
pub mod update;

// The shared chrome (header/footer/description bars), color theme, and flash
// model now live in the `tui-core` crate, shared with `ssm`. Re-export them so
// existing `crate::tui::…` paths across the dots screens keep resolving.
pub use tui_core::{draw_desc, draw_footer, draw_header, theme, FlashKind};
