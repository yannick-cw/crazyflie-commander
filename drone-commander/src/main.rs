use crate::dev_unit::DevUnit;
use crate::messages::{MissionExecutionMessage, Msg};
use crate::model::{Model, State};
use crate::update::update_all;
use crate::view::{flight_view, home_view, mission_select_view};
use crossterm::event::Event;
use drone_control::{CommandUnit, setup_link};
use futures::StreamExt;
use ratatea::{Cmd, Ratatea, Sub, run};
use ratatui::prelude::*;
use tokio_stream::wrappers::WatchStream;
use tracing::info;

mod dev_unit;
pub mod messages;
pub mod model;
pub mod update;
mod view;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let file_appender = tracing_appender::rolling::never("./logs", "commander.log");
    tracing_subscriber::fmt()
        .with_writer(file_appender)
        .with_ansi(false)
        .init();

    info!("Starting up....");
    Ok(match setup_link().await {
        Ok(real_unit) => {
            // selection process
            // this needs to live for the whole program
            let command_unit: &'static _ = Box::leak(Box::new(real_unit));

            let p = Program { command_unit };
            run(p).await?;
        }
        _ => {
            // fallback for dev
            let command_unit = &DevUnit;
            let p = Program { command_unit };
            run(p).await?;
        }
    })
}

struct Program<U: CommandUnit + 'static> {
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
            State::FreeFlight(_) => flight_view::view(model, frame),
        };
    }

    fn subscriptions(&self, _m: &Model) -> Sub<Self::Msg> {
        {
            vec![
                WatchStream::new(self.command_unit.latest_telemetry().clone())
                    .map(Msg::TelemetryUpdate)
                    .boxed(),
                WatchStream::new(self.command_unit.mission_status().clone())
                    .map(|update| {
                        Msg::MissionExecution(MissionExecutionMessage::MissionUpdate(update))
                    })
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
// - [x] mission abort shortcuts + buttons (exit: x)
// - [x] after mission show button to return to home screen - WORKS IFF mission is not ongoing
// - [x] add mission state to telemetry and display + progress
// - [ ] give real time and steps estimates? - Wont do
// - [x] render position in x y z
// - [x] build free flight; wasd, QE for yaw, jk for up down
//       - first step auto take off + w for flying forwards
// - [x] show logs in ~~log window~~ or write to file
// ---- NEXT
// - [ ] back from free flight
// --- NEXT
// - [ ] x in free flight must interrupt all!
// - [ ] ratatea re-evaluate subscriptions
// - [ ] speed up down modification in flight
// - [ ] recording live flight and replay?
// - [ ] post mission stops telemetry? - more like when battery abort telemetry stops changing?
// - [ ] "connection lost" warning or whatever when unplugged
// - [ ] build mission planner
