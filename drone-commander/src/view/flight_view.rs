use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{
        Gauge, Paragraph, Widget,
        canvas::{Canvas, Circle},
    },
};

use crate::model::{FreeFlightState, MissionExecutionState, Model, State};
use crate::view::view_common::theme::*;
use crate::view::view_common::{controls, panel, shell};
use drone_control::{Command, Meters, MissionStatus, Telemetry};

// AI GENERATED

/// Speed (m/s) that maps to a full gauge / "hot" colour.
const MAX_SPEED: f32 = 2.5;
/// Half-extent of the square top-down map viewport, in metres (takeoff origin centred → 3x3 m).
const MAP_M: f64 = 1.5;
/// Speed setting (m/s) that maps to a full speed-setting gauge.
const MAX_SPEED_SETTING: f32 = 2.0;

pub fn view(model: &Model, frame: &mut Frame) {
    let t = &model.telemetry;
    let area = frame.area();

    let mission = match &model.state {
        State::MissionExecution(s) => Some(s),
        _ => None,
    };
    // the back-to-menu hint only makes sense once the mission is no longer running
    let show_back =
        matches!(mission, Some(s) if !matches!(s.mission_status, MissionStatus::Running(_)));

    let keys: Vec<(&str, &str, Color)> = match &model.state {
        // free flight has its own control scheme
        State::FreeFlight(s) => [
            Some(("wasd", "move", BRAND)),
            Some(("←→", "yaw", BRAND)),
            (!s.is_airborne).then_some(("t", "take off", SELECTED)),
            s.is_airborne.then_some(("l", "land", WARN)),
            Some(("x", "STOP", DANGER)),
            Some(("b", "back", WARN)),
            Some(("q", "quit", LABEL)),
        ]
        .into_iter()
        .flatten()
        .collect(),
        _ => [
            Some(("x", "EMERGENCY STOP", DANGER)),
            Some(("l", "LAND", WARN)),
            show_back.then_some(("b", "back to menu", SELECTED)),
            Some(("q", "quit", LABEL)),
        ]
        .into_iter()
        .flatten()
        .collect(),
    };

    let shell = shell(controls(&keys));
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

    match &model.state {
        State::FreeFlight(s) => frame.render_widget(free_flight_bar(s), mission_area),
        _ => frame.render_widget(mission_bar(mission), mission_area),
    }
    frame.render_widget(map(t, mission), map_area);
    frame.render_widget(position_panel(t), pos_area);
    frame.render_widget(velocity_panel(t), vel_area);
    frame.render_widget(state_panel(t), state_area);
    frame.render_widget(speed_gauge(t), speed_area);
}

fn mission_bar(mission: Option<&MissionExecutionState>) -> Gauge<'static> {
    let (title, ratio, label, color) = match mission {
        None => (" FREE FLIGHT ".to_string(), 0.0, "manual".to_string(), LABEL),
        Some(s) => {
            let title = format!(" MISSION · {} ", s.name);
            match &s.mission_status {
                MissionStatus::Idle => (title, 0.0, "idle".to_string(), LABEL),
                MissionStatus::Running(None) => (title, 0.0, "starting…".to_string(), MISSION),
                MissionStatus::Running(Some(p)) => {
                    let ratio = p.command_num as f64 / p.total_commands.max(1) as f64;
                    let step = format!("{:?}", p.current_command);
                    let step = step.split([' ', '{']).next().unwrap_or(&step);
                    let label = format!("{step} · {}/{}", p.command_num, p.total_commands);
                    (title, ratio, label, MISSION)
                }
                MissionStatus::Aborted(reason) => {
                    (title, 1.0, format!("aborted · {reason:?}"), DANGER)
                }
            }
        }
    };
    Gauge::default()
        .block(panel(title))
        .gauge_style(Style::new().fg(color))
        .ratio(ratio.clamp(0.0, 1.0))
        .label(label)
}

/// Free-flight top bar: airborne status + the current speed-setting as a gauge.
fn free_flight_bar(s: &FreeFlightState) -> Gauge<'static> {
    let setting = s.speed_setting.0;
    let status = if s.is_airborne { "airborne" } else { "grounded" };
    Gauge::default()
        .block(panel(" FREE FLIGHT · SPEED "))
        .gauge_style(Style::new().fg(BRAND))
        .ratio((setting / MAX_SPEED_SETTING).clamp(0.0, 1.0) as f64)
        .label(format!("{status} · {setting:.1} m/s"))
}

/// Top-down braille map: the planned route (every waypoint, the current one highlighted)
/// plus the drone's live position.
fn map(t: &Telemetry, mission: Option<&MissionExecutionState>) -> impl Widget {
    let drone = (t.x() as f64, t.y() as f64);
    let route = mission.map(|m| waypoints(&m.mission)).unwrap_or_default();
    let current = mission.and_then(current_index);
    // top-down as seen by the pilot: origin (takeoff) centred,
    // x (forward) grows up the screen, y (left) grows to the left
    let tf = |(x, y): (f64, f64)| (-y, x);
    Canvas::default()
        .block(panel(" MAP "))
        .marker(Marker::Braille)
        .x_bounds([-MAP_M, MAP_M])
        .y_bounds([-MAP_M, MAP_M])
        .paint(move |ctx| {
            // every waypoint visible; the one being flown to is highlighted bigger + green
            for (i, &point) in route.iter().enumerate() {
                let (px, py) = tf(point);
                let (radius, color) = if Some(i) == current {
                    (0.10, SELECTED)
                } else {
                    (0.06, MISSION)
                };
                ctx.draw(&Circle { x: px, y: py, radius, color });
            }
            // the drone
            let (dx, dy) = tf(drone);
            ctx.draw(&Circle { x: dx, y: dy, radius: 0.13, color: POSITION });
        })
}

/// Each command's target point in map metres (takeoff origin), threaded through a
/// moving cursor so relative moves accumulate into the flown route.
fn waypoints(mission: &[Command]) -> Vec<(f64, f64)> {
    let m = |v: &Meters| v.0 as f64;
    mission
        .iter()
        .scan((0.0, 0.0), |cursor, cmd| {
            *cursor = match cmd {
                Command::Move { x, y, .. } => (cursor.0 + m(x), cursor.1 + m(y)),
                Command::MoveToWaypoint { x, y, .. } => (m(x), m(y)),
                Command::SmoothPath { waypoints, .. } => {
                    waypoints.last().map(|w| (m(&w.x), m(&w.y))).unwrap_or(*cursor)
                }
                Command::BilliardBox(p) => (
                    (m(&p.bl_x) + m(&p.tr_x)) / 2.0,
                    (m(&p.bl_y) + m(&p.tr_y)) / 2.0,
                ),
                // Takeoff / Orbit / Hover / Land hold position
                _ => *cursor,
            };
            Some(*cursor)
        })
        .collect()
}

fn current_index(mission: &MissionExecutionState) -> Option<usize> {
    match &mission.mission_status {
        MissionStatus::Running(Some(p)) => Some(p.command_num),
        _ => None,
    }
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
