use crate::control::command_unit::{Meters, MetersPerSecond, Telemetry};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize)]
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

pub enum Step<S> {
    Continue(Setpoint, S),
    Stop,
}
pub struct StepState<S> {
    pub telemetry: Telemetry,
    pub time_elapsed: Duration,
    pub command_state: S,
}
