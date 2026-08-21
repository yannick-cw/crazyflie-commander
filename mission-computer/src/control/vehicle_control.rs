use crate::Autopilot;
use crate::control::vehicle_control::VehicleMessages::{AbortCommand, MissionFinished, RunMission};
use crate::control::vehicle_control::VehicleState::{ExecutingMission, HardStopped, Idle, Landing};
use crate::errors::MissionError::StateError;
use crate::errors::Res;
use anyhow::Context;
use datalink::domain_types;
use datalink::domain_types::Abort;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::error;

// todo make testable

#[derive(Debug)]
enum VehicleState {
    Idle,
    Landing,
    ExecutingMission(oneshot::Sender<Abort>),
    // ManualControl { command_sender },
    HardStopped,
}

enum VehicleMessages {
    RunMission {
        mission: Vec<domain_types::MissionItem>,
        reply: oneshot::Sender<Res<()>>,
    },
    MissionFinished,
    AbortCommand {
        abort: Abort,
        reply: oneshot::Sender<Res<()>>,
    },
    // StartManualFlight,
    // UploadTrajectory(domain_types::MissionItem),
}

#[derive(Debug, Clone)]
pub struct VehicleHandle {
    command_sender: mpsc::Sender<VehicleMessages>,
}

impl VehicleHandle {
    pub async fn submit_mission(&self, m: Vec<domain_types::MissionItem>) -> Res<()> {
        let (reply, rx) = oneshot::channel();
        self.command_sender
            .send(RunMission { mission: m, reply })
            .await
            .context("could not send!")?;

        rx.await.context("could not receive")?
    }
    pub async fn abort_mission(&self, abort_signal: Abort) -> Res<()> {
        let (reply, rx) = oneshot::channel();
        self.command_sender
            .send(AbortCommand {
                abort: abort_signal,
                reply,
            })
            .await
            .context("could not send!")?;

        rx.await.context("could not receive")?
    }
}

pub struct VehicleController<A: Autopilot> {
    state: VehicleState,
    autopilot: Arc<A>,
    command_receiver: mpsc::Receiver<VehicleMessages>,
    command_sender: mpsc::Sender<VehicleMessages>,
}

impl<A: Autopilot + Send + Sync + 'static> VehicleController<A> {
    pub async fn run(self) {
        let mut receiver = self.command_receiver;
        let mut state = self.state;
        let autopilot = self.autopilot;
        let sender = self.command_sender;

        while let Some(cmd) = receiver.recv().await {
            state = match (cmd, state) {
                (RunMission { mission, reply }, Idle) => {
                    let (abort, abort_rcv) = oneshot::channel();
                    let _ = reply.send(Ok(()));

                    let thread_pilot = autopilot.clone();
                    let result_sender = sender.clone();

                    tokio::spawn(async move {
                        let _ = thread_pilot
                            .run_mission(mission, async { abort_rcv.await.ok() })
                            .await
                            .inspect_err(|err| error!("Failed mission execution: {:?}", err));
                        let _ = result_sender.send(MissionFinished).await;
                    });
                    ExecutingMission(abort)
                }
                (RunMission { reply, .. }, s) => {
                    let _ = reply.send(Err(StateError(format!(
                        "Can't run mission - current state {:?}",
                        s
                    ))));
                    s
                }
                (
                    AbortCommand {
                        abort: Abort::FlightTermination,
                        reply,
                    },
                    ExecutingMission(abort_sender),
                ) => {
                    let _ = abort_sender.send(Abort::FlightTermination);
                    let _ = reply.send(Ok(()));
                    HardStopped
                }
                (
                    AbortCommand {
                        abort: Abort::Land,
                        reply,
                    },
                    ExecutingMission(abort_sender),
                ) => {
                    let _ = abort_sender.send(Abort::Land);
                    let _ = reply.send(Ok(()));
                    Landing
                }
                (AbortCommand { reply, .. }, s) => {
                    let _ = reply.send(Err(StateError(format!(
                        "Can't abort mission - current state {:?}",
                        s
                    ))));
                    s
                }
                (MissionFinished, ExecutingMission(_)) => Idle,
                (MissionFinished, s) => s,
            }
        }
    }
}

pub fn init_vehicle_control<A: Autopilot>(
    autopilot: Arc<A>,
) -> (VehicleController<A>, VehicleHandle) {
    let (command_sender, command_receiver) = mpsc::channel(64);
    (
        VehicleController {
            state: Idle,
            autopilot,
            command_receiver,
            command_sender: command_sender.clone(),
        },
        VehicleHandle { command_sender },
    )
}
