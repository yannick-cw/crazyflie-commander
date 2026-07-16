use crate::model::Movement;
use drone_control::{Command, Telemetry};
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
    MissionsLoaded(Vec<(String, Vec<Command>)>, Vec<(String, Vec<Command>)>),
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
    StartRecording,
    StopRecording,
}

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum NavigationMessage {
    Up,
    Down,
    Select,
}
