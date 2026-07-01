use crate::control::command_unit::Command::{Land, MoveToWaypoint, TakeOff};
use crate::control::command_unit::{Command, CommandUnit, Meters, TestCommandUnit};
use crate::control::crazyflie::setup_link;
use crate::utils::errors::MissionError;
use std::time::Duration;

pub mod control;
pub mod utils;

#[tokio::main]
async fn main() -> Result<(), MissionError> {
    let commands = vec![
        TakeOff {
            height: Meters(2.0),
            duration: Duration::from_secs(3),
        },
        MoveToWaypoint {
            x: Meters(1.0),
            y: Meters(1.0),
            z: Meters(1.0),
            duration: Duration::from_secs(5),
        },
        Land {
            duration: Duration::from_secs(2),
        },
    ];

    let real_unit = setup_link().await?;
    let _test_unit = TestCommandUnit {
        start_duration: Duration::default(),
    };

    run_mission(commands, &real_unit).await?;
    Ok(())
}

async fn run_mission(
    mission: Vec<Command>,
    command_unit: &impl CommandUnit,
) -> Result<(), MissionError> {
    let mission_res = command_unit.run_mission(mission).await?;

    Ok(mission_res
        .iter()
        .for_each(|waypoint| println!("{}", waypoint)))
}
