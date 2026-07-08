use crate::control::command_unit::{Meters, MetersPerSecond, Waypoint};
use std::time::Duration;

pub fn inverse_v_when_oob(
    estimated_pos: Meters,
    max_pos: Meters,
    min_pos: Meters,
    speed: MetersPerSecond,
) -> MetersPerSecond {
    if estimated_pos > max_pos {
        MetersPerSecond(-speed.0.abs())
    } else if estimated_pos < min_pos {
        MetersPerSecond(speed.0.abs())
    } else {
        speed
    }
}

#[derive(Copy, Clone)]
pub struct OrbitPos {
    pub x: Meters,
    pub y: Meters,
    pub yaw_degrees: f32,
}
pub fn calc_orbit_points(
    orbital_period: Duration,
    center_x: Meters,
    center_y: Meters,
    radius: Meters,
) -> Vec<OrbitPos> {
    // 1000ms / 10ms => 100 slots
    // 360 / slots => 3.6 degree per slot
    // 360 / (duration / 10ms)
    let slots = orbital_period.as_millis() / 10;
    let degrees_per_slot = 360.0 / slots as f32;
    (0..slots)
        .map(|pos| {
            let angle = (pos as f32 * degrees_per_slot).to_radians();
            let x = Meters(center_x.0 + radius.0 * angle.cos());
            let y = Meters(center_y.0 + radius.0 * angle.sin());
            let yaw_degrees = (angle + std::f32::consts::PI).to_degrees();

            OrbitPos { x, y, yaw_degrees }
        })
        .collect()
}

pub fn calc_yaw_rate(dx: Meters, dy: Meters, yaw: f32) -> f32 {
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
    (3.0 * yaw_err).clamp(-200.0, 200.0)
}

pub struct WaypointDist {
    pub dx: Meters,
    pub dy: Meters,
    pub dz: Meters,
    pub dist: Meters,
}
pub fn waypoint_deltas(w: Waypoint, x: Meters, y: Meters, z: Meters) -> WaypointDist {
    let dx = w.x - x;
    let dy = w.y - y;
    let dz = w.z - z;
    let dist = Meters((dx.0 * dx.0 + dy.0 * dy.0 + dz.0 * dz.0).sqrt());
    WaypointDist { dx, dy, dz, dist }
}

pub struct SpeedVec {
    pub vx: MetersPerSecond,
    pub vy: MetersPerSecond,
    pub vz: MetersPerSecond,
}
pub fn calc_axis_speed(w_dist: WaypointDist, target_speed: MetersPerSecond) -> SpeedVec {
    // normalize vector to speed
    let WaypointDist { dx, dy, dz, dist } = w_dist;
    let delta_vec = if dist.0 != 0.0 { dist } else { Meters(1.0) };
    let (vx, vy, vz) = (
        MetersPerSecond(target_speed.0 * dx.0 / delta_vec.0),
        MetersPerSecond(target_speed.0 * dy.0 / delta_vec.0),
        MetersPerSecond(target_speed.0 * dz.0 / delta_vec.0),
    );
    SpeedVec { vx, vy, vz }
}

pub fn split_relative_speed_to_absolute(yaw: f32, speed: MetersPerSecond) -> SpeedVec {
    let yaw_rad = yaw.to_radians();
    // splitting the speed in yaw direction into its x and y speed
    // that I can then use in the world frame for vx vy
    // and vz stays from above
    let vx = MetersPerSecond(speed.0 * yaw_rad.cos());
    let vy = MetersPerSecond(speed.0 * yaw_rad.sin());
    SpeedVec {
        vx,
        vy,
        vz: MetersPerSecond(0.0),
    }
}
