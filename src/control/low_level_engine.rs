use crate::control::command_unit::{Meters, MetersPerSecond, Telemetry};
use crate::utils::errors::Res;
use crazyflie_lib::subsystems::commander::Commander;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tokio::time;

#[derive(Debug)]
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
pub async fn run_commander_steps<S>(
    commander: &Commander,
    telemetry_rx: &watch::Receiver<Telemetry>,
    init_command_state: S,
    next_step: impl Fn(StepState<S>) -> Step<S>,
) -> Res<()> {
    let start_time = Instant::now();

    let mut command_state = init_command_state;
    let mut ticks = time::interval(Duration::from_millis(10));

    while let Step::Continue(setpoint, next_cmd_state) = next_step(StepState {
        telemetry: *telemetry_rx.borrow(),
        time_elapsed: Instant::now() - start_time,
        command_state,
    }) {
        command_state = next_cmd_state;
        match setpoint {
            Setpoint::VelocityPoint {
                vx,
                vy,
                vz,
                yaw_rate,
            } => {
                commander
                    .setpoint_velocity_world(vx.0, vy.0, vz.0, yaw_rate)
                    .await?;
            }
            Setpoint::PositionPoint {
                x,
                y,
                z,
                yaw_degrees: yaw,
            } => {
                commander.setpoint_position(x.0, y.0, z.0, yaw).await?;
            }
        }

        ticks.tick().await;
    }

    // stop low level commander
    commander.notify_setpoint_stop(0).await?;

    Ok(())
}
