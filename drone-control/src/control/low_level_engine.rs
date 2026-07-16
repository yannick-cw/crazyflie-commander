use crate::control::command_unit::{Meters, MetersPerSecond, Telemetry};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Setpoint {
    VelocityPoint {
        vx: MetersPerSecond,
        vy: MetersPerSecond,
        vz: MetersPerSecond,
        yaw_rate: f32,
    },
    PositionPoint {
        x: Meters,
        y: Meters,
        z: Meters,
        yaw_degrees: f32,
    },
}
impl Default for Setpoint {
    fn default() -> Self {
        Setpoint::PositionPoint {
            x: Default::default(),
            y: Default::default(),
            z: Default::default(),
            yaw_degrees: 0.0,
        }
    }
}

pub enum Step<S> {
    Continue(Setpoint, S),
    Stop,
}
pub struct StepState<S> {
    pub telemetry: Telemetry,
    pub time_elapsed: Duration,
    pub command_state: S,
}
