use crate::messages::Msg;
use crate::model::{Model, State};
use crate::update::update_all;
use crossterm::event::Event;
use drone_control::Telemetry;
use futures::StreamExt;
use ratatea::{Cmd, Program, Sub, run};
use ratatui::prelude::*;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

pub mod flight_view;
pub mod home_view;
pub mod messages;
pub mod mission_select_view;
pub mod model;
pub mod update;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    // selection process
    // let command_unit = setup_link().await?;
    // let _receiver = command_unit.latest_telemetry();

    let (_tx, receiver) = watch::channel(Telemetry::default());

    let subscriptions: Sub<Msg> =
        { vec![WatchStream::new(receiver).map(Msg::TelemetryUpdate).boxed()] };

    let p = Program {
        init: || (Model::default(), Cmd::none()),
        update: update_all,
        view,
        subscriptions,
        lift_terminal_event: Some(|e| match e {
            Event::Key(key) => Some(Msg::Key(key)),
            _ => None,
        }),
        exit_condition: Some(|m| m.exit),
    };

    run(p).await?;
    Ok(())
}

fn view(model: &Model, frame: &mut Frame) {
    match &model.state {
        State::Home(s) => home_view::view(s, frame),
        State::MissionExecution(_) => {}
        State::MissionSelect(s) => mission_select_view::view(s, frame),
        State::MissionPlan() => {}
        State::FreeFlight() => flight_view::view(model, frame),
    };
}

// TODO:
// 1. basic telemetry data live
// 2. first screen: a select mission b plan mission c free flight
// 3. mission abort shortcuts + buttons (exit: x)
// 4. render position in x y z
// 5. "connection lost"
