use crate::external::mission_service::MissionService;
use crate::ground_data_link::vehicle_link::VehicleLink;
use crate::pages::home::ModeSelection;
use crate::pages::manual_control::Msg::CommandSet;
use crate::pages::manual_control::SetpointRecording;
use crate::pages::mission_monitor::Msg::MissionUpdate;
use crate::pages::{home, manual_control, mission_monitor, mission_select};
use crate::program::NavigationMessage::*;
use crate::view::{flight_view, home_view, mission_select_view};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use datalink::domain_types::{OccupancyGrid, Telemetry, VehicleHealth};
use futures::StreamExt;
use mission_computer::Autopilot;
use ratatea::{Cmd, Ratatea, Sub};
use ratatui::Frame;
use std::rc::Rc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::{UnboundedReceiverStream, WatchStream};

// model ------------------------------------------------
#[derive(Debug)]
pub struct Model {
    pub telemetry: Telemetry,
    pub health: VehicleHealth,
    pub grid: OccupancyGrid,
    pub terminal_supports_enhancements: bool,
    pub exit: bool,
    pub state: State,
}
#[derive(Debug)]
pub enum State {
    Home(home::Model),
    MissionExecution(mission_monitor::Model),
    // this opens a selection view
    MissionSelect(mission_select::Model),
    // MissionPlan(),
    // this will go to "current" observe only for now
    ManualControl(manual_control::Model),
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
    HealthUpdate(VehicleHealth),
    // grid is a bit larger - better to box and have on the heap
    GridUpdate(OccupancyGrid),
    Key(KeyEvent),
    Resize,
    Quit,
    Home(home::Msg),
    MissionSelect(mission_select::Msg),
    MissionExecution(mission_monitor::Msg),
    ManualControl(manual_control::Msg),
}

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum NavigationMessage {
    Up,
    Down,
    Select,
}

pub struct Program<U: Autopilot + 'static> {
    command_unit: &'static U,
    vehicle_link: VehicleLink,
    terminal_supports_enhancements: bool,
    // needs to outlive the places it's shared to go into cmds - though not share between threads
    mission_loader: Rc<dyn MissionService>,
}

impl<U: Autopilot> Program<U> {
    pub fn new(
        command_unit: &'static U,
        vehicle_link: VehicleLink,
        terminal_supports_enhancements: bool,
        loader: Rc<dyn MissionService>,
    ) -> Self {
        Self {
            command_unit,
            vehicle_link,
            terminal_supports_enhancements,
            mission_loader: loader,
        }
    }
}

impl<U: Autopilot> Ratatea for Program<U> {
    type Model = Model;
    type Msg = Msg;

    fn init(&self) -> (Self::Model, Cmd<Self::Msg>) {
        (
            Model {
                telemetry: Default::default(),
                health: Default::default(),
                grid: vec![vec![]],
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
            (_, Msg::GridUpdate(grid)) => {
                model.grid = grid;
                (model, Cmd::none())
            }
            (s, Msg::TelemetryUpdate(tele)) => {
                model.telemetry = tele;
                if let State::ManualControl(flight_state) = s
                    && flight_state.is_recording
                {
                    // todo this is a bit brittle right now - these setpoints will be replayed at 100hz
                    // so this relies on telemetry coming in at 100hz
                    flight_state.recording.push(SetpointRecording {
                        x: tele.x,
                        y: tele.y,
                        z: tele.z,
                        yaw_degrees: tele.yaw_degrees,
                    });
                };
                (model, Cmd::none())
            }
            (_, Msg::HealthUpdate(health)) => {
                model.health = health;
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
                    ModeSelection::ManualControl if model.terminal_supports_enhancements => {
                        let (motion_sender, motion_receiver) = mpsc::unbounded_channel();
                        let commands = UnboundedReceiverStream::new(motion_receiver);
                        let h = tokio::spawn(command_unit.fly(commands));
                        (
                            State::ManualControl(manual_control::Model::new(motion_sender)),
                            Cmd::new(h, |_| Msg::ManualControl(CommandSet)),
                        )
                    }
                    ModeSelection::ManualControl => (model.state, Cmd::none()),
                };
                model.state = new_state;
                (model, cmd)
            }
            (_, Msg::MissionSelect(mission_select::Msg::ExitSelected(mission, name))) => {
                let execution_state = mission_monitor::Model::new(mission, name);
                model.state = State::MissionExecution(execution_state);
                (model, Cmd::none())
            }
            (
                _,
                Msg::MissionExecution(mission_monitor::Msg::ExitPage)
                | Msg::MissionSelect(mission_select::Msg::ExitPage)
                | Msg::ManualControl(manual_control::Msg::ExitPage),
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
                    mission_select::update(select_state, msg, self.mission_loader.clone())
                        .lift_msg(Msg::MissionSelect);
                (model, next_cmd)
            }
            (State::MissionExecution(state), Msg::MissionExecution(msg)) => {
                let next_cmd = mission_monitor::update(command_unit, state, msg)
                    .lift_msg(Msg::MissionExecution);
                (model, next_cmd)
            }
            (State::ManualControl(state), Msg::ManualControl(msg)) => {
                let next_msg = manual_control::update(state, msg, self.mission_loader.clone())
                    .lift_msg(Msg::ManualControl);
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
            State::ManualControl(_) => flight_view::view(model, frame),
        };
    }

    fn subscriptions(&self, _m: &Model) -> Sub<Self::Msg> {
        {
            vec![
                WatchStream::new(self.vehicle_link.latest_telemetry.subscribe())
                    .map(Msg::TelemetryUpdate)
                    .boxed(),
                WatchStream::new(self.vehicle_link.latest_health.subscribe())
                    .map(Msg::HealthUpdate)
                    .boxed(),
                WatchStream::new(self.vehicle_link.latest_grid.subscribe())
                    .map(|g| Msg::GridUpdate(g))
                    .boxed(),
                WatchStream::new(self.vehicle_link.latest_status.subscribe())
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
        (_, State::ManualControl(s)) => {
            manual_control::map_key_evt(key_event, s).lift_msg(Msg::ManualControl)
        }
        (_, State::MissionExecution(s)) => {
            mission_monitor::map_key_evt(key_event, s).lift_msg(Msg::MissionExecution)
        }
    }
}
