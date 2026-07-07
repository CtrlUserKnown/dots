use ratatui::style::{Color, Modifier, Style};

pub fn style_header() -> Style { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) }
pub fn style_select() -> Style { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) }
pub fn style_error()  -> Style { Style::default().fg(Color::Red).add_modifier(Modifier::BOLD) }
pub fn style_dim()    -> Style { Style::default().fg(Color::DarkGray) }
