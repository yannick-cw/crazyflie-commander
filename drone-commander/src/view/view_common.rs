//! Shared view primitives so every screen (flight / home / mission-select) looks the same.

use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType},
};

pub use crate::view::flight_view::theme; // re-export the palette for the view modules
use crate::view::flight_view::theme::*;

/// The outer shell — identical framing to the flight view, but each screen supplies
/// its own footer of key hints via [`controls`].
pub fn shell(controls: Line<'static>) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BRAND))
        .title(Span::styled(
            " ⬡ CRAZYFLIE · COMMANDER ",
            Style::new().fg(BRAND).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .title_bottom(controls.centered())
}

/// A subtle rounded sub-panel with a bold grey title.
pub fn panel(title: impl Into<String>) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BORDER))
        .title(Span::styled(
            title.into(),
            Style::new().fg(TITLE).add_modifier(Modifier::BOLD),
        ))
}

/// Footer key hints: one `[key] label` chip per `(key, label, colour)`.
pub fn controls(keys: &[(&str, &str, Color)]) -> Line<'static> {
    let spans = keys
        .iter()
        .flat_map(|(key, label, color)| {
            [
                Span::styled(
                    format!(" {key} "),
                    Style::new()
                        .fg(CHIP_FG)
                        .bg(*color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {label}   "), Style::new().fg(*color)),
            ]
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

/// A selectable row: `▸ label` highlighted when selected, dim otherwise.
pub fn selectable(label: &str, selected: bool) -> Line<'static> {
    let style = if selected {
        Style::new().fg(SELECTED).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(LABEL)
    };
    let marker = if selected { " ▸ " } else { "   " };
    Line::from(vec![
        Span::styled(marker, style),
        Span::styled(label.to_string(), style),
    ])
}
