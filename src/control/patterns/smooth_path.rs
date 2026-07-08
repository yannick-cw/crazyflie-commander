use crate::control::command_unit::{FlightMode, Meters, MetersPerSecond, Telemetry, Waypoint};
use crate::utils::errors::Res;
use crate::utils::math::{
    SpeedVec, WaypointDist, calc_axis_speed, calc_yaw_rate, split_relative_speed_to_absolute,
    waypoint_deltas,
};
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
        let radius = Meters(speed.0 * 0.4);
        loop {
            let Telemetry { x, y, z, yaw, .. } = *telemetry.borrow();
            let wd @ WaypointDist {
                dx,
                dy,
                dz: _dz,
                dist,
            } = waypoint_deltas(waypoint, x, y, z);

            // we reached the radius around the waypoint, abort and off to next one
            if dist < radius {
                break;
            }

            let yaw_rate = calc_yaw_rate(dx, dy, yaw);

            let SpeedVec { vx, vy, vz } = calc_axis_speed(wd, speed);
            match flight_mode {
                FlightMode::Strafe => {
                    commander
                        .setpoint_velocity_world(vx.0, vy.0, vz.0, yaw_rate)
                        .await?
                }
                FlightMode::BodyFrame => {
                    // downside - this will accelerate potentially fast towards z
                    // at lest not with specified speed
                    // alternative would be to use world frame but translate body frame into that
                    // commander
                    //     .setpoint_hover(speed.0, 0.0, yaw_rate, waypoint.z.0)
                    //     .await?
                    let SpeedVec { vx, vy, vz } = split_relative_speed_to_absolute(yaw, speed);
                    commander
                        .setpoint_velocity_world(vx.0, vy.0, vz.0, yaw_rate)
                        .await?;
                }
            }
            ticks.tick().await;
        }
    }

    commander.notify_setpoint_stop(0).await?;
    Ok(())
}
