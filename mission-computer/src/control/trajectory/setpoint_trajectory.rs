use crate::MetersPerSecond;
use crate::control::command_unit::{FlightMode, Waypoint};
use crate::errors::Res;
use crazyflie_lib::subsystems::memory::{Poly, Poly4D};
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone)]
pub struct Trajectory {
    pub segments: Vec<Poly4D>,
    pub duration: Duration,
}

pub fn waypoints_to_trajectory(
    waypoints: Vec<Waypoint>,
    speed: MetersPerSecond,
    flight_mode: FlightMode,
) -> Res<Trajectory> {
    let waypoints_moved_1 = waypoints.iter().copied();
    let segments: Vec<_> = waypoints
        .iter()
        .zip(waypoints_moved_1.skip(1))
        .filter_map(|(start, end)| {
            let x_goal = end.x - start.x;
            let y_goal = end.y - start.y;
            let z_goal = end.z - start.z;
            // total length of vector
            let vec_len = (x_goal.0.powi(2) + y_goal.0.powi(2) + z_goal.0.powi(2)).sqrt();

            let target_yaw_radians = y_goal.0.atan2(x_goal.0);
            // instant turn to new yaw
            let yaw = Poly::from_slice(&[target_yaw_radians]);
            let yaw = match flight_mode {
                FlightMode::Strafe => Poly::from_slice(&[0.0]),
                FlightMode::BodyFrame => yaw,
            };

            if vec_len > 0.0 {
                // normalized length of vector
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Meters;
    use std::f32::consts::FRAC_1_SQRT_2;

    #[test]
    fn create_expected_trajectory() -> Res<()> {
        let wps = vec![
            Waypoint {
                x: Meters(0.0),
                y: Meters(0.0),
                z: Meters(0.5),
            },
            Waypoint {
                x: Meters(1.0),
                y: Meters(0.0),
                z: Meters(0.5),
            },
            Waypoint {
                x: Meters(1.0),
                y: Meters(1.0),
                z: Meters(0.5),
            },
            Waypoint {
                x: Meters(0.0),
                y: Meters(0.0),
                z: Meters(0.5),
            },
        ];
        let trj = waypoints_to_trajectory(wps, MetersPerSecond(1.0), FlightMode::BodyFrame)?;

        let ([s1, s2, s3], _) = trj
            .segments
            .split_first_chunk::<3>()
            .expect("three segments expected");

        assert_eq!(trj.duration, Duration::from_secs_f32(3.414213657));
        assert_eq!(trj.segments.len(), 3);
        // first segment only moves towards x
        assert_eq!(s1.x.values, [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(s1.y.values, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(s1.z.values, [0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(s1.yaw.values, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);

        // second segment moves towards y, staying at x and z and yaw 90° (moving left or north)
        assert_eq!(s2.x.values, [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(s2.y.values, [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(s2.z.values, [0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(
            s2.yaw.values,
            [90.0_f32.to_radians(), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );

        // third segment moves towards 0,0 - means ~-0.7 in x and y direction and yaw -135° (moving way left, south-west)
        assert_eq!(
            s3.x.values,
            [1.0, -FRAC_1_SQRT_2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            s3.y.values,
            [1.0, -FRAC_1_SQRT_2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(s3.z.values, [0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(
            s3.yaw.values,
            [-135.0_f32.to_radians(), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );

        Ok(())
    }
}
