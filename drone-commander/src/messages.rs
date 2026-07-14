use crate::model::Movement::{Vx, Vy, YawRate};
use crate::model::{FreeFlightState, Movement};
use crossterm::event::{KeyCode, KeyEventKind};
use drone_control::{Command, MetersPerSecond, Telemetry};
use ratatui::crossterm::event::KeyEvent;

#[derive(Clone, Debug)]
pub enum Msg {
    TelemetryUpdate(Telemetry),
    Key(KeyEvent),
    Resize,
    Quit,
    ToHomeScreen,
    Home(NavigationMessage),
    MissionSelect(MissionSelectMessage),
    MissionExecution(MissionExecutionMessage),
    FreeFlight(FreeFlightMessage),
}

#[derive(Clone, Debug)]
pub enum MissionSelectMessage {
    Nav(NavigationMessage),
    Selected(Vec<Command>, String),
}

#[derive(Clone, Debug)]
pub enum MissionExecutionMessage {
    StartMission,
    MissionResult,
    SafeLand,
    EmergencyAbort,
    MissionUpdate(drone_control::MissionStatus),
}

#[derive(Clone, Debug)]
pub enum FreeFlightMessage {
    Move(Movement),
    Abort,
    SendNextMove,
    CommandSet,
    TakeOffDone,
}

impl Movement {
    pub fn from_key_evt(k: KeyEvent, s: &FreeFlightState) -> Option<Self> {
        let one_ms = MetersPerSecond(1.0);
        let zero_ms = MetersPerSecond(0.0);
        let yaw_rate = 150.0;
        match (k.code, k.kind) {
            (KeyCode::Char('w'), KeyEventKind::Press) if s.vx <= zero_ms => Some(Vx(one_ms)),
            (KeyCode::Char('w'), KeyEventKind::Release) => Some(Vx(zero_ms)),
            (KeyCode::Char('a'), KeyEventKind::Press) if s.vy <= zero_ms => Some(Vy(one_ms)),
            (KeyCode::Char('a'), KeyEventKind::Release) => Some(Vy(zero_ms)),
            (KeyCode::Char('s'), KeyEventKind::Press) if s.vx >= zero_ms => Some(Vx(-one_ms)),
            (KeyCode::Char('s'), KeyEventKind::Release) => Some(Vx(zero_ms)),
            (KeyCode::Char('d'), KeyEventKind::Press) if s.vy >= zero_ms => Some(Vy(-one_ms)),
            (KeyCode::Char('d'), KeyEventKind::Release) => Some(Vy(zero_ms)),
            (KeyCode::Left, KeyEventKind::Press) => Some(YawRate(yaw_rate)),
            (KeyCode::Right, KeyEventKind::Press) => Some(YawRate(-yaw_rate)),
            (KeyCode::Left, KeyEventKind::Release) => Some(YawRate(0.0)),
            (KeyCode::Right, KeyEventKind::Release) => Some(YawRate(0.0)),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum NavigationMessage {
    Up,
    Down,
    Select,
}
