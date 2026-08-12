use crate::wire;
use crate::wire::WireTelemetry;
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
pub struct Telemetry {
    pub x: Meters,
    pub y: Meters,
    pub z: Meters,
    pub x_v: MetersPerSecond,
    pub y_v: MetersPerSecond,
    // pub z_v: MetersPerSecond,
    pub yaw_degrees: f32,
    pub battery_level: BatteryLevel,
    pub range_front: Meters,
    pub range_back: Meters,
    pub range_right: Meters,
    pub range_left: Meters,
    pub range_up: Meters,
}
impl Telemetry {
    pub fn is_low_bat(&self) -> bool {
        self.battery_level == BatteryLevel::Low
    }
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

impl From<Telemetry> for WireTelemetry {
    fn from(value: Telemetry) -> Self {
        WireTelemetry {
            x: value.x.0,
            y: value.y.0,
            z: value.z.0,
            x_v: value.x_v.0,
            y_v: value.y_v.0,
            z_v: 0.0,
            yaw_degrees: value.yaw_degrees,
            battery_level: match value.battery_level {
                BatteryLevel::Low => wire::wire_telemetry::BatteryLevel::Low.into(),
                BatteryLevel::High => wire::wire_telemetry::BatteryLevel::High.into(),
            },
            range_front: value.range_front.0,
            range_back: value.range_back.0,
            range_right: value.range_right.0,
            range_left: value.range_left.0,
            range_up: value.range_up.0,
        }
    }
}

impl From<WireTelemetry> for Telemetry {
    fn from(value: WireTelemetry) -> Self {
        Telemetry {
            x: Meters(value.x),
            y: Meters(value.y),
            z: Meters(value.z),
            x_v: MetersPerSecond(value.x_v),
            y_v: MetersPerSecond(value.y_v),
            yaw_degrees: value.yaw_degrees,
            battery_level: match value.battery_level() {
                wire::wire_telemetry::BatteryLevel::High => BatteryLevel::High,
                _ => BatteryLevel::Low,
            },
            range_front: Meters(value.range_front),
            range_back: Meters(value.range_back),
            range_right: Meters(value.range_right),
            range_left: Meters(value.range_left),
            range_up: Meters(value.range_up),
        }
    }
}
