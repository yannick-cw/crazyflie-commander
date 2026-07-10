use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Gauge, Paragraph},
};

use crate::model::Model;
use drone_control::Telemetry;
use theme::*;

// I can't be bothered to write the view by hand -- this is fully claude generate

/// Semantic palette. Swap these to restyle the whole screen in one place.
/// Named ANSI colours adapt to the terminal's own theme; use `Color::Rgb`/tailwind for a fixed look.
mod theme {
    use ratatui::style::Color;

    pub const BRAND: Color = Color::Cyan; // shell border + title accent
    pub const BORDER: Color = Color::DarkGray; // panel borders
    pub const TITLE: Color = Color::Gray; // panel titles
    pub const LABEL: Color = Color::DarkGray; // metric labels / dim text
    pub const CHIP_FG: Color = Color::Black; // text on a coloured chip

    pub const POSITION: Color = Color::Cyan;
    pub const VELOCITY: Color = Color::Green;
    pub const HEADING: Color = Color::Yellow;
    pub const MISSION: Color = Color::Magenta;

    pub const OK: Color = Color::Green;
    pub const WARN: Color = Color::Yellow;
    pub const DANGER: Color = Color::Red;
}

/// Speed (m/s) that maps to a full gauge / "hot" colour.
const MAX_SPEED: f32 = 2.5;

pub fn view(model: &Model, frame: &mut Frame) {
    let t = &model.telemetry;
    let area = frame.area();

    let shell = shell();
    let inner = shell.inner(area);
    frame.render_widget(shell, area);

    // rows: mission bar · body · speed gauge
    let [mission_area, body, speed_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .areas(inner);

    // body: map (fills) · telemetry sidebar (fixed width)
    let [map_area, side] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(26)]).areas(body);

    // sidebar: position · velocity · state
    let [pos_area, vel_area, state_area] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(5),
        Constraint::Min(0),
    ])
    .areas(side);

    // TODO: dummy until the model carries mission + progress
    let mission = "Orbit";
    let progress = 0.42;

    frame.render_widget(mission_bar(mission, progress), mission_area);
    frame.render_widget(map_placeholder(), map_area);
    frame.render_widget(position_panel(t), pos_area);
    frame.render_widget(velocity_panel(t), vel_area);
    frame.render_widget(state_panel(t), state_area);
    frame.render_widget(speed_gauge(t), speed_area);
}

fn shell() -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BRAND))
        .title(accent(" ⬡ CRAZYFLIE · COMMANDER "))
        .title_alignment(Alignment::Center)
        .title_bottom(controls().centered())
}

/// Bottom-bar key hints: emergency stop, safe land, quit.
fn controls() -> Line<'static> {
    let key = |k: &str, label: &str, color: Color| {
        vec![
            Span::styled(
                format!(" {k} "),
                Style::new()
                    .fg(CHIP_FG)
                    .bg(color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {label}   "), Style::new().fg(color)),
        ]
    };
    Line::from(
        [
            key("x", "EMERGENCY STOP", DANGER),
            key("l", "LAND", WARN),
            key("q", "quit", LABEL),
        ]
        .concat(),
    )
}

fn mission_bar(name: &str, progress: f64) -> Gauge<'static> {
    Gauge::default()
        .block(panel(format!(" MISSION · {name} ")))
        .gauge_style(Style::new().fg(MISSION))
        .ratio(progress.clamp(0.0, 1.0))
        .label(format!("{:.0}%", progress * 100.0))
}

fn map_placeholder() -> Paragraph<'static> {
    Paragraph::new("\n· map ·")
        .alignment(Alignment::Center)
        .style(Style::new().fg(LABEL))
        .block(panel(" MAP "))
}

fn position_panel(t: &Telemetry) -> Paragraph<'static> {
    Paragraph::new(vec![
        metric("X", format!("{:+.2} m", t.x()), POSITION),
        metric("Y", format!("{:+.2} m", t.y()), POSITION),
        metric("Z", format!("{:+.2} m", t.z()), POSITION),
    ])
    .block(panel(" POSITION "))
}

fn velocity_panel(t: &Telemetry) -> Paragraph<'static> {
    Paragraph::new(vec![
        metric("VX", format!("{:+.2} m/s", t.vx()), VELOCITY),
        metric("VY", format!("{:+.2} m/s", t.vy()), VELOCITY),
        metric("|V|", format!("{:.2} m/s", t.speed()), VELOCITY),
    ])
    .block(panel(" VELOCITY "))
}

fn state_panel(t: &Telemetry) -> Paragraph<'static> {
    let (label, color) = if t.is_low_bat() {
        (" LOW ", DANGER)
    } else {
        (" OK ", OK)
    };
    Paragraph::new(vec![
        metric("YAW", format!("{:+.0}°", t.yaw()), HEADING),
        Line::from(vec![
            Span::styled(" BAT ", Style::new().fg(LABEL)),
            Span::styled(
                label.to_string(),
                Style::new()
                    .fg(CHIP_FG)
                    .bg(color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ])
    .block(panel(" STATE "))
}

fn speed_gauge(t: &Telemetry) -> Gauge<'static> {
    let speed = t.speed();
    Gauge::default()
        .block(panel(" SPEED "))
        .gauge_style(Style::new().fg(speed_color(speed)))
        .ratio((speed / MAX_SPEED).clamp(0.0, 1.0) as f64)
        .label(format!("{speed:.2} m/s"))
}

/// A dim label + a bright, bold value on one line.
fn metric(label: &str, value: String, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {label:<4}"), Style::new().fg(LABEL)),
        Span::styled(value, Style::new().fg(color).add_modifier(Modifier::BOLD)),
    ])
}

/// A subtle rounded sub-panel with a bold grey title.
fn panel(title: impl Into<String>) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BORDER))
        .title(Span::styled(
            title.into(),
            Style::new().fg(TITLE).add_modifier(Modifier::BOLD),
        ))
}

fn accent(s: &str) -> Span<'static> {
    Span::styled(
        s.to_string(),
        Style::new().fg(BRAND).add_modifier(Modifier::BOLD),
    )
}

/// Green (slow) → yellow → red (fast).
fn speed_color(speed: f32) -> Color {
    if speed < MAX_SPEED * 0.33 {
        OK
    } else if speed < MAX_SPEED * 0.66 {
        WARN
    } else {
        DANGER
    }
}