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
// - [x] speed up down modification in flight
// - [x] back from free flight < broken does not finish flying somehow - must make sure to end flying future
// - [x] landing in free flight in place + go home?
// - [ ] x in free flight must interrupt all! (won't do for now; only relevant for landing)
// - [x] TUI stores missions as JSON and reads those on start; control exposes serde decode encode (maybe validation)
// - [x] recording live flight and replay?
//      d- r for record, r again for stop - show recording toggle
//      d- recording window goes to the right size in the column (e.g. red marker during recording)
//      d- r collect all telemetry of actual position with 10ms resolution
//      d- store as json and just replay via low level setpoint commander
//      d- but what if not in same position -> would accelarate to start point very fast -> easy go to start position
//      d=> while ignoring z below 0.1m or min 0.1m, and only then, when hovering at start point -> go
//      - could even blink LEDs before starting beep beep beeeeep and when last setpoint is below 0.1m or so land after
//      d- tui records libs telemetry for recording time and stores as json
//      d- lib gets new command::replay that takes a list of setpoints with timestamps or duration offsets from start and
//      executes as normal thing as always <-> and lib gets the logic of slow fly to start and land or hover at finish
// ---- NEXT
// --- NEXT
// - [ ] paint selected mission before flying it! and then take off t button to start
// - [ ] vehicle selection screen first? - just use CLI flag or default
// - [ ] ratatea re-evaluate subscriptions
// - [ ] post mission stops telemetry? - more like when battery abort telemetry stops changing?
// - [ ] "connection lost" warning or whatever when unplugged
// - [ ] build mission planner
// - [ ] nix for bulding executable
// - [ ] polish: only start with flowdeck, warn on non supporting terminal free flight, set bounds and prevent out of bounds
//     - [ ] free flight not selectable in terminals that do not support
// - [ ] potentially re-center map to match around drone and real room
