use crossterm::event::{Event, EventStream, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use datalink::domain_types::{Abort, MissionItem};
use futures::StreamExt;
use mission_computer::errors::Res;
use mission_computer::flight_paths::orbit;
use mission_computer::{Autopilot, setup_link};
use std::future;

#[tokio::main]
async fn main() -> Res<()> {
    let real_unit = setup_link().await?;
    let mission = run_mission(orbit(), &real_unit);

    mission.await
}

async fn run_mission(mission: Vec<MissionItem>, command_unit: &impl Autopilot) -> Res<()> {
    enable_raw_mode().unwrap();

    let mut mission_abort_event = EventStream::new().filter_map(|evt| {
        future::ready(match evt {
            Ok(Event::Key(key)) if key.code == KeyCode::Char('x') => Some(Abort::FlightTermination),
            Ok(Event::Key(key)) if key.code == KeyCode::Char('l') => Some(Abort::Land),
            _ => None,
        })
    });
    let abort_signal = async move { mission_abort_event.next().await };

    command_unit.run_mission(mission, abort_signal).await?;
    disable_raw_mode().unwrap();
    Ok(())
}
