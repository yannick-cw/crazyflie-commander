use crate::control::crazyflie::{
    PM_STATE, RANGE_BACK, RANGE_FRONT, RANGE_LEFT, RANGE_RIGHT, RANGE_UP, STATE_ESTIMATE_VX,
    STATE_ESTIMATE_VY, STATE_ESTIMATE_X, STATE_ESTIMATE_Y, STATE_ESTIMATE_YAW, STATE_ESTIMATE_Z,
};
use crate::occupancy::grid::OccupancyGrid;
use crate::utils::errors::Res;
use crazyflie_lib::Value;
use crazyflie_lib::subsystems::log::LogData;
use datalink::domain_types::{
    Abort, BatteryLevel, FlightMode, Meters, MetersPerSecond, MissionItem, MissionStatus,
    Telemetry, TrajectoryId, VehicleHealth, Waypoint,
};
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tracing::warn;

fn get_log_by_name(name: &str, l: &LogData) -> f32 {
    l.data
        .get(name)
        .map(Value::to_f64_lossy)
        .unwrap_or_else(|| {
            warn!("Could not unpack log var {name} in log data {l:?}");
            0.0
        }) as f32
}

pub fn telemetry_from_log(tele_log: &LogData, range_log: &LogData) -> Telemetry {
    Telemetry {
        x: Meters(get_log_by_name(STATE_ESTIMATE_X, tele_log)),
        y: Meters(get_log_by_name(STATE_ESTIMATE_Y, tele_log)),
        z: Meters(get_log_by_name(STATE_ESTIMATE_Z, tele_log)),
        x_v: MetersPerSecond(get_log_by_name(STATE_ESTIMATE_VX, tele_log)),
        y_v: MetersPerSecond(get_log_by_name(STATE_ESTIMATE_VY, tele_log)),
        // z_v: MetersPerSecond(get("stateEstimate.vz")),
        yaw_degrees: get_log_by_name(STATE_ESTIMATE_YAW, tele_log),
        // battery_level: if get(PM_STATE, ragene_log) >= 3.0 {
        //     BatteryLevel::Low
        // } else {
        //     BatteryLevel::High
        // },
        // if 0.0 range causes problems this could be the cause on missing log data
        range_front: Meters(get_log_by_name(RANGE_FRONT, range_log) / 1000.0),
        range_back: Meters(get_log_by_name(RANGE_BACK, range_log) / 1000.0),
        range_left: Meters(get_log_by_name(RANGE_LEFT, range_log) / 1000.0),
        range_right: Meters(get_log_by_name(RANGE_RIGHT, range_log) / 1000.0),
        range_up: Meters(get_log_by_name(RANGE_UP, range_log) / 1000.0),
    }
}

pub fn health_from_log(health_log: &LogData) -> VehicleHealth {
    VehicleHealth {
        battery_level: if get_log_by_name(PM_STATE, health_log) >= 3.0 {
            BatteryLevel::Low
        } else {
            BatteryLevel::High
        },
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SetpointHover {
    pub vx: MetersPerSecond,
    pub vy: MetersPerSecond,
    pub z: Meters,
    pub yaw_rate: f32,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ManualControl {
    TakeOff(Meters),
    Move(SetpointHover),
    Land,
    GoHome,
    Stop,
}

/// Control interface for one crazyflie: run missions, fly manually and observe live state.
///
/// Implemented by [`crate::CrazyPilot`], create with [`crate::setup_link`].
/// Run a mission with [`run_mission`](Self::run_mission) or fly live with [`fly`](Self::fly).
#[allow(async_fn_in_trait)]
pub trait Autopilot {
    fn run_mission(
        &mut self,
        mission: Vec<MissionItem>,
        abort_signal: impl Future<Output = Option<Abort>> + Send,
    ) -> impl Future<Output = Res<()>> + Send;

    async fn upload_command(
        &mut self,
        command: MissionItem,
    ) -> Res<Option<(TrajectoryId, Duration)>> {
        match command {
            MissionItem::SmoothPath {
                waypoints,
                speed,
                flight_mode,
            } => Ok(Some(
                self.upload_smooth_path(waypoints, speed, flight_mode)
                    .await?,
            )),
            MissionItem::Orbit {
                radius,
                orbital_period,
                orbits,
                z,
            } => Ok(Some(
                self.upload_orbit(radius, orbital_period, orbits, z).await?,
            )),
            _ => Ok(None),
        }
    }

    async fn upload_orbit(
        &mut self,
        radius: Meters,
        orbital_period: Duration,
        orbits: usize,
        z: Meters,
    ) -> Res<(TrajectoryId, Duration)>;

    async fn upload_smooth_path(
        &mut self,
        waypoints: Vec<Waypoint>,
        speed: MetersPerSecond,
        flight_mode: FlightMode,
    ) -> Res<(TrajectoryId, Duration)>;

    fn fly(
        &mut self,
        commands: impl Stream<Item = ManualControl> + Send,
    ) -> impl Future<Output = Res<()>> + Send;
}

pub struct VehicleDownlink {
    // emits telemetry - is updates every 10ms
    telemetry: broadcast::Sender<Telemetry>,
    // emits health - is updates every 1s
    health: broadcast::Sender<VehicleHealth>,
    // emits mission status - is updates every 100ms
    status: watch::Sender<MissionStatus>,
    // emits latest grid - updates every 100ms
    grid: broadcast::Sender<OccupancyGrid>,
}

impl VehicleDownlink {
    pub fn new(
        telemetry: broadcast::Sender<Telemetry>,
        health: broadcast::Sender<VehicleHealth>,
        status: watch::Sender<MissionStatus>,
        grid: broadcast::Sender<OccupancyGrid>,
    ) -> Self {
        Self {
            telemetry,
            health,
            status,
            grid,
        }
    }

    pub fn subscribe_telemetry(&self) -> broadcast::Receiver<Telemetry> {
        self.telemetry.subscribe()
    }
    pub fn subscribe_health(&self) -> broadcast::Receiver<VehicleHealth> {
        self.health.subscribe()
    }
    pub fn subscribe_status(&self) -> watch::Receiver<MissionStatus> {
        self.status.subscribe()
    }
    pub fn subscribe_grid(&self) -> broadcast::Receiver<OccupancyGrid> {
        self.grid.subscribe()
    }
}
