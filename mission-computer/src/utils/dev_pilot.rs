use crate::errors::Res;
use crate::{
    Abort, Autopilot, FlightMode, ManualControl, MissionItem, OccupancyGrid, TrajectoryId, Waypoint,
};
use datalink::domain_types::{
    BatteryLevel, Meters, MetersPerSecond, MissionStatus, Progress, Telemetry, VehicleHealth,
};
use futures::Stream;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Receiver;
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

    async fn fly(&self, _commands: impl Stream<Item = ManualControl>) -> Res<()> {
        Ok(())
    }

    fn telemetry(&self) -> Receiver<Telemetry> {
        let (sender, receiver) = broadcast::channel(1024);
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
                let _ = sender.send(tele);
            }
        });
        receiver
    }

    fn health(&self) -> Receiver<VehicleHealth> {
        let (sender, receiver) = broadcast::channel(64);
        spawn(async move {
            let _ = sender.send(VehicleHealth::default());
            sleep(Duration::from_millis(1000)).await;
            let _ = sender.send(VehicleHealth {
                battery_level: BatteryLevel::Low,
            });
        });
        receiver
    }

    fn status(&self) -> Receiver<MissionStatus> {
        let (sender, receiver) = broadcast::channel(64);
        spawn(async move {
            loop {
                let mut ticks = time::interval(Duration::from_millis(2000));
                let commands = ["Takeoff", "MoveToWaypoint", "Land"];
                for (i, c) in commands.iter().enumerate() {
                    let progress = Progress {
                        current_command: c.to_string(),
                        command_num: i,
                        total_commands: commands.len(),
                    };
                    sender.send(MissionStatus::Running(Some(progress))).unwrap();
                    ticks.tick().await;
                }
                let _ = sender.send(MissionStatus::Idle);
                ticks.tick().await;
            }
        });

        receiver
    }

    fn grid(&self) -> Receiver<OccupancyGrid> {
        let (s, receiver) = broadcast::channel(64);
        let _ = s.send(OccupancyGrid::default());
        receiver
    }
}
