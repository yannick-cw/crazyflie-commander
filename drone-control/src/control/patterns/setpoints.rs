use crate::control::low_level_engine::{Setpoint, Step, StepState};
use crate::control::vehicle::Vehicle;
use crate::utils::errors::Res;
use std::vec;

pub async fn run_setpoints(points: Vec<Setpoint>, vehicle: &Vehicle) -> Res<()> {
    vehicle
        .run_steps(points.into_iter(), setpoints_steps())
        .await
}
fn setpoints_steps() -> impl Fn(StepState<vec::IntoIter<Setpoint>>) -> Step<vec::IntoIter<Setpoint>>
{
    move |s| {
        let mut i = s.command_state;
        match i.next() {
            None => Step::Stop,
            Some(setpoint) => Step::Continue(setpoint, i),
        }
    }
}
