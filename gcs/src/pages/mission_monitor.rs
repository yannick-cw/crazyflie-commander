use crate::ground_data_link::vehicle_link::VehicleLink;
use crate::pages::mission_monitor::Msg::{
    EmergencyAbort, ExitPage, SafeLand, StartMission, ToggleLinkMode,
};
use Msg::{MissionResult, MissionUpdate};
use crossterm::event::{KeyCode, KeyEvent};
use datalink::domain_types::{Abort, MissionItem, VehicleStatus};
use futures::{TryFutureExt, TryStreamExt};
use mission_computer::errors::{MissionError, Res};
use ratatea::Cmd;
use std::rc::Rc;
use tokio_stream::StreamExt;
use tracing::{error, warn};

// model ------------------------------------
#[derive(Debug)]
pub struct Model {
    pub mission: Vec<MissionItem>,
    pub name: String,
    pub mission_status: VehicleStatus,
    pub link_mode: ExecutionMode,
}
impl Model {
    pub fn new(mission: Vec<MissionItem>, name: String) -> Self {
        Self {
            mission,
            name,
            mission_status: VehicleStatus::Idle,
            link_mode: ExecutionMode::Online,
        }
    }

    pub fn trajectory_upload_available(&self) -> bool {
        let grounded = self.mission_status == VehicleStatus::Idle;
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
    MissionResult(Res<()>),
    SafeLand,
    EmergencyAbort,
    MissionUpdate(VehicleStatus),
    ExitPage,
    ToggleLinkMode,
    MissionUploaded(Res<Vec<MissionItem>>),
}

// update ------------------------------------

pub fn update(vehicle_link: Rc<VehicleLink>, model: &mut Model, msg: Msg) -> Cmd<Msg> {
    match msg {
        StartMission => {
            let mission = model.mission.clone();
            Cmd::new(
                async move { vehicle_link.submit_mission(mission).await },
                MissionResult,
            )
        }
        MissionResult(Ok(_)) => Cmd::none(),
        MissionResult(Err(err)) => {
            error!("failed with {:?}", err);
            Cmd::none()
        }
        SafeLand => abort_mission(vehicle_link, Abort::Land),
        EmergencyAbort => abort_mission(vehicle_link, Abort::FlightTermination),
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
                upload_mission(vehicle_link, model.mission.clone()),
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
fn abort_mission(link: Rc<VehicleLink>, signal: Abort) -> Cmd<Msg> {
    Cmd::new(
        async move { link.clone().abort_mission(signal).await },
        MissionResult,
    )
}

pub fn map_key_evt(k: KeyEvent, s: &Model) -> Cmd<Msg> {
    let grounded = s.mission_status == VehicleStatus::Idle;

    match k.code {
        KeyCode::Char('l') if k.is_press() => Cmd::pure(SafeLand),
        KeyCode::Char('u') if k.is_press() && grounded => Cmd::pure(ToggleLinkMode),
        KeyCode::Char('x') if k.is_press() => Cmd::pure(EmergencyAbort),
        KeyCode::Char('t') if k.is_press() && grounded => Cmd::pure(StartMission),
        KeyCode::Char('b')
            if k.is_press()
                && matches!(
                    s.mission_status,
                    VehicleStatus::Idle | VehicleStatus::Aborted(_)
                ) =>
        {
            Cmd::pure(ExitPage)
        }
        _ => Cmd::none(),
    }
}

async fn upload_mission(
    vehicle_link: Rc<VehicleLink>,
    mission: Vec<MissionItem>,
) -> Result<Vec<MissionItem>, MissionError> {
    tokio_stream::iter(mission)
        .then(|c| {
            vehicle_link
                .upload_mission(c.clone())
                .map_ok(|res| match res {
                    None => c,
                    Some((id, duration)) => MissionItem::OnVehicleTrajectory {
                        id,
                        duration: duration.into(),
                        original_command: Box::new(c),
                    },
                })
        })
        .try_collect::<Vec<_>>()
        .await
}
