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
            state: Home(HomeState {
                selected_mode: ModeSelection::MissionSelectItem,
            }),
        }
    }
}
#[derive(Debug, Clone)]
pub enum State {
    Home(HomeState),
    MissionExecution(MissionPlan),
    // this opens a selection view
    MissionSelect(MissionSelectState),
    MissionPlan(),
    // this will go to "current" observe only for now
    FreeFlight(),
}

#[derive(Clone, Debug)]
pub struct MissionPlan {
    pub mission: Vec<Command>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct HomeState {
    pub selected_mode: ModeSelection,
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
    pub missions: Vec<(String, Vec<Command>)>,
    pub selection: usize,
}
impl Default for MissionSelectState {
    fn default() -> Self {
        MissionSelectState {
            missions: vec![
                ("nikolaus".to_string(), haus_nikolaus()),
                ("orbit".to_string(), orbit()),
                ("smooth".to_string(), smooth_curves()),
                ("body smooth".to_string(), body_frame_smooth()),
                ("lawn mower".to_string(), lawn_mower()),
            ],
            selection: 0,
        }
    }
}
