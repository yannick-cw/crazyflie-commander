use crate::Autopilot;
use crate::control::vehicle_control::VehicleState::{
    ExecutingMission, HardStopped, Idle, Landing, LowBatteryStopped,
};
use crate::errors::MissionError::StateError;
use crate::errors::Res;
use anyhow::Context;
use datalink::domain_types;
use datalink::domain_types::{Abort, Progress, Reason, TrajectoryId, VehicleStatus};
use std::fmt::{Debug, Formatter};
use std::time::Duration;
use tokio::select;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tracing::error;

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Progress(Progress),
    LowBatLanding,
}

enum LandingTrigger {
    ManualLanding,
    LowBatLanding,
}
enum VehicleState<A: Autopilot> {
    Idle(A),
    Landing(JoinHandle<A>, LandingTrigger),
    ExecutingMission(oneshot::Sender<Abort>, JoinHandle<A>, Option<Progress>),
    HardStopped,
    LowBatteryStopped,
}
impl<A: Autopilot> Debug for VehicleState<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Idle(_) => write!(f, "Idle"),
            Landing(_, _) => write!(f, "Landing"),
            ExecutingMission(_, _, _) => write!(f, "ExecutingMissions"),
            HardStopped => write!(f, "HardStopped"),
            LowBatteryStopped => write!(f, "LowBatteryStopped"),
        }
    }
}
impl<A: Autopilot> VehicleState<A> {
    fn as_vehicle_status(&self) -> VehicleStatus {
        match self {
            Idle(_) => VehicleStatus::Idle,
            Landing(_, _) => VehicleStatus::Landing,
            ExecutingMission(_, _, p) => VehicleStatus::MissionRunning(p.clone()),
            HardStopped => VehicleStatus::Aborted(Reason::HardStop),
            LowBatteryStopped => VehicleStatus::Aborted(Reason::LowBattery),
        }
    }
}

#[derive(Debug)]
enum Message {
    RunMission(Vec<domain_types::MissionItem>, oneshot::Sender<Res<()>>),
    AbortCommand(Abort, oneshot::Sender<Res<()>>),
    MissionFinished,
    UploadMissionItem(
        domain_types::MissionItem,
        oneshot::Sender<Res<Option<(TrajectoryId, Duration)>>>,
    ),
}

#[derive(Debug, Clone)]
pub struct VehicleHandle {
    command_sender: mpsc::Sender<Message>,
}

impl VehicleHandle {
    pub async fn submit_mission(&self, m: Vec<domain_types::MissionItem>) -> Res<()> {
        let (reply, rx) = oneshot::channel();
        self.command_sender
            .send(Message::RunMission(m, reply))
            .await
            .context("could not send!")?;

        rx.await.context("could not receive")?
    }
    pub async fn upload_mission_item(
        &self,
        m: domain_types::MissionItem,
    ) -> Res<Option<(TrajectoryId, Duration)>> {
        let (reply, rx) = oneshot::channel();
        self.command_sender
            .send(Message::UploadMissionItem(m, reply))
            .await
            .context("could not send!")?;

        rx.await.context("could not receive")?
    }
    pub async fn abort_mission(&self, abort_signal: Abort) -> Res<()> {
        let (reply, rx) = oneshot::channel();
        self.command_sender
            .send(Message::AbortCommand(abort_signal, reply))
            .await
            .context("could not send!")?;

        rx.await.context("could not receive")?
    }
}

pub struct VehicleController<A: Autopilot> {
    state: VehicleState<A>,
    command_receiver: mpsc::Receiver<Message>,
    command_sender: mpsc::Sender<Message>,
    status_updates: watch::Sender<VehicleStatus>,
    status_reports: mpsc::Receiver<ProgressEvent>,
}

impl<A: Autopilot + Send + Sync + 'static> VehicleController<A> {
    async fn handle_msg(
        cmd: Message,
        state: VehicleState<A>,
        cmd_sender: mpsc::Sender<Message>,
    ) -> VehicleState<A> {
        match (cmd, state) {
            (Message::RunMission(mission, reply), Idle(mut autopilot)) => {
                let (abort_sender, abort_rcv) = oneshot::channel();

                let handle = tokio::spawn(async move {
                    let _ = autopilot
                        .run_mission(mission, async { abort_rcv.await.ok() })
                        .await
                        .inspect_err(|err| error!("Failed mission execution: {:?}", err));
                    let _ = cmd_sender.send(Message::MissionFinished).await;
                    autopilot
                });
                let _ = reply.send(Ok(()));
                ExecutingMission(abort_sender, handle, None)
            }
            (Message::AbortCommand(abort, reply), ExecutingMission(abort_handler, h, _)) => {
                let new_state = match abort {
                    Abort::FlightTermination => HardStopped,
                    Abort::Land => Landing(h, LandingTrigger::ManualLanding),
                };
                let _ = abort_handler.send(abort);
                let _ = reply.send(Ok(()));
                new_state
            }
            (Message::MissionFinished, ExecutingMission(_, handle, _)) => {
                let autopilot = handle.await.expect("should not fail");
                Idle(autopilot)
            }
            (Message::MissionFinished, Landing(handle, LandingTrigger::ManualLanding)) => {
                let autopilot = handle.await.expect("should not fail");
                Idle(autopilot)
            }
            (Message::MissionFinished, Landing(_, LandingTrigger::LowBatLanding)) => {
                LowBatteryStopped
            }
            (Message::MissionFinished, state) => state,
            (Message::UploadMissionItem(item, reply), Idle(mut autopilot)) => {
                let res = autopilot.upload_command(item).await;
                let _ = reply.send(res);
                Idle(autopilot)
            }
            (Message::RunMission(_, reply), s) => {
                let _ = reply.send(Err(StateError(format!(
                    "Can't start mission when in {:?} state",
                    s
                ))));
                s
            }
            (Message::UploadMissionItem(_, reply), s) => {
                let _ = reply.send(Err(StateError(format!(
                    "Can't upload mission when in {:?} state",
                    s
                ))));
                s
            }
            (Message::AbortCommand(_, reply), s) => {
                let _ = reply.send(Err(StateError(format!(
                    "Can't abort mission when in {:?} state",
                    s
                ))));
                s
            }
        }
    }

    fn handle_progress_update(update: ProgressEvent, state: VehicleState<A>) -> VehicleState<A> {
        match (update, state) {
            (ProgressEvent::LowBatLanding, ExecutingMission(_, h, _) | Landing(h, _)) => {
                Landing(h, LandingTrigger::LowBatLanding)
            }
            (ProgressEvent::LowBatLanding, state) => state,
            (ProgressEvent::Progress(progress), ExecutingMission(a, h, _)) => {
                ExecutingMission(a, h, Some(progress))
            }
            (ProgressEvent::Progress(_), state) => state,
        }
    }

    pub async fn run(self) {
        let mut receiver = self.command_receiver;
        let mut current_state = self.state;
        let command_sender = self.command_sender;
        let mut progress_update = self.status_reports;

        loop {
            select! {
                Some(progress_update) = progress_update.recv() => {
                    let new_state = Self::handle_progress_update(progress_update, current_state);
                    let _ = self.status_updates.send(new_state.as_vehicle_status());
                    current_state = new_state
                },
                Some(cmd) = receiver.recv() => {
                    let new_state = Self::handle_msg(cmd, current_state, command_sender.clone()).await;
                    let _ = self.status_updates.send(new_state.as_vehicle_status());
                    current_state = new_state
                }
            }
        }
    }
}

pub fn init_vehicle_control<A: Autopilot>(
    autopilot: A,
    status_updates: watch::Sender<VehicleStatus>,
    status_reports: mpsc::Receiver<ProgressEvent>,
) -> (VehicleController<A>, VehicleHandle) {
    let (command_sender, command_receiver) = mpsc::channel(64);
    (
        VehicleController {
            state: Idle(autopilot),
            command_receiver,
            command_sender: command_sender.clone(),
            status_updates,
            status_reports,
        },
        VehicleHandle { command_sender },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Autopilot, ManualControl};
    use datalink::domain_types::{FlightMode, Meters, MetersPerSecond, MissionItem, Waypoint};
    use futures::Stream;
    use std::assert_matches;
    use tokio::time::timeout;

    struct TestPilot {
        step_mission: mpsc::Receiver<()>,
    }

    impl Autopilot for TestPilot {
        async fn run_mission(
            &mut self,
            _mission: Vec<MissionItem>,
            _abort_signal: impl Future<Output = Option<Abort>> + Send,
        ) -> Res<()> {
            self.step_mission.recv().await;
            Ok(())
        }

        async fn upload_orbit(
            &mut self,
            _radius: Meters,
            _orbital_period: Duration,
            _orbits: usize,
            _z: Meters,
        ) -> Res<(TrajectoryId, Duration)> {
            todo!()
        }
        async fn upload_smooth_path(
            &mut self,
            _waypoints: Vec<Waypoint>,
            _speed: MetersPerSecond,
            _flight_mode: FlightMode,
        ) -> Res<(TrajectoryId, Duration)> {
            todo!()
        }
        fn fly(
            &mut self,
            _commands: impl Stream<Item = ManualControl> + Send,
        ) -> impl Future<Output = Res<()>> + Send {
            async { Ok(()) }
        }
    }

    fn test_setup() -> (
        watch::Receiver<VehicleStatus>,
        VehicleHandle,
        mpsc::Sender<()>,
        mpsc::Sender<ProgressEvent>,
    ) {
        let (send, rec) = watch::channel(VehicleStatus::Idle);
        let (send_prg, progress) = mpsc::channel(64);

        let (step_sender, step_receiver) = mpsc::channel(64);
        let (controller, handle) = init_vehicle_control(
            TestPilot {
                step_mission: step_receiver,
            },
            send,
            progress,
        );

        tokio::spawn(controller.run());

        (rec, handle, step_sender, send_prg)
    }

    fn mission() -> Vec<MissionItem> {
        vec![MissionItem::Takeoff {
            height: Default::default(),
            duration: Default::default(),
        }]
    }
    async fn assert_status(rec: &mut watch::Receiver<VehicleStatus>, expected: VehicleStatus) {
        let status = timeout(Duration::from_millis(100), rec.wait_for(|s| *s == expected))
            .await
            .expect("status never reached")
            .expect("rec error");
        assert_eq!(*status, expected);
    }
    #[tokio::test]
    async fn transition_through_mission() {
        let (mut rec, handle, step_sender, _) = test_setup();
        assert_status(&mut rec, VehicleStatus::Idle).await;

        handle.submit_mission(mission()).await.expect("runs fine");

        assert_status(&mut rec, VehicleStatus::MissionRunning(None)).await;

        // finish mission
        let _ = step_sender.send(()).await;

        assert_status(&mut rec, VehicleStatus::Idle).await;

        // start another mission
        handle.submit_mission(mission()).await.expect("runs fine");

        assert_status(&mut rec, VehicleStatus::MissionRunning(None)).await;

        // finish mission
        let _ = step_sender.send(()).await;

        assert_status(&mut rec, VehicleStatus::Idle).await;
    }

    #[tokio::test]
    async fn cant_start_mission_after_abort() {
        let (mut rec, handle, _step_sender, _) = test_setup();
        assert_status(&mut rec, VehicleStatus::Idle).await;

        handle.submit_mission(mission()).await.expect("runs fine");

        assert_status(&mut rec, VehicleStatus::MissionRunning(None)).await;

        handle
            .abort_mission(Abort::FlightTermination)
            .await
            .expect("aborted");

        assert_status(&mut rec, VehicleStatus::Aborted(Reason::HardStop)).await;

        let mission_start_res = handle.submit_mission(mission()).await;
        assert_matches!(mission_start_res, Err(_));
    }

    #[tokio::test]
    async fn cant_start_mission_after_low_bat_landing() {
        let (mut rec, handle, step_sender, prg_sender) = test_setup();
        assert_status(&mut rec, VehicleStatus::Idle).await;

        handle.submit_mission(mission()).await.expect("runs fine");

        assert_status(&mut rec, VehicleStatus::MissionRunning(None)).await;

        prg_sender
            .send(ProgressEvent::LowBatLanding)
            .await
            .expect("sending");

        assert_status(&mut rec, VehicleStatus::Landing).await;

        // finish landing
        let _ = step_sender.send(()).await;

        assert_status(&mut rec, VehicleStatus::Aborted(Reason::LowBattery)).await;

        let mission_start_res = handle.submit_mission(mission()).await;
        assert_matches!(mission_start_res, Err(_));
    }

    #[tokio::test]
    async fn can_start_mission_after_manual_landing() {
        let (mut rec, handle, step_sender, _) = test_setup();
        assert_status(&mut rec, VehicleStatus::Idle).await;

        handle.submit_mission(mission()).await.expect("runs fine");

        assert_status(&mut rec, VehicleStatus::MissionRunning(None)).await;

        handle.abort_mission(Abort::Land).await.expect("aborted");

        assert_status(&mut rec, VehicleStatus::Landing).await;

        // finish landing
        let _ = step_sender.send(()).await;

        assert_status(&mut rec, VehicleStatus::Idle).await;
    }
}
