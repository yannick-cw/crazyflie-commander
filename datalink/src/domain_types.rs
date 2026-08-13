use crate::downlink::MissionStatus as RawStatus;
use crate::downlink::mission_status::aborted::Reason as RawReason;
use crate::downlink::mission_status::running::Progress as RawProgress;
use crate::downlink::{VehicleHealth as RawHealth, vehicle_health};
use crate::downlink::{VehicleState, mission_status};
use derive_more::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

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

#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct VehicleHealth {
    pub battery_level: BatteryLevel,
}
impl VehicleHealth {
    pub fn is_low_bat(&self) -> bool {
        self.battery_level == BatteryLevel::Low
    }
}

impl From<VehicleHealth> for RawHealth {
    fn from(value: VehicleHealth) -> Self {
        RawHealth {
            battery_level: match value.battery_level {
                BatteryLevel::Low => vehicle_health::BatteryLevel::Low.into(),
                BatteryLevel::High => vehicle_health::BatteryLevel::High.into(),
            },
        }
    }
}

impl From<RawHealth> for VehicleHealth {
    fn from(value: RawHealth) -> Self {
        VehicleHealth {
            battery_level: match value.battery_level() {
                vehicle_health::BatteryLevel::High => BatteryLevel::High,
                _ => BatteryLevel::Low,
            }, //todo fix
        }
    }
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
    pub range_front: Meters,
    pub range_back: Meters,
    pub range_right: Meters,
    pub range_left: Meters,
    pub range_up: Meters,
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

#[derive(Debug, Copy, Clone, PartialEq, Default, PartialOrd, Hash, Serialize, Deserialize)]
pub enum BatteryLevel {
    Low,
    #[default]
    High,
}

impl From<Telemetry> for VehicleState {
    fn from(value: Telemetry) -> Self {
        VehicleState {
            x: value.x.0,
            y: value.y.0,
            z: value.z.0,
            x_v: value.x_v.0,
            y_v: value.y_v.0,
            z_v: 0.0,
            yaw_degrees: value.yaw_degrees,
            range_front: value.range_front.0,
            range_back: value.range_back.0,
            range_right: value.range_right.0,
            range_left: value.range_left.0,
            range_up: value.range_up.0,
        }
    }
}

impl From<VehicleState> for Telemetry {
    fn from(value: VehicleState) -> Self {
        Telemetry {
            x: Meters(value.x),
            y: Meters(value.y),
            z: Meters(value.z),
            x_v: MetersPerSecond(value.x_v),
            y_v: MetersPerSecond(value.y_v),
            yaw_degrees: value.yaw_degrees,
            range_front: Meters(value.range_front),
            range_back: Meters(value.range_back),
            range_right: Meters(value.range_right),
            range_left: Meters(value.range_left),
            range_up: Meters(value.range_up),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, PartialOrd, Serialize, Deserialize)]
pub enum MissionStatus {
    #[default]
    Idle,
    Running(Option<Progress>),
    Aborted(Reason),
}

impl From<MissionStatus> for RawStatus {
    fn from(value: MissionStatus) -> Self {
        Self {
            status: Some(match value {
                MissionStatus::Idle => mission_status::Status::Idle(mission_status::Idle {}),
                MissionStatus::Running(progress) => {
                    mission_status::Status::Running(mission_status::Running {
                        p: progress.map(|p| p.into()),
                    })
                }
                MissionStatus::Aborted(reason) => {
                    mission_status::Status::Aborted(mission_status::Aborted {
                        reason: mission_status::aborted::Reason::from(reason).into(),
                    })
                }
            }),
        }
    }
}

impl From<RawStatus> for MissionStatus {
    fn from(value: RawStatus) -> Self {
        match value.status {
            None => MissionStatus::Idle,
            Some(mission_status::Status::Idle(_)) => MissionStatus::Idle,
            Some(mission_status::Status::Running(running)) => {
                MissionStatus::Running(running.p.map(|p| p.into()))
            }
            Some(mission_status::Status::Aborted(aborted)) => {
                MissionStatus::Aborted(aborted.reason().into())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Progress {
    pub current_command: String,
    pub command_num: usize,
    pub total_commands: usize,
}

impl From<Progress> for RawProgress {
    fn from(
        Progress {
            current_command,
            command_num,
            total_commands,
        }: Progress,
    ) -> Self {
        Self {
            current_command,
            command_num: command_num as u32,
            total_commands: total_commands as u32,
        }
    }
}

impl From<RawProgress> for Progress {
    fn from(value: RawProgress) -> Self {
        Self {
            current_command: value.current_command,
            command_num: value.command_num.try_into().unwrap(),
            total_commands: value.total_commands.try_into().unwrap(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Reason {
    Landing,
    HardStop,
    LowBattery,
}

impl From<RawReason> for Reason {
    fn from(value: RawReason) -> Self {
        match value {
            RawReason::Landing => Reason::Landing,
            RawReason::HardStop => Reason::HardStop,
            RawReason::LowBattery => Reason::LowBattery,
        }
    }
}

impl From<Reason> for RawReason {
    fn from(value: Reason) -> Self {
        match value {
            Reason::Landing => RawReason::Landing,
            Reason::HardStop => RawReason::HardStop,
            Reason::LowBattery => RawReason::LowBattery,
        }
    }
}
