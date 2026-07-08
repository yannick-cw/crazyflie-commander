use crate::control::command_unit::{BilliardParams, Meters};
use crate::control::low_level_engine::{Setpoint, Step, StepState};
use crate::control::vehicle::Autopilot;
use crate::utils::errors::Res;
use crate::utils::math::{SpeedVec, inverse_v_when_oob};
use std::time::Duration;
use tokio::time::sleep;

pub async fn run_billiard_loop(billiard_params: BilliardParams, vehicle: &Autopilot) -> Res<()> {
    let BilliardParams {
        bl_x,
        bl_y,
        tr_x,
        tr_y,
        vx,
        vy,
        vz,
        ..
    } = billiard_params;

    let middle_x = (tr_x + bl_x) / 2.0;
    let middle_y = (tr_y + bl_y) / 2.0;
    // aim to stay at most at this z
    let z_min = Meters(0.5);

    // move to center first
    vehicle
        .go_to(
            middle_x,
            middle_y,
            z_min,
            0.0,
            Duration::from_secs(3),
            false,
            false,
        )
        .await?;
    sleep(Duration::from_secs(3)).await;

    vehicle
        .run_steps(SpeedVec { vx, vy, vz }, billiard_steps(billiard_params))
        .await?;

    // return to bl starting point
    vehicle
        .go_to(bl_x, bl_y, z_min, 0.0, Duration::from_secs(2), false, false)
        .await?;
    sleep(Duration::from_millis(2200)).await;
    Ok(())
}

fn billiard_steps(bx: BilliardParams) -> impl Fn(StepState<SpeedVec>) -> Step<SpeedVec> {
    move |StepState {
              telemetry,
              time_elapsed,
              command_state: current_speed,
          }| {
        if time_elapsed > bx.hold_for {
            Step::Stop
        } else {
            let vx = inverse_v_when_oob(telemetry.x, bx.tr_x, bx.bl_x, current_speed.vx);
            let vy = inverse_v_when_oob(telemetry.y, bx.tr_y, bx.bl_y, current_speed.vy);
            let vz = inverse_v_when_oob(telemetry.z, bx.tr_z, bx.bl_z, current_speed.vz);
            Step::Continue(
                Setpoint::VelocityPoint {
                    vx,
                    vy,
                    vz,
                    yaw_rate: 0.0,
                },
                SpeedVec { vx, vy, vz },
            )
        }
    }
}
