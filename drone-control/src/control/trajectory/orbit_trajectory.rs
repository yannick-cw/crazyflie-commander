use crate::Meters;
use crate::errors::Res;
use crazyflie_lib::subsystems::memory::{CompressedSegment, CompressedStart};
use std::time::Duration;

pub struct CompressedTrajectory {
    pub start: CompressedStart,
    pub segments: Vec<CompressedSegment>,
    pub duration: Duration,
}

pub fn orbit_to_trajectory(
    radius: Meters,
    orbital_period: Duration,
    orbits: usize,
    z: Meters,
) -> Res<CompressedTrajectory> {
    let segment_duration = orbital_period / 4;
    // kappa constant for handle distance
    let radius = radius.0;
    let z = z.0;
    let kappa = 0.5522847;
    // distance l where the two handles for the quarter circle sit (between start and end points)
    let l = kappa * radius;
    let zero = 0.0;

    // the 4 x vectors for each quarter circle, each is handle 1 -> handle 2 -> end point
    let (x1, x2, x3, x4) = (
        [radius, l, zero],
        [-l, -radius, -radius],
        [-radius, -l, zero],
        [l, radius, radius],
    );
    // same but for y
    let (y1, y2, y3, y4) = (
        [l, radius, radius],
        [radius, l, zero],
        [-l, -radius, -radius],
        [-radius, -l, zero],
    );
    let z_steady = vec![z];

    let to_start_duration = Duration::from_secs(2);
    let to_start_east = CompressedSegment::new(
        to_start_duration.as_secs_f32(),
        vec![radius],
        vec![zero],
        vec![z],
        vec![-180_f32.to_radians()],
    )?;

    let to_north = CompressedSegment::new(
        segment_duration.as_secs_f32(),
        x1.to_vec(),
        y1.to_vec(),
        z_steady.clone(),
        vec![-90_f32.to_radians()].clone(),
    )?;
    let to_west = CompressedSegment::new(
        segment_duration.as_secs_f32(),
        x2.to_vec(),
        y2.to_vec(),
        z_steady.clone(),
        vec![0_f32.to_radians()].clone(),
    )?;
    let to_south = CompressedSegment::new(
        segment_duration.as_secs_f32(),
        x3.to_vec(),
        y3.to_vec(),
        z_steady.clone(),
        vec![90_f32.to_radians()].clone(),
    )?;
    let to_east = CompressedSegment::new(
        segment_duration.as_secs_f32(),
        x4.to_vec(),
        y4.to_vec(),
        z_steady,
        vec![180_f32.to_radians()],
    )?;

    let start = CompressedStart::new(0.0, 0.0, z, 0.0);

    let mut segments = vec![to_start_east];
    // flight to start from first segment
    let mut total_duration = to_start_duration;

    for _ in 1..=orbits {
        // for each orbit - adding orbit duration
        total_duration += orbital_period;
        segments.extend([
            to_north.clone(),
            to_west.clone(),
            to_south.clone(),
            to_east.clone(),
        ]);
    }

    Ok(CompressedTrajectory {
        start,
        segments,
        duration: total_duration,
    })
}
