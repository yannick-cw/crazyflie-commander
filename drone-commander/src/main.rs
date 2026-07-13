use crate::messages::Msg;
use crate::model::{Model, State};
use crate::update::update_all;
use crate::view::{flight_view, home_view, mission_select_view};
use crossterm::event::Event;
use drone_control::errors::Res;
use drone_control::{Abort, Command, CommandUnit, Meters, MetersPerSecond, Telemetry, setup_link};
use futures::StreamExt;
use ratatea::{Cmd, Ratatea, Sub, run};
use ratatui::prelude::*;
use std::time::Duration;
use tokio::sync::broadcast::Receiver;
use tokio::sync::watch;
use tokio::time::sleep;
use tokio::{select, spawn, time};
use tokio_stream::wrappers::WatchStream;

pub mod messages;
pub mod model;
pub mod update;
mod view;

struct DebugUnit;
impl CommandUnit for DebugUnit {
    async fn run_mission(
        &self,
        _mission: Vec<Command>,
        abort_signal: impl Future<Output = Option<Abort>>,
    ) -> Res<()> {
        Ok(select! {
            _ = sleep(Duration::from_secs(5))=> {},
            Some(_) = abort_signal=> {},
        })
    }

    fn telemetry(&self) -> Receiver<Telemetry> {
        todo!()
    }

    fn latest_telemetry(&self) -> watch::Receiver<Telemetry> {
        let (sender, receiver) = watch::channel(Telemetry::default());
        spawn(async move {
            let mut ticks = time::interval(Duration::from_millis(10));
            let mut tele = Telemetry::default();
            loop {
                ticks.tick().await;
                let j = || fastrand::f32() - 0.5;
                tele.x = tele.x + Meters(j());
                tele.y = tele.y + Meters(j());
                tele.z = tele.z + Meters(j());
                tele.x_v = tele.x_v + MetersPerSecond(j());
                tele.y_v = tele.y_v + MetersPerSecond(j());
                tele.yaw += j();
                sender.send(tele).unwrap();
            }
        });
        receiver
    }
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    Ok(match setup_link().await {
        Ok(real_unit) => {
            // selection process
            // this needs to live for the whole program
            let command_unit: &'static _ = Box::leak(Box::new(real_unit));

            let p = Program { command_unit };
            run(p).await?;
        }
        _ => {
            let p = Program {
                command_unit: &DebugUnit,
            };
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
            State::FreeFlight() => flight_view::view(model, frame),
        };
    }

    fn subscriptions(&self, _m: &Model) -> Sub<Self::Msg> {
        {
            vec![
                WatchStream::new(self.command_unit.latest_telemetry().clone())
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
// - [x] mission abort shortcuts + buttons (exit: x)
// ----
// - [ ] post mission stops telemetry? - more like when battery abort telemetry stops changing?
// - [ ] after mission show button to return to home screen - WORKS IFF mission is not ongoing
// - [ ] add mission state to telemetry and display + progress
// - [ ] give real time and steps estimates?
// - [ ] render position in x y z
// - [ ] "connection lost" warning or whatever when unplugged
// - [ ] show logs in log window or write to file
