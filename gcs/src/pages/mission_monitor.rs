use crate::pages::mission_monitor::Msg::{
    EmergencyAbort, ExitPage, SafeLand, StartMission, ToggleLinkMode,
};
use Msg::{MissionResult, MissionUpdate};
use crossterm::event::{KeyCode, KeyEvent};
use futures::{TryFutureExt, TryStreamExt, stream};
use mission_computer::errors::{MissionError, Res};
use mission_computer::{Abort, Autopilot, MissionItem, Reason};
use ratatea::Cmd;
use tokio::sync::oneshot;
use tokio_stream::StreamExt;
use tracing::warn;

// model ------------------------------------
#[derive(Debug)]
pub struct Model {
    pub mission: Vec<MissionItem>,
    pub name: String,
    pub abort_sender: Option<oneshot::Sender<Abort>>,
    pub mission_status: mission_computer::MissionStatus,
    pub link_mode: ExecutionMode,
}
impl Model {
    pub fn new(mission: Vec<MissionItem>, name: String) -> Self {
        Self {
            mission,
            name,
            abort_sender: None,
            mission_status: mission_computer::MissionStatus::Idle,
            link_mode: ExecutionMode::Online,
        }
    }

    pub fn trajectory_upload_available(&self) -> bool {
        let grounded = self.mission_status == mission_computer::MissionStatus::Idle
            || self.mission_status == mission_computer::MissionStatus::Aborted(Reason::Landing);
        self.mission.iter().any(MissionItem::can_upload_trajectory) && grounded
    }

    pub fn convert_to_online_missions(&mut self) {
        self.mission = self
            .mission
            .iter()
            .map(|m| match m {
                MissionItem::OnVehicleTrajectory {
                    original_command, ..
                } => original_command,
                c => c,
            })
            .cloned()
            .collect();
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ExecutionMode {
    Offline,
    Online,
    FailedUpload,
}

// msg ------------------------------------
#[derive(Debug)]
pub enum Msg {
    StartMission,
    MissionResult,
    SafeLand,
    EmergencyAbort,
    MissionUpdate(mission_computer::MissionStatus),
    ExitPage,
    ToggleLinkMode,
    MissionUploaded(Res<Vec<MissionItem>>),
}

// update ------------------------------------

pub fn update(command_unit: &'static impl Autopilot, model: &mut Model, msg: Msg) -> Cmd<Msg> {
    match msg {
        StartMission => {
            let mission = model.mission.clone();
            let (sender, receiver) = oneshot::channel();
            let h = tokio::spawn(
                command_unit.run_mission(mission, async move { Some(receiver.await.unwrap()) }),
            );
            model.abort_sender = Some(sender);

            Cmd::new(h, |r| {
                r.unwrap()
                    .unwrap_or_else(|err| warn!("Mission failed with: {err}"));
                MissionResult
            })
        }
        MissionResult => Cmd::none(),
        SafeLand => abort_mission(model, Abort::Land),
        EmergencyAbort => abort_mission(model, Abort::FlightTermination),
        MissionUpdate(update) => {
            model.mission_status = update;
            Cmd::none()
        }
        // exit events handled by parent
        ExitPage => Cmd::none(),
        ToggleLinkMode
            if model.trajectory_upload_available()
                && matches!(
                    model.link_mode,
                    ExecutionMode::Offline | ExecutionMode::FailedUpload
                ) =>
        {
            model.link_mode = ExecutionMode::Online;
            model.convert_to_online_missions();
            Cmd::none()
        }
        ToggleLinkMode
            if model.trajectory_upload_available()
                && matches!(model.link_mode, ExecutionMode::Online) =>
        {
            Cmd::new(
                upload_mission(command_unit, model.mission.clone()),
                Msg::MissionUploaded,
            )
        }
        ToggleLinkMode => Cmd::none(),
        Msg::MissionUploaded(Ok(m)) => {
            model.link_mode = ExecutionMode::Offline;
            model.mission = m;
            Cmd::none()
        }
        Msg::MissionUploaded(Err(err)) => {
            model.link_mode = ExecutionMode::FailedUpload;
            warn!("Mission upload failed with {err}");
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
            Cmd::new(signal, |_| MissionResult)
        }
    }
}

pub fn map_key_evt(k: KeyEvent, s: &Model) -> Cmd<Msg> {
    let grounded = s.mission_status == mission_computer::MissionStatus::Idle
        || s.mission_status == mission_computer::MissionStatus::Aborted(Reason::Landing);

    match k.code {
        KeyCode::Char('l') if k.is_press() => Cmd::pure(SafeLand),
        KeyCode::Char('u') if k.is_press() && grounded => Cmd::pure(ToggleLinkMode),
        KeyCode::Char('x') if k.is_press() => Cmd::pure(EmergencyAbort),
        KeyCode::Char('t') if k.is_press() && grounded => Cmd::pure(StartMission),
        KeyCode::Char('b')
            if k.is_press()
                && matches!(
                    s.mission_status,
                    mission_computer::MissionStatus::Idle
                        | mission_computer::MissionStatus::Aborted(_)
                ) =>
        {
            Cmd::pure(ExitPage)
        }
        _ => Cmd::none(),
    }
}

async fn upload_mission(
    command_unit: &impl Autopilot,
    mission: Vec<MissionItem>,
) -> Result<Vec<MissionItem>, MissionError> {
    stream::iter(mission)
        .then(|c| {
            command_unit
                .upload_command(c.clone())
                .map_ok(|res| match res {
                    None => c,
                    Some((id, duration)) => MissionItem::OnVehicleTrajectory {
                        id,
                        duration,
                        original_command: Box::new(c),
                    },
                })
        })
        .try_collect::<Vec<_>>()
        .await
}
