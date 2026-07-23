use crossterm::event::{Event, EventStream, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use drone_control::errors::Res;
use drone_control::flight_paths::orbit;
use drone_control::{Abort, Command, CommandUnit, LinkMode, setup_link};
use futures::StreamExt;
use std::future;

#[tokio::main]
async fn main() -> Res<()> {
    let real_unit = setup_link().await?;
    let mission = run_mission(orbit(), &real_unit);

    mission.await
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

    command_unit
        .run_mission(mission, LinkMode::StreamToVehicle, abort_signal)
        .await?;
    disable_raw_mode().unwrap();
    Ok(())
}
