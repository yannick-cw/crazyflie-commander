use datalink::domain_types::{
    FlightMode, Meters, MetersPerSecond, MissionItem, MissionStatus, Telemetry, TrajectoryId,
    VehicleHealth, Waypoint,
};
use futures::Stream;
use mission_computer::errors::Res;
use mission_computer::{Abort, Autopilot, OccupancyGrid};
use std::time::Duration;
use tokio::select;
use tokio::sync::broadcast::Receiver;
use tokio::time::sleep;

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

    fn health(&self) -> Receiver<VehicleHealth> {
        todo!()
    }

    fn status(&self) -> Receiver<MissionStatus> {
        todo!()
    }

    fn grid(&self) -> Receiver<OccupancyGrid> {
        todo!()
    }
}
