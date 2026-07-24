use crate::pages::free_flight::Msg::CommandSet;
use crate::pages::free_flight::SetpointRecording;
use crate::pages::home::ModeSelection;
use crate::pages::mission_execution::Msg::MissionUpdate;
use crate::pages::{free_flight, home, mission_execution, mission_select};
use crate::program::NavigationMessage::*;
use crate::view::{flight_view, home_view, mission_select_view};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use drone_control::{CommandUnit, Telemetry};
use futures::StreamExt;
use ratatea::{Cmd, Ratatea, Sub};
use ratatui::Frame;
use tokio::sync::mpsc;
use tokio_stream::wrappers::{UnboundedReceiverStream, WatchStream};

// model ------------------------------------------------
#[derive(Debug)]
pub struct Model {
    pub telemetry: Telemetry,
    pub terminal_supports_enhancements: bool,
    pub exit: bool,
    pub state: State,
}
#[derive(Debug)]
pub enum State {
    Home(home::Model),
    MissionExecution(mission_execution::Model),
    // this opens a selection view
    MissionSelect(mission_select::Model),
    // MissionPlan(),
    // this will go to "current" observe only for now
    FreeFlight(free_flight::Model),
}
impl Default for State {
    fn default() -> Self {
        State::Home(home::Model {
            selected_mode: ModeSelection::MissionSelect,
        })
    }
}

// msg ---------------------------------------
#[derive(Debug)]
pub enum Msg {
    TelemetryUpdate(Telemetry),
    Key(KeyEvent),
    Resize,
    Quit,
    Home(home::Msg),
    MissionSelect(mission_select::Msg),
    MissionExecution(mission_execution::Msg),
    FreeFlight(free_flight::Msg),
}

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum NavigationMessage {
    Up,
    Down,
    Select,
}

pub struct Program<U: CommandUnit + 'static> {
    command_unit: &'static U,
    terminal_supports_enhancements: bool,
}

impl<U: CommandUnit> Program<U> {
    pub fn new(command_unit: &'static U, terminal_supports_enhancements: bool) -> Self {
        Self {
            command_unit,
            terminal_supports_enhancements,
        }
    }
}

impl<U: CommandUnit> Ratatea for Program<U> {
    type Model = Model;
    type Msg = Msg;

    fn init(&self) -> (Self::Model, Cmd<Self::Msg>) {
        (
            Model {
                telemetry: Default::default(),
                exit: false,
                terminal_supports_enhancements: self.terminal_supports_enhancements,
                state: State::default(),
            },
            Cmd::none(),
        )
    }

    fn update(&self, msg: Self::Msg, m: Self::Model) -> (Self::Model, Cmd<Self::Msg>) {
        let command_unit = self.command_unit;
        let mut model: Model = m;
        match (&mut model.state, msg) {
            (
                s,
                Msg::TelemetryUpdate(
                    tele @ Telemetry {
                        x,
                        y,
                        z,
                        yaw_degrees: yaw,
                        ..
                    },
                ),
            ) => {
                model.telemetry = tele;
                if let State::FreeFlight(flight_state) = s
                    && flight_state.is_recording
                {
                    // todo this is a bit brittle right now - these setpoints will be replayed at 100hz
                    // so this relies on telemetry coming in at 100hz
                    flight_state.recording.push(SetpointRecording {
                        x,
                        y,
                        z,
                        yaw_degrees: yaw,
                    });
                };
                (model, Cmd::none())
            }
            // key events
            (_, Msg::Key(key_event)) => {
                let key_cmd = update_key_evt(key_event, &model);
                (model, key_cmd)
            }
            (_, Msg::Quit) => {
                model.exit = true;
                (model, Cmd::none())
            }
            // ------------------------------------------------------------
            // communication towards parent to change view
            // ------------------------------------------------------------
            (State::Home(home::Model { selected_mode }), Msg::Home(home::Msg::Nav(Select))) => {
                let (new_state, cmd) = match selected_mode {
                    ModeSelection::MissionSelect => (
                        State::MissionSelect(mission_select::Model::default()),
                        Cmd::pure(Msg::MissionSelect(mission_select::Msg::LoadMissions)),
                    ),
                    ModeSelection::MissionPlan => (model.state, Cmd::none()),
                    ModeSelection::FreeFlight if model.terminal_supports_enhancements => {
                        let (motion_sender, motion_receiver) = mpsc::unbounded_channel();
                        let commands = UnboundedReceiverStream::new(motion_receiver);
                        (
                            State::FreeFlight(free_flight::Model::new(motion_sender)),
                            Cmd::new(command_unit.fly(commands), |_| Msg::FreeFlight(CommandSet)),
                        )
                    }
                    ModeSelection::FreeFlight => (model.state, Cmd::none()),
                };
                model.state = new_state;
                (model, cmd)
            }
            (_, Msg::MissionSelect(mission_select::Msg::ExitSelected(mission, name))) => {
                let execution_state = mission_execution::Model::new(mission, name);
                model.state = State::MissionExecution(execution_state);
                (model, Cmd::none())
            }
            (
                _,
                Msg::MissionExecution(mission_execution::Msg::ExitPage)
                | Msg::MissionSelect(mission_select::Msg::ExitPage)
                | Msg::FreeFlight(free_flight::Msg::ExitPage),
            ) => (
                Model {
                    state: State::default(),
                    ..model
                },
                Cmd::none(),
            ),
            // sub state updates
            // ------------------------------------------------------------
            (State::Home(home_state), Msg::Home(msg)) => {
                let home_cmd = home::update(home_state, msg).lift_msg(Msg::Home);
                (model, home_cmd)
            }
            (State::MissionSelect(select_state), Msg::MissionSelect(msg)) => {
                let next_cmd =
                    mission_select::update(select_state, msg).lift_msg(Msg::MissionSelect);
                (model, next_cmd)
            }
            (State::MissionExecution(state), Msg::MissionExecution(msg)) => {
                let next_cmd = mission_execution::update(command_unit, state, msg)
                    .lift_msg(Msg::MissionExecution);
                (model, next_cmd)
            }
            (State::FreeFlight(state), Msg::FreeFlight(msg)) => {
                let next_msg = free_flight::update(state, msg).lift_msg(Msg::FreeFlight);
                (model, next_msg)
            }
            // (State::MissionPlan(), _) => (model1, Cmd::none()),
            _ => (model, Cmd::none()),
        }
    }

    fn view(&self, model: &Self::Model, frame: &mut Frame) {
        match &model.state {
            State::Home(s) => home_view::view(s, model.terminal_supports_enhancements, frame),
            State::MissionExecution(_) => flight_view::view(model, frame),
            State::MissionSelect(s) => mission_select_view::view(s, frame),
            // State::MissionPlan() => {}
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
                    .map(|update| Msg::MissionExecution(MissionUpdate(update)))
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

fn update_key_evt(key_event: KeyEvent, model: &Model) -> Cmd<Msg> {
    match (key_event.code, &model.state) {
        (KeyCode::Esc | KeyCode::Char('q'), _) if key_event.is_press() => Cmd::pure(Msg::Quit),
        (KeyCode::Char('c') | KeyCode::Char('C'), _)
            if key_event.modifiers == KeyModifiers::CONTROL && key_event.is_press() =>
        {
            Cmd::pure(Msg::Quit)
        }
        (_, State::MissionSelect(s)) => {
            mission_select::map_key_evt(key_event, s).lift_msg(Msg::MissionSelect)
        }
        (_, State::Home(s)) => home::map_key_evt(key_event, s).lift_msg(Msg::Home),
        (_, State::FreeFlight(s)) => {
            free_flight::map_key_evt(key_event, s).lift_msg(Msg::FreeFlight)
        }
        (_, State::MissionExecution(s)) => {
            mission_execution::map_key_evt(key_event, s).lift_msg(Msg::MissionExecution)
        }
    }
}
