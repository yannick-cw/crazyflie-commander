use crate::app::App;
use crate::event::{Event, EventHandler};
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

    let mut app = App::default();
    let event_handler = EventHandler::new(250);
    let backend = CrosstermBackend::new(stderr());
    let terminal = ratatui::Terminal::new(backend)?;
    let mut tui = Tui::new(terminal, event_handler);
    tui.enter()?;

    // Start the main loop.
    while !app.exit {
        // Handle events.
        match tui.events.next().await? {
            Event::Tick => {}
            Event::Key(key_event) => update(&mut app, key_event),
        };

        // Render the user interface.
        tui.draw(&mut app)?;
    }

    // Exit the user interface.
    tui.exit()?;
    Ok(())
}

// TODO:
// 1. mission select and run
// 2. basic telemetry data live
// 3. mission abort shortcuts + buttons (exit: x)
// 4. render position in x y z
