use crate::model::HomeState::Overview;
use crate::model::ModeSelection::MissionSelectItem;
use crate::model::State::Home;
use drone_control::flight_paths::{
    body_frame_smooth, haus_nikolaus, lawn_mower, orbit, smooth_curves,
};
use drone_control::{Command, Telemetry};

#[derive(Debug, Clone)]
pub struct Model {
    pub telemetry: Telemetry,
    pub exit: bool,
    pub state: State,
}
impl Default for Model {
    fn default() -> Self {
        Model {
            telemetry: Default::default(),
            exit: false,
            state: Home(Overview(MissionSelectItem)),
        }
    }
}
#[derive(Debug, Clone)]
pub enum State {
    Home(HomeState),
    MissionExecution(Vec<Command>),
}

#[derive(Debug, Clone)]
pub enum HomeState {
    Overview(ModeSelection),
    // this opens a selection view
    MissionSelect(MissionSelectState),
    MissionPlan(),
    // this will go to "current" observe only for now
    FreeFlight(),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ModeSelection {
    MissionSelectItem,
    MissionPlanItem,
    FreeFlightItem,
}

impl ModeSelection {
    pub fn next(&self) -> Self {
        match self {
            ModeSelection::MissionSelectItem => ModeSelection::MissionPlanItem,
            ModeSelection::MissionPlanItem => ModeSelection::FreeFlightItem,
            ModeSelection::FreeFlightItem => ModeSelection::MissionSelectItem,
        }
    }
    pub fn prev(&self) -> Self {
        match self {
            ModeSelection::MissionSelectItem => ModeSelection::FreeFlightItem,
            ModeSelection::MissionPlanItem => ModeSelection::MissionSelectItem,
            ModeSelection::FreeFlightItem => ModeSelection::MissionPlanItem,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MissionSelectState {
    _missions: Vec<(String, Vec<Command>)>,
    _selection: i8,
}
impl Default for MissionSelectState {
    fn default() -> Self {
        MissionSelectState {
            _missions: vec![
                ("nikolaus".to_string(), haus_nikolaus()),
                ("orbit".to_string(), orbit()),
                ("smooth".to_string(), smooth_curves()),
                ("body smooth".to_string(), body_frame_smooth()),
                ("lawn mower".to_string(), lawn_mower()),
            ],
            _selection: 0,
        }
    }
}
