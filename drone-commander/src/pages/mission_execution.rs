use crate::pages::mission_execution::Msg::{EmergencyAbort, ExitPage, SafeLand, StartMission};
use Msg::{MissionResult, MissionUpdate};
use crossterm::event::{KeyCode, KeyEvent};
use drone_control::{Abort, Command, CommandUnit, Reason};
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
    ExitPage,
}

// update ------------------------------------

pub fn update(command_unit: &'static impl CommandUnit, model: &mut Model, msg: Msg) -> Cmd<Msg> {
    match msg {
        StartMission => {
            let mission = model.mission.clone();
            let (sender, receiver) = oneshot::channel();
            let mission =
                command_unit.run_mission(mission, async move { Some(receiver.await.unwrap()) });
            model.abort_sender = Some(sender);

            Cmd::new(mission, |_| MissionResult)
        }
        MissionResult => Cmd::none(),
        SafeLand => abort_mission(model, Abort::Land),
        EmergencyAbort => abort_mission(model, Abort::HardStop),
        MissionUpdate(update) => {
            model.mission_status = update;
            Cmd::none()
        }
        // exit events handled by parent
        ExitPage => Cmd::none(),
    }
}

// util ------------------------------------------
fn abort_mission(model: &mut Model, signal: Abort) -> Cmd<Msg> {
    match model.abort_sender.take() {
        None => Cmd::none(),
        Some(s) => {
            let signal = async move { s.send(signal) };
            Cmd::new(signal, |_| MissionResult)
        }
    }
}

pub fn map_key_evt(k: KeyEvent, s: &Model) -> Cmd<Msg> {
    match k.code {
        KeyCode::Char('l') if k.is_press() => Cmd::pure(SafeLand),
        KeyCode::Char('x') if k.is_press() => Cmd::pure(EmergencyAbort),
        KeyCode::Char('t')
            if k.is_press()
                && (s.mission_status == drone_control::MissionStatus::Idle
                    || s.mission_status
                        == drone_control::MissionStatus::Aborted(Reason::Landing)) =>
        {
            Cmd::pure(StartMission)
        }
        KeyCode::Char('b')
            if k.is_press()
                && matches!(
                    s.mission_status,
                    drone_control::MissionStatus::Idle | drone_control::MissionStatus::Aborted(_)
                ) =>
        {
            Cmd::pure(ExitPage)
        }
        _ => Cmd::none(),
    }
}
