use drone_control::errors::Res;
use drone_control::{Abort, Command, CommandUnit, LinkMode, Meters, MetersPerSecond, Telemetry};
use futures::Stream;
use std::time::Duration;
use tokio::sync::broadcast::Receiver;
use tokio::sync::watch;
use tokio::time::sleep;
use tokio::{select, spawn, time};

pub struct DevUnit;
impl CommandUnit for DevUnit {
    async fn run_mission(
        &self,
        _mission: Vec<Command>,
        _link_mode: LinkMode,
        abort_signal: impl Future<Output = Option<Abort>>,
    ) -> Res<()> {
        Ok(select! {
            _ = sleep(Duration::from_secs(5))=> {},
            Some(_) = abort_signal=> {},
        })
    }

    async fn fly(&self, _commands: impl Stream<Item = drone_control::MotionCommand>) -> Res<()> {
        Ok(())
    }

    fn telemetry(&self) -> Receiver<Telemetry> {
        todo!()
    }

    fn latest_telemetry(&self) -> watch::Receiver<Telemetry> {
        let (sender, receiver) = watch::channel(Telemetry::default());
        spawn(async move {
            let mut ticks = time::interval(Duration::from_millis(10));
            let mut tele = Telemetry::default();
            loop {
                ticks.tick().await;
                let j = || fastrand::f32() - 0.5;
                tele.x = tele.x + Meters(j());
                tele.y = tele.y + Meters(j());
                tele.z = tele.z + Meters(j());
                tele.x_v = tele.x_v + MetersPerSecond(j());
                tele.y_v = tele.y_v + MetersPerSecond(j());
                tele.yaw_degrees += j();
                sender.send(tele).unwrap();
            }
        });
        receiver
    }

    fn mission_status(&self) -> watch::Receiver<drone_control::MissionStatus> {
        let (sender, receiver) = watch::channel(drone_control::MissionStatus::Running(None));
        spawn(async move {
            loop {
                let mut ticks = time::interval(Duration::from_millis(2000));
                let commands = vec![
                    Command::Takeoff {
                        height: Default::default(),
                        duration: Default::default(),
                    },
                    Command::MoveToWaypoint {
                        x: Default::default(),
                        y: Default::default(),
                        z: Default::default(),
                        duration: Default::default(),
                    },
                    Command::MoveToWaypoint {
                        x: Default::default(),
                        y: Default::default(),
                        z: Default::default(),
                        duration: Default::default(),
                    },
                    Command::MoveToWaypoint {
                        x: Default::default(),
                        y: Default::default(),
                        z: Default::default(),
                        duration: Default::default(),
                    },
                    Command::Land {
                        duration: Default::default(),
                    },
                ];
                for (i, c) in commands.iter().enumerate() {
                    let progress = drone_control::Progress {
                        current_command: c.clone(),
                        command_num: i,
                        total_commands: commands.len(),
                    };
                    sender
                        .send(drone_control::MissionStatus::Running(Some(progress)))
                        .unwrap();
                    ticks.tick().await;
                }
                sender.send(drone_control::MissionStatus::Idle).unwrap();
                ticks.tick().await;
            }
        });

        receiver
    }
}
