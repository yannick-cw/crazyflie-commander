//! Shared view primitives so every screen (flight / home / mission-select) looks the same.

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType},
};

use theme::*;

// AI GENERATED

/// Semantic palette. Swap these to restyle every screen in one place.
/// Named ANSI colours adapt to the terminal's own theme; use `Color::Rgb` for a fixed look.
pub mod theme {
    use ratatui::style::Color;

    pub const BRAND: Color = Color::Cyan; // shell border + title accent
    pub const BORDER: Color = Color::DarkGray; // panel borders
    pub const TITLE: Color = Color::Gray; // panel titles
    pub const LABEL: Color = Color::DarkGray; // metric labels / dim text
    pub const CHIP_FG: Color = Color::Black; // text on a coloured chip

    pub const SELECTED: Color = Color::LightGreen;

    pub const POSITION: Color = Color::Cyan;
    pub const VELOCITY: Color = Color::Green;
    pub const HEADING: Color = Color::Yellow;
    pub const MISSION: Color = Color::Magenta;

    pub const OK: Color = Color::Green;
    pub const WARN: Color = Color::Yellow;
    pub const DANGER: Color = Color::Red;
}

/// The outer shell — same framing on every screen; each supplies its own footer via [`controls`].
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

/// A fixed-size rectangle centred inside `area`.
pub fn center(area: Rect, width: u16, height: u16) -> Rect {
    let [_, mid, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [_, centre, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width),
        Constraint::Fill(1),
    ])
    .areas(mid);
    centre
}
