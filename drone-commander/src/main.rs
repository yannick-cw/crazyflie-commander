use crate::event::{EventHandler, Message};
use crate::model::Model;
use crate::tui::Tui;
use crate::update::update;
use drone_control::Telemetry;
use ratatui::prelude::*;
use std::io::stderr;
use tokio::sync::watch;

pub mod event;
pub mod flight_view;
pub mod home_view;
pub mod model;
pub mod tui;
pub mod update;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    // selection process
    // let command_unit = setup_link().await?;
    // let receiver = command_unit.latest_telemetry();

    let (_tx, receiver) = watch::channel(Telemetry::default());
    color_eyre::install()?;

    let backend = CrosstermBackend::new(stderr());
    let terminal = Terminal::new(backend)?;
    let mut tui = Tui::new(terminal);
    tui.enter()?;
    let mut model = Model::default();
    let mut event_handler = EventHandler::new(250, receiver);

    // Start the main loop.
    while !model.exit {
        // Handle events.
        let next_msg = event_handler.next().await?;

        model = process_messages(model, next_msg);

        // Render the user interface.
        tui.draw(&model)?;
    }

    // Exit the user interface.
    tui.exit()?;
    Ok(())
}

fn process_messages(model: Model, msg: Message) -> Model {
    let (model, next) = update(&model, msg); // shadow, not mut
    match next {
        Some(m) => process_messages(model, m),
        None => model,
    }
}

// TODO:
// 1. basic telemetry data live
// 2. first screen: a select mission b plan mission c free flight
// 3. mission abort shortcuts + buttons (exit: x)
// 4. render position in x y z
// 5. "connection lost"
