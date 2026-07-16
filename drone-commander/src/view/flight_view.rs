use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{
        Gauge, Paragraph, Widget,
        canvas::{Canvas, Circle, Line as CanvasLine},
    },
};

use crate::model::{FreeFlightState, MissionExecutionState, Model, State};
use crate::view::view_common::theme::*;
use crate::view::view_common::{controls, panel, shell};
use drone_control::{Command, Meters, MissionStatus, Setpoint, Telemetry};

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
            s.is_airborne.then_some(("h", "go home", WARN)),
            Some(if s.is_recording {
                ("r", "stop rec", DANGER)
            } else {
                ("r", "record", SELECTED)
            }),
            Some(("x", "STOP", DANGER)),
            Some(("b", "back", WARN)),
            Some(("q", "quit", LABEL)),
        ]
        .into_iter()
        .flatten()
        .collect(),
        _ => [
            show_back.then_some(("t", "start mission", SELECTED)),
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

    // sidebar: position · velocity · state · (recording, fills the rest)
    let [pos_area, vel_area, state_area, rec_area] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(5),
        Constraint::Length(4),
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
    if let State::FreeFlight(s) = &model.state {
        if s.is_recording {
            frame.render_widget(recording_panel(s), rec_area);
        }
    }
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

/// A blinking red ● REC indicator with the running sample count, shown while recording.
fn recording_panel(s: &FreeFlightState) -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                " ● ",
                Style::new()
                    .fg(DANGER)
                    .add_modifier(Modifier::BOLD | Modifier::RAPID_BLINK),
            ),
            Span::styled("REC", Style::new().fg(DANGER).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(Span::styled(
            format!(" {} samples", s.recording.len()),
            Style::new().fg(LABEL),
        )),
    ])
    .block(panel(" RECORDING "))
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

/// A piece of the planned route, in world metres.
enum PathElem {
    Seg((f64, f64), (f64, f64)),
    Ring((f64, f64), f64),
    Rect((f64, f64), (f64, f64)),
}

/// Top-down braille map: before take-off it previews the whole planned route; during
/// flight it marks the waypoints with the current one highlighted. Always shows the
/// live drone position and heading.
fn map(t: &Telemetry, mission: Option<&MissionExecutionState>) -> impl Widget {
    let drone = (t.x() as f64, t.y() as f64);
    let yaw = t.yaw() as f64;
    let route = mission.map(|m| waypoints(&m.mission)).unwrap_or_default();
    let current = mission.and_then(current_index);
    // preview the full route only in a take-off-ready state (idle / aborted)
    let preflight = matches!(
        mission.map(|m| &m.mission_status),
        Some(MissionStatus::Idle | MissionStatus::Aborted(_))
    );
    let path = if preflight {
        mission.map(|m| mission_path(&m.mission)).unwrap_or_default()
    } else {
        Vec::new()
    };
    // origin (takeoff) centred; x (forward) up, y (left) left
    let tf = |(x, y): (f64, f64)| (-y, x);
    Canvas::default()
        .block(panel(" MAP "))
        .marker(Marker::Braille)
        .x_bounds([-MAP_M, MAP_M])
        .y_bounds([-MAP_M, MAP_M])
        .paint(move |ctx| {
            // planned route preview (pre-flight only)
            for elem in &path {
                match elem {
                    PathElem::Seg(a, b) => {
                        let (a, b) = (tf(*a), tf(*b));
                        ctx.draw(&CanvasLine { x1: a.0, y1: a.1, x2: b.0, y2: b.1, color: MISSION });
                    }
                    PathElem::Ring(c, r) => {
                        let c = tf(*c);
                        ctx.draw(&Circle { x: c.0, y: c.1, radius: *r, color: MISSION });
                    }
                    PathElem::Rect(bl, tr) => {
                        let corners = [
                            (bl.0, bl.1),
                            (tr.0, bl.1),
                            (tr.0, tr.1),
                            (bl.0, tr.1),
                            (bl.0, bl.1),
                        ];
                        for w in corners.windows(2) {
                            let (a, b) = (tf(w[0]), tf(w[1]));
                            ctx.draw(&CanvasLine {
                                x1: a.0,
                                y1: a.1,
                                x2: b.0,
                                y2: b.1,
                                color: MISSION,
                            });
                        }
                    }
                }
            }
            // waypoint dots; the one being flown to is highlighted bigger + green
            for (i, &point) in route.iter().enumerate() {
                let (px, py) = tf(point);
                let (radius, color) = if Some(i) == current {
                    (0.10, SELECTED)
                } else {
                    (0.06, MISSION)
                };
                ctx.draw(&Circle { x: px, y: py, radius, color });
            }
            // the drone, and a heading line showing which way it faces
            let d = tf(drone);
            ctx.draw(&Circle { x: d.0, y: d.1, radius: 0.13, color: POSITION });
            let rad = yaw.to_radians();
            let nose = tf((drone.0 + 0.45 * rad.cos(), drone.1 + 0.45 * rad.sin()));
            ctx.draw(&CanvasLine {
                x1: d.0,
                y1: d.1,
                x2: nose.0,
                y2: nose.1,
                color: HEADING,
            });
        })
}

/// Fold the command list into drawable route pieces, threading a cursor so relative
/// moves accumulate. Handles every `Command` variant.
fn mission_path(mission: &[Command]) -> Vec<PathElem> {
    let m = |v: &Meters| v.0 as f64;
    let mut cursor = (0.0, 0.0);
    let mut elems = Vec::new();
    for cmd in mission {
        match cmd {
            Command::Move { x, y, .. } => {
                let next = (cursor.0 + m(x), cursor.1 + m(y));
                elems.push(PathElem::Seg(cursor, next));
                cursor = next;
            }
            Command::MoveToWaypoint { x, y, .. } => {
                let next = (m(x), m(y));
                elems.push(PathElem::Seg(cursor, next));
                cursor = next;
            }
            Command::SmoothPath { waypoints, .. } => {
                for w in waypoints {
                    let next = (m(&w.x), m(&w.y));
                    elems.push(PathElem::Seg(cursor, next));
                    cursor = next;
                }
            }
            Command::Setpoints { points } => {
                for p in points {
                    if let Setpoint::PositionPoint { x, y, .. } = p {
                        let next = (m(x), m(y));
                        elems.push(PathElem::Seg(cursor, next));
                        cursor = next;
                    }
                }
            }
            Command::Orbit { radius, .. } => elems.push(PathElem::Ring(cursor, m(radius))),
            Command::BilliardBox(p) => {
                elems.push(PathElem::Rect(
                    (m(&p.bl_x), m(&p.bl_y)),
                    (m(&p.tr_x), m(&p.tr_y)),
                ));
                cursor = (
                    (m(&p.bl_x) + m(&p.tr_x)) / 2.0,
                    (m(&p.bl_y) + m(&p.tr_y)) / 2.0,
                );
            }
            // no horizontal displacement
            Command::Takeoff { .. } | Command::Hover { .. } | Command::Land { .. } => {}
        }
    }
    elems
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
