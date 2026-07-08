use crate::control::command_unit::{Abort, Command, CommandUnit, Meters};
use crate::control::crazyflie::setup_link;
use crate::utils::errors::MissionError::RenderFailure;
use crate::utils::errors::Res;
use crate::utils::flight_paths::orbit;
use crate::utils::render::{PathTrace, render_telemetry};
use crossterm::event::{Event, EventStream, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use futures::{FutureExt, StreamExt};
use std::future;
use std::future::pending;
use std::time::Duration;

pub mod control;
pub mod utils;

#[tokio::main]
async fn main() -> Res<()> {
    let real_unit = setup_link().await?;

    let mut receiver_telemetry = real_unit.telemetry();
    let render_loop = async {
        let mut trace = PathTrace::new();
        while let Ok(tele) = receiver_telemetry.recv().await {
            render_telemetry(&tele, &mut trace);
        }
    };
    let forever_render = render_loop.then(|_| pending());
    let mission = run_mission(orbit(), &real_unit);

    tokio::select! { res =  mission=> res, _ = forever_render  => Err(RenderFailure)}
}

async fn run_mission(mission: Vec<Command>, command_unit: &impl CommandUnit) -> Res<()> {
    enable_raw_mode().unwrap();

    let mut mission_abort_event = EventStream::new().filter_map(|evt| {
        future::ready(match evt {
            Ok(Event::Key(key)) if key.code == KeyCode::Char('x') => Some(Abort::HardStop),
            Ok(Event::Key(key)) if key.code == KeyCode::Char('l') => Some(Abort::Land),
            _ => None,
        })
    });
    let abort_signal = async move { mission_abort_event.next().await };

    command_unit.run_mission(mission, abort_signal).await?;
    disable_raw_mode().unwrap();
    Ok(())
}
