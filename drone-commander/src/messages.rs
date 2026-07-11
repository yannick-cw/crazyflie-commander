use crate::model::MissionPlan;
use drone_control::Telemetry;
use ratatui::crossterm::event::KeyEvent;

#[derive(Clone, Debug)]
pub enum Msg {
    TelemetryUpdate(Telemetry),
    /// Key press.
    Key(KeyEvent),
    Quit,
    Home(NavigationMessage),
    MissionSelect(MissionSelectMessage),
    MissionExecution(),
}

#[derive(Clone, Debug)]
pub enum MissionSelectMessage {
    Nav(NavigationMessage),
    Selected(MissionPlan),
}

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum NavigationMessage {
    Up,
    Down,
    Select,
}
