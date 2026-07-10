use crate::app::Model;
use crate::event::{EventHandler, Message};
use crate::tui::Tui;
use crate::update::update;
use ratatui::prelude::*;
use std::io::stderr;

pub mod app;
pub mod event;
pub mod tui;
pub mod ui;
pub mod update;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    // let real_unit = setup_link().await?;

    color_eyre::install()?;

    let mut model = Model::default();
    let event_handler = EventHandler::new(250);
    let backend = CrosstermBackend::new(stderr());
    let terminal = ratatui::Terminal::new(backend)?;
    let mut tui = Tui::new(terminal, event_handler);
    tui.enter()?;

    // Start the main loop.
    while !model.exit {
        // Handle events.
        let next_msg = tui.events.next().await?;

        model = process_messages(model, next_msg);

        // Render the user interface.
        tui.draw(&mut model)?;
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
// 1. mission select and run
// 2. basic telemetry data live
// 3. mission abort shortcuts + buttons (exit: x)
// 4. render position in x y z
