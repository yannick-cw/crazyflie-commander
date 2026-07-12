use crate::control::command_unit::{Meters, Telemetry};
use crate::control::low_level_engine::{Setpoint, Step, StepState};
use crate::control::vehicle::Vehicle;
use crate::utils::errors::Res;
use crate::utils::math::{OrbitPos, calc_orbit_points};
use std::time::Duration;
use std::vec;
use tracing::info;

pub async fn run_orbit(
    radius: Meters,
    orbital_period: Duration,
    orbits: usize,
    z: Meters,
    vehicle: &Vehicle,
) -> Res<()> {
    let Telemetry { x, y, .. } = vehicle.latest_telemetry();
    // move onto the orbit
    vehicle
        .go_to(
            x + radius,
            y,
            z,
            180.0_f32.to_radians(),
            Duration::from_millis(2200),
            false,
            true,
        )
        .await?;
    info!("Moved to orbit..");

    let points = calc_orbit_points(orbital_period, x, y, radius);
    let all_orbits = points.repeat(orbits);

    vehicle
        .run_steps(all_orbits.into_iter(), orbit_steps(z))
        .await
}

fn orbit_steps(
    z: Meters,
) -> impl Fn(StepState<vec::IntoIter<OrbitPos>>) -> Step<vec::IntoIter<OrbitPos>> {
    move |s| {
        let mut orbits = s.command_state;
        match orbits.next() {
            None => Step::Stop,
            Some(OrbitPos { x, y, yaw_degrees }) => Step::Continue(
                Setpoint::PositionPoint {
                    x,
                    y,
                    z,
                    yaw_degrees,
                },
                orbits,
            ),
        }
    }
}
