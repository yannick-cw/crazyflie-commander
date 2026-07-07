use crate::control::command_unit::{FlightMode, MetersPerSecond, Telemetry, Waypoint};
use crate::utils::errors::Res;
use crazyflie_lib::subsystems::commander::Commander;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time;

pub async fn run_smooth_path(
    path: Vec<Waypoint>,
    commander: &Commander,
    speed: MetersPerSecond,
    telemetry: watch::Receiver<Telemetry>,
    flight_mode: FlightMode,
) -> Res<()> {
    let mut ticks = time::interval(Duration::from_millis(10));
    for waypoint in path {
        // faster flying, wider radius to start the turn
        let radius = speed.0 * 0.4;
        loop {
            let Telemetry { x, y, z, yaw, .. } = *telemetry.borrow();
            let dx = waypoint.x - x;
            let dy = waypoint.y - y;
            let dz = waypoint.z - z;
            let dist = (dx.0 * dx.0 + dy.0 * dy.0 + dz.0 * dz.0).sqrt();
            // we reached the radius around the waypoint, abort and off to next one
            if dist < radius {
                break;
            }

            // yaw towards target minus current yaw
            let raw_error = dy.0.atan2(dx.0).to_degrees() - yaw;
            // gets shortest turn [-180,180]
            let yaw_err = if raw_error > 180.0 {
                raw_error - 360.0
            } else if raw_error < -180.0 {
                raw_error + 360.0
            } else {
                raw_error
            };
            // get a good rate = further away => higher rate, but max limit
            let yaw_rate = (3.0 * yaw_err).clamp(-200.0, 200.0);

            // normalize vector to speed
            let delta_vec = if dist != 0.0 { dist } else { 1.0 };
            let (vx, vy, vz) = (
                speed.0 * dx.0 / delta_vec,
                speed.0 * dy.0 / delta_vec,
                speed.0 * dz.0 / delta_vec,
            );
            match flight_mode {
                FlightMode::Strafe => {
                    commander
                        .setpoint_velocity_world(vx, vy, vz, yaw_rate)
                        .await?
                }
                FlightMode::BodyFrame => {
                    // downside - this will accelerate potentially fast towards z
                    // at lest not with specified speed
                    // alternative would be to use world frame but translate body frame into that
                    // commander
                    //     .setpoint_hover(speed.0, 0.0, yaw_rate, waypoint.z.0)
                    //     .await?
                    let yaw_rad = yaw.to_radians();
                    // splitting the speed in yaw direction into its x and y speed
                    // that I can then use in the world frame for vx vy
                    // and vz stays from above
                    let vx_world = speed.0 * yaw_rad.cos();
                    let vy_world = speed.0 * yaw_rad.sin();
                    commander
                        .setpoint_velocity_world(vx_world, vy_world, vz, yaw_rate)
                        .await?;
                }
            }
            ticks.tick().await;
        }
    }

    commander.notify_setpoint_stop(0).await?;
    Ok(())
}
