use crate::model::State::Home;
use drone_control::{Abort, Command, Meters, MetersPerSecond, MotionCommand, Telemetry};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
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
#[derive(Debug)]
pub enum State {
    Home(HomeState),
    MissionExecution(MissionExecutionState),
    // this opens a selection view
    MissionSelect(MissionSelectState),
    MissionPlan(),
    // this will go to "current" observe only for now
    FreeFlight(FreeFlightState),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Movement {
    Vx(MetersPerSecond),
    Vy(MetersPerSecond),
    YawRate(f32),
    Land,
    GoHome,
    Start,
    SpeedUp,
    SpeedDown,
}

#[derive(Debug)]
pub struct FreeFlightState {
    pub vx: MetersPerSecond,
    pub vy: MetersPerSecond,
    pub yaw_rate: f32,
    pub z: Meters,
    pub motion_sender: mpsc::UnboundedSender<MotionCommand>,
    pub is_airborne: bool,
    pub speed_setting: MetersPerSecond,
    pub yaw_rate_setting: f32,
}

#[derive(Debug)]
pub struct MissionExecutionState {
    pub mission: Vec<Command>,
    pub name: String,
    pub abort_sender: Option<oneshot::Sender<Abort>>,
    pub mission_status: drone_control::MissionStatus,
}

#[derive(Debug)]
pub struct HomeState {
    pub selected_mode: ModeSelection,
}

#[derive(Debug, PartialEq)]
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

#[derive(Debug)]
pub struct MissionSelectState {
    pub missions: Vec<(String, Vec<Command>)>,
    pub selection: usize,
}
impl Default for MissionSelectState {
    fn default() -> Self {
        MissionSelectState {
            missions: Vec::new(),
            selection: 0,
        }
    }
}
