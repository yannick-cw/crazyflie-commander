use crate::utils::errors::Res;
use crazyflie_lib::Value;
use crazyflie_lib::subsystems::log::LogData;
use derive_more::{Add, Mul, Sub};
use std::fmt::{Display, Formatter};
use std::time::Duration;
use tokio::sync::{broadcast, watch};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Add, Sub, Mul)]
pub struct Meters(pub f32);
impl Display for Meters {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}m", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Add, Sub, Mul)]
pub struct MetersPerSecond(pub f64);
impl Display for MetersPerSecond {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}m/s", self.0)
    }
}

pub enum Command {
    TakeOff {
        height: Meters,
        duration: Duration,
    },
    // move relative to the current position
    Move {
        x: Meters,
        y: Meters,
        z: Meters,
        duration: Duration,
    },
    // move to a waypoint relative to the take off position
    MoveToWaypoint {
        x: Meters,
        y: Meters,
        z: Meters,
        duration: Duration,
    },
    Hover {
        duration: Duration,
    },
    Land {
        duration: Duration,
    },
}

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub struct Telemetry {
    x: Meters,
    y: Meters,
    z: Meters,
    x_v: MetersPerSecond,
    y_v: MetersPerSecond,
    z_v: MetersPerSecond,
    yaw: f64,
}
impl Telemetry {
    pub fn from_log_data(l: LogData) -> Self {
        let get = |name: &str| l.data.get(name).map(Value::to_f64_lossy).unwrap_or(0.0);
        Self {
            x: Meters(get("stateEstimate.x") as f32),
            y: Meters(get("stateEstimate.y") as f32),
            z: Meters(get("stateEstimate.z") as f32),
            x_v: MetersPerSecond(get("stateEstimate.vx")),
            y_v: MetersPerSecond(get("stateEstimate.vy")),
            z_v: MetersPerSecond(get("stateEstimate.vz")),
            yaw: get("stateEstimate.yaw"),
        }
    }
}
impl Telemetry {
    pub fn x(&self) -> f32 {
        self.x.0
    }
    pub fn y(&self) -> f32 {
        self.y.0
    }
    pub fn z(&self) -> f32 {
        self.z.0
    }
    pub fn vx(&self) -> f64 {
        self.x_v.0
    }
    pub fn vy(&self) -> f64 {
        self.y_v.0
    }
    pub fn vz(&self) -> f64 {
        self.z_v.0
    }
    pub fn yaw(&self) -> f64 {
        self.yaw
    }
    pub fn speed(&self) -> f64 {
        (self.x_v.0.powi(2) + self.y_v.0.powi(2) + self.z_v.0.powi(2)).sqrt()
    }
}

#[allow(async_fn_in_trait)]
pub trait CommandUnit {
    async fn run_mission(&self, mission: Vec<Command>) -> Res<()>;
    fn telemetry(&self) -> broadcast::Receiver<Telemetry>;
    fn latest_telemetry(&self) -> watch::Receiver<Telemetry>;
}
