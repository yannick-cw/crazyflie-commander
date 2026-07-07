use crate::control::command_unit::{Meters, Telemetry};
use crate::utils::errors::Res;
use crazyflie_lib::subsystems::commander::Commander;
use crazyflie_lib::subsystems::high_level_commander::HighLevelCommander;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time;
use tokio::time::sleep;

pub async fn run_orbit(
    radius: Meters,
    orbital_period: Duration,
    orbits: usize,
    z: Meters,
    commander: &Commander,
    high_level_commander: &HighLevelCommander,
    telemetry: watch::Receiver<Telemetry>,
) -> Res<()> {
    let Telemetry { x, y, .. } = *telemetry.borrow();
    // move onto the orbit
    high_level_commander
        .go_to(
            x.0 + radius.0,
            y.0,
            z.0,
            180.0_f32.to_radians(),
            2.0,
            false,
            true,
            None,
        )
        .await?;
    sleep(Duration::from_millis(2200)).await;

    // 1000ms / 10ms => 100 slots
    // 360 / slots => 3.6 degree per slot
    // 360 / (duration / 10ms)
    let slots = orbital_period.as_millis() / 10;
    let degrees_per_slot = 360.0 / slots as f32;
    let points: Vec<_> = (0..slots)
        .map(|pos| {
            let angle = (pos as f32 * degrees_per_slot).to_radians();
            let x_o = x.0 + radius.0 * angle.cos();
            let y_o = y.0 + radius.0 * angle.sin();
            let yaw_deg = (angle + std::f32::consts::PI).to_degrees();

            (x_o, y_o, yaw_deg)
        })
        .collect();

    let all_orbits = points.repeat(orbits);

    let mut ticks = time::interval(Duration::from_millis(10));
    for (x, y, yaw) in all_orbits {
        commander.setpoint_position(x, y, z.0, yaw).await?;
        ticks.tick().await;
    }

    commander.notify_setpoint_stop(0).await?;
    Ok(())
}
