use futures::Stream;
use mission_computer::errors::Res;
use mission_computer::{
    Abort, Autopilot, FlightMode, Meters, MetersPerSecond, MissionItem, OccupancyGrid, Telemetry,
    TrajectoryId, Waypoint,
};
use std::time::Duration;
use tokio::sync::broadcast::Receiver;
use tokio::sync::watch;
use tokio::time::sleep;
use tokio::{select, spawn, time};

pub struct DevPilot;
impl Autopilot for DevPilot {
    async fn run_mission(
        &self,
        _mission: Vec<MissionItem>,
        abort_signal: impl Future<Output = Option<Abort>>,
    ) -> Res<()> {
        select! {
            _ = sleep(Duration::from_secs(5))=> {},
            Some(_) = abort_signal=> {},
        };
        Ok(())
    }

    async fn upload_orbit(
        &self,
        _radius: Meters,
        _orbital_period: Duration,
        _orbits: usize,
        _z: Meters,
    ) -> Res<(TrajectoryId, Duration)> {
        Ok((TrajectoryId::default(), Duration::default()))
    }

    async fn upload_smooth_path(
        &self,
        _waypoints: Vec<Waypoint>,
        _speed: MetersPerSecond,
        _flight_mode: FlightMode,
    ) -> Res<(TrajectoryId, Duration)> {
        Ok((TrajectoryId::default(), Duration::default()))
    }

    async fn fly(&self, _commands: impl Stream<Item = mission_computer::ManualControl>) -> Res<()> {
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
                tele.x_v += MetersPerSecond(j());
                tele.y_v += MetersPerSecond(j());
                tele.yaw_degrees += j();
                sender.send(tele).unwrap();
            }
        });
        receiver
    }

    fn latest_grid(&self) -> watch::Receiver<mission_computer::OccupancyGrid> {
        let (_, receiver) = watch::channel(OccupancyGrid::new());
        receiver
    }

    fn mission_status(&self) -> watch::Receiver<mission_computer::MissionStatus> {
        let (sender, receiver) = watch::channel(mission_computer::MissionStatus::Running(None));
        spawn(async move {
            loop {
                let mut ticks = time::interval(Duration::from_millis(2000));
                let commands = [
                    MissionItem::Takeoff {
                        height: Default::default(),
                        duration: Default::default(),
                    },
                    MissionItem::MoveToWaypoint {
                        x: Default::default(),
                        y: Default::default(),
                        z: Default::default(),
                        duration: Default::default(),
                    },
                    MissionItem::MoveToWaypoint {
                        x: Default::default(),
                        y: Default::default(),
                        z: Default::default(),
                        duration: Default::default(),
                    },
                    MissionItem::MoveToWaypoint {
                        x: Default::default(),
                        y: Default::default(),
                        z: Default::default(),
                        duration: Default::default(),
                    },
                    MissionItem::Land {
                        duration: Default::default(),
                    },
                ];
                for (i, c) in commands.iter().enumerate() {
                    let progress = mission_computer::Progress {
                        current_command: c.clone(),
                        command_num: i,
                        total_commands: commands.len(),
                    };
                    sender
                        .send(mission_computer::MissionStatus::Running(Some(progress)))
                        .unwrap();
                    ticks.tick().await;
                }
                sender.send(mission_computer::MissionStatus::Idle).unwrap();
                ticks.tick().await;
            }
        });

        receiver
    }
}
