use datalink::domain_types::{Setpoint, Telemetry};
use std::time::Duration;

pub enum Step<S> {
    Continue(Setpoint, S),
    Stop,
}
pub struct StepState<S> {
    pub telemetry: Telemetry,
    pub time_elapsed: Duration,
    pub command_state: S,
}
