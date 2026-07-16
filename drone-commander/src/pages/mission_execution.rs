use drone_control::{Abort, Command, CommandUnit};
use ratatea::Cmd;
use tokio::sync::oneshot;

// model ------------------------------------
#[derive(Debug)]
pub struct Model {
    pub mission: Vec<Command>,
    pub name: String,
    pub abort_sender: Option<oneshot::Sender<Abort>>,
    pub mission_status: drone_control::MissionStatus,
}
impl Model {
    pub fn new(mission: Vec<Command>, name: String) -> Self {
        Self {
            mission,
            name,
            abort_sender: None,
            mission_status: drone_control::MissionStatus::Idle,
        }
    }
}

// msg ------------------------------------
#[derive(Clone, Debug)]
pub enum Msg {
    StartMission,
    MissionResult,
    SafeLand,
    EmergencyAbort,
    MissionUpdate(drone_control::MissionStatus),
}

// update ------------------------------------

pub fn update(command_unit: &'static impl CommandUnit, model: &mut Model, msg: Msg) -> Cmd<Msg> {
    match msg {
        Msg::StartMission => {
            let mission = model.mission.clone();
            let (sender, receiver) = oneshot::channel();
            let mission =
                command_unit.run_mission(mission, async move { Some(receiver.await.unwrap()) });
            model.abort_sender = Some(sender);

            Cmd::new(mission, |_| Msg::MissionResult)
        }
        Msg::MissionResult => Cmd::none(),
        Msg::SafeLand => abort_mission(model, Abort::Land),
        Msg::EmergencyAbort => abort_mission(model, Abort::HardStop),
        Msg::MissionUpdate(update) => {
            model.mission_status = update;
            Cmd::none()
        }
    }
}

// util ------------------------------------------
fn abort_mission(model: &mut Model, signal: Abort) -> Cmd<Msg> {
    match model.abort_sender.take() {
        None => Cmd::none(),
        Some(s) => {
            let signal = async move { s.send(signal) };
            Cmd::new(signal, |_| Msg::MissionResult)
        }
    }
}
