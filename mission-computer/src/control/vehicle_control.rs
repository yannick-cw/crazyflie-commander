use crate::Autopilot;
use crate::control::vehicle_control::VehicleState::{ExecutingMission, HardStopped, Idle, Landing};
use crate::errors::MissionError::StateError;
use crate::errors::Res;
use anyhow::Context;
use datalink::domain_types;
use datalink::domain_types::{Abort, TrajectoryId};
use std::fmt::{Debug, Formatter};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::error;

enum VehicleState<A: Autopilot> {
    Idle(A),
    Landing(JoinHandle<A>),
    ExecutingMission(oneshot::Sender<Abort>, JoinHandle<A>),
    HardStopped,
}
impl<A: Autopilot> Debug for VehicleState<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Idle(_) => write!(f, "Idle"),
            Landing(_) => write!(f, "Landing"),
            ExecutingMission(_, _) => write!(f, "ExecutingMissions"),
            HardStopped => write!(f, "HardStopped"),
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
}

impl<A: Autopilot + Send + Sync + 'static> VehicleController<A> {
    pub async fn run(self) {
        let mut receiver = self.command_receiver;
        let mut current_state = self.state;
        let command_sender = self.command_sender;

        while let Some(cmd) = receiver.recv().await {
            current_state = match (cmd, current_state) {
                (Message::RunMission(mission, reply), Idle(mut autopilot)) => {
                    let (abort_sender, abort_rcv) = oneshot::channel();

                    let local_sender = command_sender.clone();
                    let handle = tokio::spawn(async move {
                        let _ = autopilot
                            .run_mission(mission, async { abort_rcv.await.ok() })
                            .await
                            .inspect_err(|err| error!("Failed mission execution: {:?}", err));
                        let _ = local_sender.send(Message::MissionFinished).await;
                        autopilot
                    });
                    let _ = reply.send(Ok(()));
                    ExecutingMission(abort_sender, handle)
                }
                (Message::AbortCommand(abort, reply), ExecutingMission(abort_handler, h)) => {
                    let new_state = match abort {
                        Abort::FlightTermination => HardStopped,
                        Abort::Land => Landing(h),
                    };
                    let _ = abort_handler.send(abort);
                    let _ = reply.send(Ok(()));
                    new_state
                }
                (Message::MissionFinished, ExecutingMission(_, handle) | Landing(handle)) => {
                    let autopilot = handle.await.expect("should not fail");
                    Idle(autopilot)
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
    }
}

pub fn init_vehicle_control<A: Autopilot>(autopilot: A) -> (VehicleController<A>, VehicleHandle) {
    let (command_sender, command_receiver) = mpsc::channel(64);
    (
        VehicleController {
            state: Idle(autopilot),
            command_receiver,
            command_sender: command_sender.clone(),
        },
        VehicleHandle { command_sender },
    )
}

// todo test the implemented handler with a test specific Autopilot that waits on a channel to move forward
// #[cfg(test)]
// mod tests {
//     use crate::control::vehicle_control::VehicleMsg::{
//         AbortCommand, MissionFinished, RunMission, UploadMissionItem,
//     };
//     use crate::control::vehicle_control::VehicleState::{
//         ExecutingMission, HardStopped, Idle, Landing,
//     };
//     use crate::control::vehicle_control::{VehicleAction, VehicleMsg, VehicleState, step};
//     use datalink::domain_types::{Abort, MissionItem};
//     use proptest::strategy::{Just, Strategy};
//     use proptest::{prop_assert, prop_assert_eq, prop_oneof};
//     use test_strategy::proptest;
//     use tokio::sync::oneshot;
//
//     fn arb_msg() -> impl Strategy<Value = VehicleMsg> {
//         let (fake_sender, _) = oneshot::channel();
//         prop_oneof![
//             Just(MissionFinished),
//             Just(UploadMissionItem(MissionItem::Setpoints { points: vec![] })),
//             Just(RunMission(vec![], fake_sender)),
//             Just(AbortCommand(Abort::FlightTermination)),
//             Just(AbortCommand(Abort::Land)),
//         ]
//     }
//
//     fn arb_state() -> impl Strategy<Value = VehicleState> {
//         prop_oneof![
//             Just(Idle),
//             Just(ExecutingMission),
//             Just(HardStopped),
//             Just(Landing),
//         ]
//     }
//
//     #[proptest]
//     fn never_recover_from_hard_stopped(#[strategy(arb_msg())] any_cmd: VehicleMsg) {
//         let res = step(any_cmd, &HardStopped);
//
//         match res {
//             Ok((s, _)) => prop_assert_eq!(HardStopped, s),
//             Err(_) => {}
//         }
//     }
//
//     #[proptest]
//     fn only_start_mission_from_idle(#[strategy(arb_state())] any_state: VehicleState) {
//         let res = step(RunMission(vec![]), &any_state);
//
//         match res {
//             Ok((s, action)) => {
//                 prop_assert_eq!(ExecutingMission, s);
//                 prop_assert_eq!(Idle, any_state);
//                 prop_assert_eq!(VehicleAction::StartMission(vec![]), action);
//             }
//             Err(_) => {
//                 prop_assert!(any_state != Idle)
//             }
//         }
//     }
//
//     #[proptest]
//     fn mission_finished_always_ok(#[strategy(arb_state())] any_state: VehicleState) {
//         let res = step(MissionFinished, &any_state);
//         prop_assert!(res.is_ok())
//     }
//
//     #[proptest]
//     fn can_only_land_during_mission(#[strategy(arb_state())] any_state: VehicleState) {
//         let res = step(AbortCommand(Abort::Land), &any_state);
//         match res {
//             Ok((s, action)) => {
//                 prop_assert_eq!(Landing, s);
//                 prop_assert_eq!(ExecutingMission, any_state);
//                 prop_assert_eq!(VehicleAction::AbortMission(Abort::Land), action);
//             }
//             Err(_) => {
//                 prop_assert!(any_state != ExecutingMission)
//             }
//         }
//     }
//
//     #[proptest]
//     fn can_only_upload_when_idle(#[strategy(arb_state())] any_state: VehicleState) {
//         let item = MissionItem::Setpoints { points: vec![] };
//         let res = step(UploadMissionItem(item.clone()), &any_state);
//         match res {
//             Ok((s, action)) => {
//                 prop_assert_eq!(Idle, s);
//                 prop_assert_eq!(Idle, any_state);
//                 prop_assert_eq!(VehicleAction::UploadTrajectory(item), action);
//             }
//             Err(_) => {
//                 prop_assert!(any_state != Idle)
//             }
//         }
//     }
// }
