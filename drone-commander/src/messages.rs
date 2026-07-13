use drone_control::{Command, Telemetry};
use ratatui::crossterm::event::KeyEvent;

#[derive(Clone, Debug)]
pub enum Msg {
    TelemetryUpdate(Telemetry),
    /// Key press.
    Key(KeyEvent),
    Resize,
    Quit,
    ToHomeScreen,
    Home(NavigationMessage),
    MissionSelect(MissionSelectMessage),
    MissionExecution(MissionExecutionMessage),
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

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum NavigationMessage {
    Up,
    Down,
    Select,
}
