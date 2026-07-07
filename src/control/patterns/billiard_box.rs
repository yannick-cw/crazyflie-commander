use crate::control::command_unit::{BilliardParams, Meters, Telemetry};
use crate::utils::errors::Res;
use crazyflie_lib::subsystems::commander::Commander;
use crazyflie_lib::subsystems::high_level_commander::HighLevelCommander;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tokio::time;
use tokio::time::sleep;

pub async fn run_billiard_loop(
    bx: BilliardParams,
    high_level_commander: &HighLevelCommander,
    commander: &Commander,
    telemetry: watch::Receiver<Telemetry>,
) -> Res<()> {
    let BilliardParams {
        bl_x,
        bl_y,
        bl_z,
        tr_x,
        tr_y,
        tr_z,
        vx,
        vy,
        vz,
        hold_for,
    } = bx;

    let middle_x = (tr_x + bl_x) / 2.0;
    let middle_y = (tr_y + bl_y) / 2.0;
    // aim to stay at most at this z
    let z_min = Meters(0.5);

    // move to center first
    high_level_commander
        .go_to(
            middle_x.0, middle_y.0, z_min.0, 0.0, 3.0, false, false, None,
        )
        .await?;
    sleep(Duration::from_secs(3)).await;

    let start_time = Instant::now();

    // start acceleration
    commander
        .setpoint_velocity_world(vx.0, vy.0, vz.0, 0.0)
        .await?;
    sleep(Duration::from_millis(100)).await;

    let mut ticks = time::interval(Duration::from_millis(10));
    let mut vx = vx.0;
    let mut vy = vy.0;
    let mut vz = vz.0;
    let calc_new_velocity =
        |estimated_pos: Meters, max_pos: Meters, min_pos: Meters, speed: f32| {
            if estimated_pos > max_pos {
                -speed.abs()
            } else if estimated_pos < min_pos {
                speed.abs()
            } else {
                speed
            }
        };
    while start_time + hold_for > Instant::now() {
        // deref immediately to not hold this and block
        let tele = *telemetry.borrow();
        vx = calc_new_velocity(tele.x, tr_x, bl_x, vx);
        vy = calc_new_velocity(tele.y, tr_y, bl_y, vy);
        vz = calc_new_velocity(tele.z, tr_z, bl_z, vz);
        commander.setpoint_velocity_world(vx, vy, vz, 0.0).await?;
        ticks.tick().await;
    }

    // return to center
    commander.notify_setpoint_stop(0).await?;
    // return to bl starting point
    high_level_commander
        .go_to(bl_x.0, bl_y.0, z_min.0, 0.0, 2.0, false, false, None)
        .await?;
    sleep(Duration::from_millis(2200)).await;
    Ok(())
}
