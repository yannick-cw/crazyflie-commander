use crate::MetersPerSecond;
use crate::control::command_unit::{FlightMode, Waypoint};
use crate::errors::Res;
use crazyflie_lib::subsystems::memory::{Poly, Poly4D};
use std::time::Duration;
use tracing::info;

pub struct Trajectory {
    pub segments: Vec<Poly4D>,
    pub duration: Duration,
}

pub fn waypoints_to_trajectory(
    waypoints: Vec<Waypoint>,
    speed: MetersPerSecond,
    _flight_mode: FlightMode, // TODO use for yaw
) -> Res<Trajectory> {
    let waypoints_moved_1 = waypoints.iter().copied();
    let yaw = Poly::from_slice(&[]);
    let segments: Vec<_> = waypoints
        .iter()
        .zip(waypoints_moved_1.skip(1))
        .filter_map(|(start, end)| {
            let x_goal = end.x - start.x;
            let y_goal = end.y - start.y;
            let z_goal = end.z - start.z;
            // total length of vector
            let vec_len = (x_goal.0.powi(2) + y_goal.0.powi(2) + z_goal.0.powi(2)).sqrt();

            if vec_len > 0.0 {
                // normalised length of vector
                let x_norm = x_goal.0 / vec_len;
                let y_norm = y_goal.0 / vec_len;
                let z_norm = z_goal.0 / vec_len;

                // wanted speed applied in each direction
                let x_speed = speed * x_norm;
                let y_speed = speed * y_norm;
                let z_speed = speed * z_norm;

                // duration is total needed vector divided by m/s wanted
                let poly_duration = vec_len / speed.0;

                let x_vec = Poly::from_slice(&[start.x.0, x_speed.0]);
                let y_vec = Poly::from_slice(&[start.y.0, y_speed.0]);
                let z_vec = Poly::from_slice(&[start.z.0, z_speed.0]);
                Some(Poly4D::new(poly_duration, x_vec, y_vec, z_vec, yaw.clone()))
            } else {
                None
            }
        })
        .collect();
    let duration = Duration::from_secs_f32(segments.iter().map(|s| s.duration).sum());
    info!("p: {:?} d: {:?}", segments, duration);

    Ok(Trajectory { segments, duration })
}

// f(t) = 0 x (px_0=1) + t x (px_1=2)
// f(0) = 1 // at time 0 be at x = 1
// f(1) = 3 // at time 1s be at x = 3 => 2 m/s
// f(2) = 5 // at time 2s be at x = 5 => 2m/s
// => px_1 defines speed in m/s => means px_1 needs to be set to `speed` and duration changes to x(m) / speed(m/s) = duration (s)
// => but also speed distributed on x,y,z then?
