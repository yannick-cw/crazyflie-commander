use crate::model::MissionExecutionState;
use drone_control::Telemetry;
use ratatui::crossterm::event::KeyEvent;

#[derive(Clone, Debug)]
pub enum Msg {
    TelemetryUpdate(Telemetry),
    /// Key press.
    Key(KeyEvent),
    Resize,
    Quit,
    Home(NavigationMessage),
    MissionSelect(MissionSelectMessage),
    MissionExecution(MissionExecutionMessage),
}

#[derive(Clone, Debug)]
pub enum MissionSelectMessage {
    Nav(NavigationMessage),
    Selected(MissionExecutionState),
}

#[derive(Clone, Debug)]
pub enum MissionExecutionMessage {
    StartMission,
    MissionResult,
}

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum NavigationMessage {
    Up,
    Down,
    Select,
}
