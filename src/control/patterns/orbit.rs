use crate::control::command_unit::{Meters, Telemetry};
use crate::control::low_level_engine::{Setpoint, Step, StepState, run_commander_steps};
use crate::utils::errors::Res;
use crate::utils::math::{OrbitPos, calc_orbit_points};
use crazyflie_lib::subsystems::commander::Commander;
use crazyflie_lib::subsystems::high_level_commander::HighLevelCommander;
use std::time::Duration;
use std::vec;
use tokio::sync::watch;
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

    run_commander_steps(commander, &telemetry, all_orbits.into_iter(), orbit_steps(z)).await
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
