use crate::messages::Msg;
use crate::model::{Model, State};
use crate::update::update_all;
use crate::view::{flight_view, home_view, mission_select_view};
use crossterm::event::Event;
use drone_control::{CommandUnit, Telemetry, setup_link};
use futures::StreamExt;
use ratatea::{Cmd, Ratatea, Sub, run};
use ratatui::prelude::*;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

pub mod messages;
pub mod model;
pub mod update;
mod view;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    // selection process
    // this needs to live for the whole program
    let command_unit: &'static _ = Box::leak(Box::new(setup_link().await?));
    let receiver = command_unit.latest_telemetry();

    // let (_tx, receiver) = watch::channel(Telemetry::default());

    let p = Program {
        receiver,
        command_unit,
    };

    run(p).await?;
    Ok(())
}

struct Program<U: CommandUnit + 'static> {
    receiver: watch::Receiver<Telemetry>,
    command_unit: &'static U,
}
impl<U: CommandUnit> Ratatea for Program<U> {
    type Model = Model;
    type Msg = Msg;

    fn init(&self) -> (Self::Model, Cmd<Self::Msg>) {
        (Model::default(), Cmd::none())
    }

    fn update(&self, msg: Self::Msg, model: Self::Model) -> (Self::Model, Cmd<Self::Msg>) {
        update_all(self.command_unit, msg, model)
    }

    fn view(&self, model: &Self::Model, frame: &mut Frame) {
        match &model.state {
            State::Home(s) => home_view::view(s, frame),
            State::MissionExecution(_) => flight_view::view(model, frame),
            State::MissionSelect(s) => mission_select_view::view(s, frame),
            State::MissionPlan() => {}
            State::FreeFlight() => flight_view::view(model, frame),
        };
    }

    fn subscriptions(&self, _m: &Model) -> Sub<Self::Msg> {
        {
            vec![
                WatchStream::new(self.receiver.clone())
                    .map(Msg::TelemetryUpdate)
                    .boxed(),
            ]
        }
    }

    fn exit_condition(&self, model: &Self::Model) -> bool {
        model.exit
    }

    fn lift_terminal_event(&self, e: Event) -> Option<Self::Msg> {
        match e {
            Event::Key(key) => Some(Msg::Key(key)),
            // just getting the message in is enough -> triggers re-render
            Event::Resize(_, _) => Some(Msg::Resize),
            _ => None,
        }
    }
}

// TODO:
// - [x] basic telemetry data live
// - [x] first screen: a select mission b plan mission c free flight
// - [x] messages spam into screen
// ----
// - [ ] post mission stops telemetry? - more like when battery abort telemetry stops changing?
// - [ ] after mission show button to return to home screen - WORKS IFF mission is not ongoing
// - [ ] mission abort shortcuts + buttons (exit: x)
// - [ ] add mission state to telemetry and display + progress
// - [ ] give real time and steps estimates?
// - [ ] render position in x y z
// - [ ] "connection lost" warning or whatever when unplugged
// - [ ] show logs in log window or write to file
