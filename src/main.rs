use crate::control::command_unit::{Command, CommandUnit, Meters};
use crate::control::crazyflie::setup_link;
use crate::utils::errors::Res;
use crate::utils::flight_paths::body_frame_smooth;
use crate::utils::render::{render_telemetry, PathTrace};
use std::time::Duration;

pub mod control;
pub mod utils;

#[tokio::main]
async fn main() -> Res<()> {
    let real_unit = setup_link().await?;

    let mut receiver_telemetry = real_unit.telemetry();
    tokio::spawn(async move {
        let mut trace = PathTrace::new();
        while let Ok(tele) = receiver_telemetry.recv().await {
            render_telemetry(&tele, &mut trace);
        }
    });

    run_mission(body_frame_smooth(), &real_unit).await?;
    Ok(())
}

async fn run_mission(mission: Vec<Command>, command_unit: &impl CommandUnit) -> Res<()> {
    command_unit.run_mission(mission).await
}
