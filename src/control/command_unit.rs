use crate::utils::errors::Res;
use crazyflie_lib::Value;
use crazyflie_lib::subsystems::log::LogData;
use derive_more::{Add, Div, Mul, Sub};
use std::fmt::{Display, Formatter};
use std::time::Duration;
use tokio::sync::{broadcast, watch};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Add, Sub, Mul, Div)]
pub struct Meters(pub f32);
impl Display for Meters {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}m", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Add, Sub, Mul)]
pub struct MetersPerSecond(pub f32);
impl Display for MetersPerSecond {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}m/s", self.0)
    }
}
#[derive(Clone, Copy)]
pub struct BilliardParams {
    pub bl_x: Meters,
    pub bl_y: Meters,
    pub bl_z: Meters,
    pub tr_x: Meters,
    pub tr_y: Meters,
    pub tr_z: Meters,
    pub vx: MetersPerSecond,
    pub vy: MetersPerSecond,
    pub vz: MetersPerSecond,
    pub hold_for: Duration,
}

#[derive(Clone, Copy)]
pub struct Waypoint {
    pub x: Meters,
    pub y: Meters,
    pub z: Meters,
}

pub enum FlightMode {
    Strafe,
    BodyFrame,
}

pub enum Command {
    Takeoff {
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
    // move to a waypoint relative to the takeoff position
    MoveToWaypoint {
        x: Meters,
        y: Meters,
        z: Meters,
        duration: Duration,
    },
    // smooth waypoint - relative to takeoff position
    SmoothPath {
        waypoints: Vec<Waypoint>,
        speed: MetersPerSecond,
        flight_mode: FlightMode,
    },
    // fly a bouncing pattern in the rectangle define by bl tr
    //   | ------- tr
    //   |         |
    //  bl ------- |
    BilliardBox(BilliardParams),
    Orbit {
        radius: Meters,
        orbital_period: Duration,
        orbits: usize,
        z: Meters,
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
    pub x: Meters,
    pub y: Meters,
    pub z: Meters,
    pub x_v: MetersPerSecond,
    pub y_v: MetersPerSecond,
    // pub z_v: MetersPerSecond,
    pub yaw: f32,
}
impl Telemetry {
    pub fn from_log_data(l: LogData) -> Self {
        let get = |name: &str| l.data.get(name).map(Value::to_f64_lossy).unwrap_or(0.0) as f32;
        Self {
            x: Meters(get("stateEstimate.x")),
            y: Meters(get("stateEstimate.y")),
            z: Meters(get("stateEstimate.z")),
            x_v: MetersPerSecond(get("stateEstimate.vx")),
            y_v: MetersPerSecond(get("stateEstimate.vy")),
            // z_v: MetersPerSecond(get("stateEstimate.vz")),
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
    pub fn vx(&self) -> f32 {
        self.x_v.0
    }
    pub fn vy(&self) -> f32 {
        self.y_v.0
    }
    // pub fn vz(&self) -> f32 {
    //     self.z_v.0
    // }
    pub fn yaw(&self) -> f32 {
        self.yaw
    }
    pub fn speed(&self) -> f32 {
        (self.x_v.0.powi(2) + self.y_v.0.powi(2)).sqrt()
    }
}

#[allow(async_fn_in_trait)]
pub trait CommandUnit {
    async fn run_mission(&self, mission: Vec<Command>) -> Res<()>;
    fn telemetry(&self) -> broadcast::Receiver<Telemetry>;
    fn latest_telemetry(&self) -> watch::Receiver<Telemetry>;
}
