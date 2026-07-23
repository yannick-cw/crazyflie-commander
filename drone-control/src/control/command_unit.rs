use crate::control::low_level_engine::Setpoint;
use crate::utils::errors::Res;
use crazyflie_lib::Value;
use crazyflie_lib::subsystems::log::LogData;
use derive_more::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::time::Duration;
use tokio::sync::{broadcast, watch};

#[derive(Clone, Copy, Debug)]
pub enum Abort {
    HardStop,
    Land,
}

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    PartialOrd,
    Serialize,
    Deserialize,
    Copy,
    Add,
    Sub,
    Mul,
    Div,
    Neg,
)]
pub struct Meters(pub f32);

impl Display for Meters {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}m", self.0)
    }
}

#[derive(
    Serialize,
    Deserialize,
    Debug,
    Neg,
    Clone,
    Copy,
    PartialEq,
    PartialOrd,
    Default,
    Add,
    AddAssign,
    SubAssign,
    Sub,
    Mul,
)]
pub struct MetersPerSecond(pub f32);
impl Display for MetersPerSecond {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}m/s", self.0)
    }
}
#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
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

#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Waypoint {
    pub x: Meters,
    pub y: Meters,
    pub z: Meters,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
pub enum FlightMode {
    Strafe,
    BodyFrame,
}

/// A single high-level flight instruction.
///
/// A mission is a list of `Command`s executed by [`CommandUnit::run_mission`].
/// Positions are relative to the takeoff point unless a variant states otherwise.
#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
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
    // important - first setpoint has to be the current position!
    SmoothPath {
        waypoints: Vec<Waypoint>,
        speed: MetersPerSecond,
        flight_mode: FlightMode,
    },
    Setpoints {
        points: Vec<Setpoint>,
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

impl Command {
    // currently only `Orbit` supports uploading trajectory
    pub fn can_upload_trajectory(&self) -> bool {
        matches!(self, Command::Orbit { .. } | Command::SmoothPath { .. })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Default, PartialOrd, Serialize, Deserialize)]
pub enum LinkMode {
    OnVehicle,
    #[default]
    StreamToVehicle,
}

#[derive(Debug, Copy, Clone, PartialEq, Default, PartialOrd, Hash, Serialize, Deserialize)]
pub enum BatteryLevel {
    Low,
    #[default]
    High,
}

#[derive(Debug, Clone, PartialEq, Default, PartialOrd, Serialize, Deserialize)]
pub enum MissionStatus {
    #[default]
    Idle,
    Running(Option<Progress>),
    Aborted(Reason),
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Reason {
    Landing,
    HardStop,
    LowBattery,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Progress {
    pub current_command: Command,
    pub command_num: usize,
    pub total_commands: usize,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Telemetry {
    pub x: Meters,
    pub y: Meters,
    pub z: Meters,
    pub x_v: MetersPerSecond,
    pub y_v: MetersPerSecond,
    // pub z_v: MetersPerSecond,
    pub yaw_degrees: f32,
    pub battery_level: BatteryLevel,
}
impl Telemetry {
    pub fn from_log_data(tele_log: &LogData, battery_log: &LogData) -> Self {
        let get = |name: &str, l: &LogData| {
            l.data.get(name).map(Value::to_f64_lossy).unwrap_or(0.0) as f32
        };
        Self {
            x: Meters(get("stateEstimate.x", tele_log)),
            y: Meters(get("stateEstimate.y", tele_log)),
            z: Meters(get("stateEstimate.z", tele_log)),
            x_v: MetersPerSecond(get("stateEstimate.vx", tele_log)),
            y_v: MetersPerSecond(get("stateEstimate.vy", tele_log)),
            // z_v: MetersPerSecond(get("stateEstimate.vz")),
            yaw_degrees: get("stateEstimate.yaw", tele_log),
            battery_level: if get("pm.state", battery_log) >= 3.0 {
                BatteryLevel::Low
            } else {
                BatteryLevel::High
            },
        }
    }
    pub fn is_low_bat(&self) -> bool {
        self.battery_level == BatteryLevel::Low
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
        self.yaw_degrees
    }
    pub fn speed(&self) -> f32 {
        (self.x_v.0.powi(2) + self.y_v.0.powi(2)).sqrt()
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
pub enum MotionCommand {
    TakeOff(Meters),
    Move(SetpointHover),
    Land,
    GoHome,
    Stop,
}

/// Control interface for one crazyflie: run missions, fly manually and observe live state.
///
/// Implemented by [`crate::CrazyflieCommandUnit`], create with [`crate::setup_link`].
/// Run a mission with [`run_mission`](Self::run_mission) or fly live with [`fly`](Self::fly).
#[allow(async_fn_in_trait)]
pub trait CommandUnit {
    async fn run_mission(
        &self,
        mission: Vec<Command>,
        link_mode: LinkMode,
        abort_signal: impl Future<Output = Option<Abort>>,
    ) -> Res<()>;
    async fn fly(&self, commands: impl Stream<Item = MotionCommand>) -> Res<()>;
    // emits latest telemetry - is updates every 10ms
    fn telemetry(&self) -> broadcast::Receiver<Telemetry>;
    // emits latest telemetry - is updates every 10ms
    fn latest_telemetry(&self) -> watch::Receiver<Telemetry>;
    // todo maybe more sense to return form run_mission
    fn mission_status(&self) -> watch::Receiver<MissionStatus>;
}
