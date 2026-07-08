use crate::control::command_unit::{Meters, Telemetry};
use crate::utils::errors::Res;
use crate::utils::math::{OrbitPos, calc_orbit_points};
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

    let points: Vec<_> = calc_orbit_points(orbital_period, x, y, radius);

    let all_orbits = points.repeat(orbits);

    let mut ticks = time::interval(Duration::from_millis(10));
    for OrbitPos { x, y, yaw_degrees } in all_orbits {
        commander
            .setpoint_position(x.0, y.0, z.0, yaw_degrees)
            .await?;
        ticks.tick().await;
    }

    commander.notify_setpoint_stop(0).await?;
    Ok(())
}
