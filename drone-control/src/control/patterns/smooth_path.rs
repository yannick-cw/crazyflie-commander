use crate::control::command_unit::{FlightMode, Meters, MetersPerSecond, Telemetry, Waypoint};
use crate::control::low_level_engine::{Setpoint, Step, StepState};
use crate::control::vehicle::Vehicle;
use crate::utils::errors::Res;
use crate::utils::math::{
    SpeedVec, WaypointDist, calc_axis_speed, calc_yaw_rate, split_relative_speed_to_absolute,
    waypoint_deltas,
};

pub async fn run_smooth_path(
    path: Vec<Waypoint>,
    vehicle: &Vehicle,
    speed: MetersPerSecond,
    flight_mode: FlightMode,
) -> Res<()> {
    // faster flying, wider radius to start the turn
    let radius = Meters(speed.0 * 0.4);

    vehicle
        .run_steps(0, smooth_path_steps(path, radius, flight_mode, speed))
        .await
}
fn smooth_path_steps(
    waypoints: Vec<Waypoint>,
    radius: Meters,
    flight_mode: FlightMode,
    speed: MetersPerSecond,
) -> impl Fn(StepState<usize>) -> Step<usize> {
    move |StepState {
              telemetry: Telemetry { x, y, z, yaw, .. },
              command_state: current_wp_cursor,
              ..
          }| {
        match waypoints.get(current_wp_cursor) {
            None => Step::Stop,
            Some(waypoint) => {
                let wd @ WaypointDist {
                    dx,
                    dy,
                    dz: _dz,
                    dist,
                } = waypoint_deltas(waypoint, x, y, z);

                let yaw_rate = calc_yaw_rate(dx, dy, yaw);

                let world_speeds = calc_axis_speed(wd, speed);
                let SpeedVec { vx, vy, vz } = match flight_mode {
                    FlightMode::Strafe => world_speeds,
                    FlightMode::BodyFrame => SpeedVec {
                        vz: world_speeds.vz,
                        ..split_relative_speed_to_absolute(yaw, speed)
                    },
                };

                // if we are in radius next step we accelerate towards next wp
                let next_waypoint = if dist < radius {
                    current_wp_cursor + 1
                } else {
                    current_wp_cursor
                };
                Step::Continue(
                    Setpoint::VelocityPoint {
                        vx,
                        vy,
                        vz,
                        yaw_rate,
                    },
                    next_waypoint,
                )
            }
        }
    }
}
