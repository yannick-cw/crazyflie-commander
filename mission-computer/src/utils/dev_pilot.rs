use crate::control::autopilot::ProgressEvent;
use crate::control::autopilot::VehicleDownlink;
use crate::errors::Res;
use crate::{Autopilot, ManualControl, OccupancyGrid};
use datalink::domain_types::{
    Abort, BatteryLevel, FlightMode, Meters, MetersPerSecond, MissionItem, Progress, Telemetry,
    TrajectoryId, VehicleHealth, VehicleStatus, Waypoint,
};
use futures::Stream;
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tokio::time::sleep;
use tokio::{spawn, time};

pub struct DevPilot;
impl Autopilot for DevPilot {
    fn run_mission(
        &mut self,
        _mission: Vec<MissionItem>,
        _abort_signal: impl Future<Output = Option<Abort>> + Send,
    ) -> impl Stream<Item = ProgressEvent> {
        tokio_stream::once(ProgressEvent::LowBatLanding)
    }

    async fn upload_orbit(
        &mut self,
        _radius: Meters,
        _orbital_period: Duration,
        _orbits: usize,
        _z: Meters,
    ) -> Res<(TrajectoryId, Duration)> {
        Ok((TrajectoryId::default(), Duration::default()))
    }

    async fn upload_smooth_path(
        &mut self,
        _waypoints: Vec<Waypoint>,
        _speed: MetersPerSecond,
        _flight_mode: FlightMode,
    ) -> Res<(TrajectoryId, Duration)> {
        Ok((TrajectoryId::default(), Duration::default()))
    }

    async fn fly(&mut self, _commands: impl Stream<Item = ManualControl>) -> Res<()> {
        Ok(())
    }
}

pub fn dev_downlink() -> VehicleDownlink {
    VehicleDownlink::new(
        {
            let (sender, _receiver) = broadcast::channel(1024);
            let s = sender.clone();
            spawn(async move {
                let mut ticks = time::interval(Duration::from_millis(100));
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
                    let _ = s.send(tele);
                }
            });
            sender
        },
        {
            let (sender, _receiver) = broadcast::channel(64);
            let s = sender.clone();
            spawn(async move {
                let _ = s.send(VehicleHealth::default());
                sleep(Duration::from_millis(100)).await;
                let _ = s.send(VehicleHealth {
                    battery_level: BatteryLevel::Low,
                });
            });
            sender
        },
        {
            let (sender, _receiver) = watch::channel(VehicleStatus::Idle);
            let s = sender.clone();
            spawn(async move {
                loop {
                    let mut ticks = time::interval(Duration::from_millis(100));
                    let commands = ["Takeoff", "MoveToWaypoint", "Land"];
                    for (i, c) in commands.iter().enumerate() {
                        let progress = Progress {
                            current_command: c.to_string(),
                            command_num: i,
                            total_commands: commands.len(),
                        };
                        let _ = s.send(VehicleStatus::MissionRunning(Some(progress)));
                        ticks.tick().await;
                    }
                    let _ = s.send(VehicleStatus::Idle);
                    ticks.tick().await;
                }
            });

            sender
        },
        {
            let (sender, _receiver) = broadcast::channel(64);
            let s = sender.clone();
            let mut ticks = time::interval(Duration::from_millis(100));
            spawn(async move {
                loop {
                    let _ = s.send(OccupancyGrid::default());
                    ticks.tick().await;
                }
            });
            sender
        },
    )
}
