use crate::Autopilot;
use crate::control::vehicle_control::VehicleAction::{
    AbortMission, PostMissionCleanup, StartMission,
};
use crate::control::vehicle_control::VehicleMsg::{AbortCommand, RunMission};
use crate::control::vehicle_control::VehicleState::{ExecutingMission, HardStopped, Idle, Landing};
use crate::errors::MissionError::StateError;
use crate::errors::Res;
use anyhow::Context;
use datalink::domain_types;
use datalink::domain_types::Abort;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};

#[derive(Clone, Debug, PartialOrd, PartialEq)]
enum VehicleState {
    Idle,
    Landing,
    ExecutingMission,
    // ManualControl { command_sender },
    HardStopped,
}

#[derive(Debug)]
struct Message {
    cmd: VehicleMsg,
    reply: Option<oneshot::Sender<Res<()>>>,
}
impl Message {
    fn new(cmd: VehicleMsg) -> Self {
        Self { cmd, reply: None }
    }
}

#[derive(Debug, Clone)]
enum VehicleMsg {
    RunMission(Vec<domain_types::MissionItem>),
    AbortCommand(Abort),
    MissionFinished,
    // StartManualFlight,
    // UploadTrajectory(domain_types::MissionItem),
}

#[derive(Debug, PartialOrd, PartialEq)]
enum VehicleAction {
    StartMission(Vec<domain_types::MissionItem>),
    AbortMission(Abort),
    PostMissionCleanup,
}

fn step(command: VehicleMsg, state: &VehicleState) -> Res<(VehicleState, Option<VehicleAction>)> {
    match (command, state) {
        (RunMission(mission), Idle) => Ok((ExecutingMission, Some(StartMission(mission)))),
        (RunMission(_), _) => Err(StateError("Not ready to start mission".into())),
        (VehicleMsg::MissionFinished, HardStopped) => Ok((HardStopped, Some(PostMissionCleanup))),
        (VehicleMsg::MissionFinished, _) => Ok((Idle, Some(PostMissionCleanup))),
        (AbortCommand(Abort::FlightTermination), _) => {
            Ok((HardStopped, Some(AbortMission(Abort::FlightTermination))))
        }
        (AbortCommand(Abort::Land), ExecutingMission) => {
            Ok((Landing, Some(AbortMission(Abort::Land))))
        }
        (AbortCommand(Abort::Land), _) => Err(StateError("Not in mission - cant land".into())),
    }
}

#[derive(Debug, Clone)]
pub struct VehicleHandle {
    command_sender: mpsc::Sender<Message>,
}

impl VehicleHandle {
    pub async fn submit_mission(&self, m: Vec<domain_types::MissionItem>) -> Res<()> {
        let (reply, rx) = oneshot::channel();
        self.command_sender
            .send(Message {
                cmd: RunMission(m),
                reply: Some(reply),
            })
            .await
            .context("could not send!")?;

        rx.await.context("could not receive")?
    }
    pub async fn abort_mission(&self, abort_signal: Abort) -> Res<()> {
        let (reply, rx) = oneshot::channel();
        self.command_sender
            .send(Message {
                cmd: AbortCommand(abort_signal),
                reply: Some(reply),
            })
            .await
            .context("could not send!")?;

        rx.await.context("could not receive")?
    }
}

pub struct VehicleController<A: Autopilot> {
    state: VehicleState,
    action_runner: ActionRunner<A>,
    command_receiver: mpsc::Receiver<Message>,
}

impl<A: Autopilot + Send + Sync + 'static> VehicleController<A> {
    pub async fn run(self) {
        let mut receiver = self.command_receiver;
        let mut current_state = self.state;
        let mut runner = self.action_runner;

        while let Some(cmd) = receiver.recv().await {
            current_state = match step(cmd.cmd, &current_state) {
                Ok((new_state, action)) => {
                    if let Some(reply) = cmd.reply {
                        let _ = reply.send(Ok(()));
                    }
                    if let Some(action) = action {
                        runner.execute(action).await;
                    }
                    new_state
                }
                Err(err) => {
                    info!("Invalid state transition: {:?}", err);
                    if let Some(reply) = cmd.reply {
                        let _ = reply.send(Err(err));
                    }
                    current_state
                }
            }
        }
    }
}

struct ActionRunner<A: Autopilot> {
    autopilot: Arc<A>,
    command_sender: mpsc::Sender<Message>,
    abort: Option<oneshot::Sender<Abort>>,
}

impl<A: Autopilot + Send + Sync + 'static> ActionRunner<A> {
    async fn execute(&mut self, a: VehicleAction) {
        match a {
            StartMission(mission) => {
                let (abort_sender, abort_rcv) = oneshot::channel();
                self.abort = Some(abort_sender);
                let thread_pilot = self.autopilot.clone();
                let result_sender = self.command_sender.clone();

                tokio::spawn(async move {
                    let _ = thread_pilot
                        .run_mission(mission, async { abort_rcv.await.ok() })
                        .await
                        .inspect_err(|err| error!("Failed mission execution: {:?}", err));
                    let _ = result_sender.send(Message::new(VehicleMsg::MissionFinished));
                });
            }
            AbortMission(abort_action) => {
                if let Some(s) = self.abort.take() {
                    let _ = s.send(abort_action);
                }
            }
            PostMissionCleanup => {
                self.abort = None;
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
            action_runner: ActionRunner {
                autopilot,
                command_sender: command_sender.clone(),
                abort: None,
            },
            command_receiver,
        },
        VehicleHandle { command_sender },
    )
}

#[cfg(test)]
mod tests {
    use crate::control::vehicle_control::VehicleMsg::{AbortCommand, MissionFinished, RunMission};
    use crate::control::vehicle_control::VehicleState::{
        ExecutingMission, HardStopped, Idle, Landing,
    };
    use crate::control::vehicle_control::{VehicleMsg, VehicleState, step};
    use datalink::domain_types::Abort;
    use proptest::strategy::{Just, Strategy};
    use proptest::{prop_assert, prop_assert_eq, prop_oneof};
    use test_strategy::proptest;

    fn arb_msg() -> impl Strategy<Value = VehicleMsg> {
        prop_oneof![
            Just(MissionFinished),
            Just(RunMission(vec![])),
            Just(AbortCommand(Abort::FlightTermination)),
            Just(AbortCommand(Abort::Land)),
        ]
    }

    fn arb_state() -> impl Strategy<Value = VehicleState> {
        prop_oneof![
            Just(Idle),
            Just(ExecutingMission),
            Just(HardStopped),
            Just(Landing),
        ]
    }

    #[proptest]
    fn never_recover_from_hard_stopped(#[strategy(arb_msg())] any_cmd: VehicleMsg) {
        let res = step(any_cmd, &HardStopped);

        match res {
            Ok((s, _)) => prop_assert_eq!(HardStopped, s),
            Err(_) => {}
        }
    }

    #[proptest]
    fn only_start_mission_from_idle(#[strategy(arb_state())] any_state: VehicleState) {
        let res = step(RunMission(vec![]), &any_state);

        match res {
            Ok((s, _)) => {
                prop_assert_eq!(ExecutingMission, s);
                prop_assert_eq!(Idle, any_state);
            }
            Err(_) => {
                prop_assert!(any_state != Idle)
            }
        }
    }

    #[proptest]
    fn mission_finished_always_ok(#[strategy(arb_state())] any_state: VehicleState) {
        let res = step(MissionFinished, &any_state);
        prop_assert!(res.is_ok())
    }

    #[proptest]
    fn can_only_land_during_mission(#[strategy(arb_state())] any_state: VehicleState) {
        let res = step(AbortCommand(Abort::Land), &any_state);
        match res {
            Ok((s, _)) => {
                prop_assert_eq!(Landing, s);
                prop_assert_eq!(ExecutingMission, any_state);
            }
            Err(_) => {
                prop_assert!(any_state != ExecutingMission)
            }
        }
    }
}
