use crate::Autopilot;
use crate::control::autopilot::ProgressEvent;
use crate::control::vehicle_control::VehicleState::{
    ExecutingMission, HardStopped, Idle, Landing, LowBatteryStopped,
};
use crate::errors::MissionError::StateError;
use crate::errors::Res;
use anyhow::Context;
use datalink::domain_types;
use datalink::domain_types::{Abort, Progress, Reason, TrajectoryId, VehicleStatus};
use futures::FutureExt;
use futures::StreamExt;
use std::fmt::{Debug, Formatter};
use std::time::Duration;
use tokio::select;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tracing::error;

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
    progress_sender: mpsc::Sender<ProgressEvent>,
    progress_receiver: mpsc::Receiver<ProgressEvent>,
    status_updates: watch::Sender<VehicleStatus>,
}

impl<A: Autopilot + Send + 'static> VehicleController<A> {
    async fn handle_msg(
        cmd: Message,
        state: VehicleState<A>,
        progress_sender: mpsc::Sender<ProgressEvent>,
    ) -> VehicleState<A> {
        match (cmd, state) {
            (Message::RunMission(mission, reply), Idle(mut autopilot)) => {
                let (abort_sender, abort_rcv) = oneshot::channel();

                let handle = tokio::spawn(async move {
                    autopilot
                        .run_mission(mission, async { abort_rcv.await.ok() })
                        .for_each(|p_evt| progress_sender.send(p_evt).map(|_| ()))
                        .await;
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
            (Message::UploadMissionItem(item, reply), Idle(mut autopilot)) => {
                let res = autopilot.upload_command(item).await;
                let _ = reply.send(res);
                Idle(autopilot)
            }
            (Message::RunMission(_, reply), state) => {
                let _ = reply.send(Err(StateError(format!(
                    "Can't start mission when in {:?} state",
                    state
                ))));
                state
            }
            (Message::UploadMissionItem(_, reply), state) => {
                let _ = reply.send(Err(StateError(format!(
                    "Can't upload mission when in {:?} state",
                    state
                ))));
                state
            }
            (Message::AbortCommand(_, reply), state) => {
                let _ = reply.send(Err(StateError(format!(
                    "Can't abort mission when in {:?} state",
                    state
                ))));
                state
            }
        }
    }

    async fn handle_progress_update(
        update: ProgressEvent,
        state: VehicleState<A>,
    ) -> VehicleState<A> {
        match (update, state) {
            (ProgressEvent::LowBatLanding, ExecutingMission(_, h, _) | Landing(h, _)) => {
                Landing(h, LandingTrigger::LowBatLanding)
            }
            (ProgressEvent::LowBatLanding, state) => state,
            (ProgressEvent::Progress(progress), ExecutingMission(a, h, _)) => {
                ExecutingMission(a, h, Some(progress))
            }
            (ProgressEvent::Progress(_), state) => state,
            (
                ProgressEvent::FailedMission(err),
                ExecutingMission(_, h, _) | Landing(h, LandingTrigger::ManualLanding),
            ) => {
                error!("Failed mission {:?}", err);
                Idle(h.await.expect(""))
            }
            (ProgressEvent::FailedMission(err), Landing(_, LandingTrigger::LowBatLanding)) => {
                error!("Failed mission {:?}", err);
                LowBatteryStopped
            }
            (ProgressEvent::FailedMission(err), state) => {
                error!("Failed mission {:?}", err);
                state
            }
            (
                ProgressEvent::MissionComplete,
                ExecutingMission(_, h, _) | Landing(h, LandingTrigger::ManualLanding),
            ) => Idle(h.await.expect("")),
            (ProgressEvent::MissionComplete, Landing(_, LandingTrigger::LowBatLanding)) => {
                LowBatteryStopped
            }
            (ProgressEvent::MissionComplete, state) => state,
        }
    }

    pub async fn run(self) {
        let mut receiver = self.command_receiver;
        let mut current_state = self.state;
        let progress_sender = self.progress_sender;
        let mut progress_update = self.progress_receiver;

        loop {
            select! {
                Some(progress_update) = progress_update.recv() => {
                    let new_state = Self::handle_progress_update(progress_update, current_state).await;
                    let _ = self.status_updates.send(new_state.as_vehicle_status());
                    current_state = new_state
                },
                Some(cmd) = receiver.recv() => {
                    let new_state = Self::handle_msg(cmd, current_state, progress_sender.clone()).await;
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
) -> (VehicleController<A>, VehicleHandle) {
    let (command_sender, command_receiver) = mpsc::channel(64);
    let (progress_sender, progress_receiver) = mpsc::channel(64);
    (
        VehicleController {
            state: Idle(autopilot),
            command_receiver,
            progress_sender,
            progress_receiver,
            status_updates,
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

    #[derive(Debug, Clone)]
    enum TestStep {
        LowBat,
        Success,
    }
    struct TestPilot {
        step_mission: mpsc::Receiver<TestStep>,
    }

    impl Autopilot for TestPilot {
        fn run_mission(
            &mut self,
            _mission: Vec<MissionItem>,
            _abort_signal: impl Future<Output = Option<Abort>> + Send,
        ) -> impl Stream<Item = ProgressEvent> {
            // steps through test commands - produces `None` when `MissionComplete`
            // sent for the first time -- `step_mission` stays alive for next test call
            futures::stream::unfold(
                (&mut self.step_mission, false),
                |(stepper, is_done)| async move {
                    if is_done {
                        None
                    } else {
                        match stepper.recv().await {
                            None => None,
                            Some(TestStep::Success) => {
                                Some((ProgressEvent::MissionComplete, (stepper, true)))
                            }
                            Some(TestStep::LowBat) => {
                                Some((ProgressEvent::LowBatLanding, (stepper, false)))
                            }
                        }
                    }
                },
            )
        }

        async fn upload_orbit(
            &mut self,
            _radius: Meters,
            _orbital_period: Duration,
            _orbits: usize,
            _z: Meters,
        ) -> Res<(TrajectoryId, Duration)> {
            Ok((TrajectoryId(12), Duration::from_millis(100)))
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
        mpsc::Sender<TestStep>,
    ) {
        let (send, rec) = watch::channel(VehicleStatus::Idle);

        let (step_sender, step_mission) = mpsc::channel(64);
        let (controller, handle) = init_vehicle_control(TestPilot { step_mission }, send);

        tokio::spawn(controller.run());

        (rec, handle, step_sender)
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
        let (mut rec, handle, step_sender) = test_setup();
        assert_status(&mut rec, VehicleStatus::Idle).await;

        handle.submit_mission(mission()).await.expect("runs fine");

        assert_status(&mut rec, VehicleStatus::MissionRunning(None)).await;

        // finish mission
        let _ = step_sender.send(TestStep::Success).await;

        assert_status(&mut rec, VehicleStatus::Idle).await;

        // start another mission
        handle.submit_mission(mission()).await.expect("runs fine");

        assert_status(&mut rec, VehicleStatus::MissionRunning(None)).await;

        // finish mission
        let _ = step_sender.send(TestStep::Success).await;

        assert_status(&mut rec, VehicleStatus::Idle).await;
    }

    #[tokio::test]
    async fn cant_start_mission_after_abort() {
        let (mut rec, handle, _step_sender) = test_setup();
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
        let (mut rec, handle, step_sender) = test_setup();
        assert_status(&mut rec, VehicleStatus::Idle).await;

        handle.submit_mission(mission()).await.expect("runs fine");

        assert_status(&mut rec, VehicleStatus::MissionRunning(None)).await;

        step_sender.send(TestStep::LowBat).await.expect("sending");

        assert_status(&mut rec, VehicleStatus::Landing).await;

        // finish landing
        let _ = step_sender.send(TestStep::Success).await;

        assert_status(&mut rec, VehicleStatus::Aborted(Reason::LowBattery)).await;

        let mission_start_res = handle.submit_mission(mission()).await;
        assert_matches!(mission_start_res, Err(_));
    }

    #[tokio::test]
    async fn can_start_mission_after_manual_landing() {
        let (mut rec, handle, step_sender) = test_setup();
        assert_status(&mut rec, VehicleStatus::Idle).await;

        handle.submit_mission(mission()).await.expect("runs fine");

        assert_status(&mut rec, VehicleStatus::MissionRunning(None)).await;

        handle.abort_mission(Abort::Land).await.expect("aborted");

        assert_status(&mut rec, VehicleStatus::Landing).await;

        // finish landing
        step_sender.send(TestStep::Success).await.expect("works");

        println!("here");
        assert_status(&mut rec, VehicleStatus::Idle).await;
    }

    #[tokio::test]
    async fn only_upload_mission_when_idle() {
        let (mut rec, handle, step_sender) = test_setup();
        assert_status(&mut rec, VehicleStatus::Idle).await;
        handle
            .upload_mission_item(mission()[0].clone())
            .await
            .expect("upload works");

        assert_status(&mut rec, VehicleStatus::Idle).await;

        handle.submit_mission(mission()).await.expect("runs fine");
        assert_status(&mut rec, VehicleStatus::MissionRunning(None)).await;

        let upload_failed = handle.upload_mission_item(mission()[0].clone()).await;
        assert_matches!(upload_failed, Err(_));

        handle.abort_mission(Abort::Land).await.expect("aborted");
        assert_status(&mut rec, VehicleStatus::Landing).await;

        let upload_failed = handle.upload_mission_item(mission()[0].clone()).await;
        assert_matches!(upload_failed, Err(_));

        // finish landing
        let _ = step_sender.send(TestStep::Success).await;
        assert_status(&mut rec, VehicleStatus::Idle).await;

        handle
            .upload_mission_item(mission()[0].clone())
            .await
            .expect("upload works");
    }
}
