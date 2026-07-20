use crate::pages::mission_execution::Msg::{
    EmergencyAbort, ExitPage, SafeLand, StartMission, ToggleLinkMode,
};
use Msg::{MissionResult, MissionUpdate};
use crossterm::event::{KeyCode, KeyEvent};
use drone_control::{Abort, Command, CommandUnit, LinkMode, Reason};
use ratatea::Cmd;
use tokio::sync::oneshot;
use tracing::warn;

// model ------------------------------------
#[derive(Debug)]
pub struct Model {
    pub mission: Vec<Command>,
    pub name: String,
    pub abort_sender: Option<oneshot::Sender<Abort>>,
    pub mission_status: drone_control::MissionStatus,
    pub link_mode: LinkMode,
}
impl Model {
    pub fn new(mission: Vec<Command>, name: String) -> Self {
        let mode = LinkMode::default();
        Self {
            mission: mission.into_iter().map(|c| c.to_link_mode(mode)).collect(),
            name,
            abort_sender: None,
            mission_status: drone_control::MissionStatus::Idle,
            link_mode: mode,
        }
    }

    pub fn trajectory_upload_available(&self) -> bool {
        let grounded = self.mission_status == drone_control::MissionStatus::Idle
            || self.mission_status == drone_control::MissionStatus::Aborted(Reason::Landing);
        self.mission.iter().any(Command::has_link_mode) && grounded
    }

    pub fn toggle_link_mode(&mut self) {
        let new_link_mode = match self.link_mode {
            LinkMode::OnVehicle => LinkMode::StreamToVehicle,
            LinkMode::StreamToVehicle => LinkMode::OnVehicle,
        };
        self.mission = self
            .mission
            .clone()
            .into_iter()
            .map(|c| c.to_link_mode(new_link_mode))
            .collect();
        self.link_mode = new_link_mode;
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
    ToggleLinkMode,
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

            Cmd::new(mission, |r| {
                r.unwrap_or_else(|err| warn!("Mission failed with: {err}"));
                MissionResult
            })
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
        ToggleLinkMode if model.trajectory_upload_available() => {
            model.toggle_link_mode();
            Cmd::none()
        }
        ToggleLinkMode => Cmd::none(),
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
    let grounded = s.mission_status == drone_control::MissionStatus::Idle
        || s.mission_status == drone_control::MissionStatus::Aborted(Reason::Landing);

    match k.code {
        KeyCode::Char('l') if k.is_press() => Cmd::pure(SafeLand),
        KeyCode::Char('u') if k.is_press() && grounded => Cmd::pure(ToggleLinkMode),
        KeyCode::Char('x') if k.is_press() => Cmd::pure(EmergencyAbort),
        KeyCode::Char('t') if k.is_press() && grounded => Cmd::pure(StartMission),
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
