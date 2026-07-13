use crate::messages::{MissionExecutionMessage, MissionSelectMessage, Msg, NavigationMessage};
use crate::model::{
    HomeState, MissionExecutionState, MissionSelectState, ModeSelection, Model, State,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use drone_control::{Abort, CommandUnit};
use ratatea::Cmd;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

pub fn update_all(
    command_unit: &'static impl CommandUnit,
    msg: Msg,
    model: Model,
) -> (Model, Cmd<Msg>) {
    let mut model: Model = model.clone();
    match (&model.state, msg) {
        // global message
        // ------------------------------------------------------------
        (_, Msg::TelemetryUpdate(tele)) => {
            model.telemetry = tele;
            (model, Cmd::none())
        }
        // key events
        (_, Msg::Key(key_event)) => update_key_evt(key_event, model),
        (_, Msg::Quit) => {
            model.exit = true;
            (model, Cmd::none())
        }
        // ------------------------------------------------------------
        // communication towards parent to change view
        // ------------------------------------------------------------
        (
            State::Home(HomeState {
                selected_mode: ModeSelection::MissionSelectItem,
            }),
            Msg::Home(NavigationMessage::Select),
        ) => {
            model.state = State::MissionSelect(MissionSelectState::default());
            (model, Cmd::none())
        }
        (_, Msg::MissionSelect(MissionSelectMessage::Selected(mission))) => {
            model.state = State::MissionExecution(mission);
            (
                model,
                Cmd::pure(Msg::MissionExecution(MissionExecutionMessage::StartMission)),
            )
        }
        // ------------------------------------------------------------
        (State::Home(home_state), Msg::Home(msg)) => {
            let (new_home, next_home_msg) = update_home(&home_state, msg);
            model.state = State::Home(new_home);
            (model, next_home_msg)
        }
        (State::MissionSelect(select_state), Msg::MissionSelect(msg)) => {
            let (select, next_home_msg) = update_mission_select(&select_state, msg);
            model.state = State::MissionSelect(select);
            let next_msg = next_home_msg.lift_msg(Msg::MissionSelect);
            (model, next_msg)
        }
        (State::MissionExecution(state), Msg::MissionExecution(msg)) => {
            let (select, next_msg) = update_mission_execution(command_unit, &state, msg);
            model.state = State::MissionExecution(select);
            let next_msg = next_msg.lift_msg(Msg::MissionExecution);
            (model, next_msg)
        }
        (&State::MissionExecution(_), Msg::MissionSelect(_))
        | (&State::MissionPlan(), Msg::MissionSelect(_))
        | (&State::FreeFlight(), Msg::MissionSelect(_)) => (model, Cmd::none()),
        _ => (model, Cmd::none()),
    }
}

fn update_home(model: &HomeState, msg: NavigationMessage) -> (HomeState, Cmd<Msg>) {
    let mut model = model.clone();
    match msg {
        NavigationMessage::Up => {
            model.selected_mode = model.selected_mode.prev();
            (model, Cmd::none())
        }
        NavigationMessage::Down => {
            model.selected_mode = model.selected_mode.next();
            (model, Cmd::none())
        }
        // handled by parent - transition out
        NavigationMessage::Select => (model, Cmd::none()),
    }
}

fn update_mission_select(
    model: &MissionSelectState,
    msg: MissionSelectMessage,
) -> (MissionSelectState, Cmd<MissionSelectMessage>) {
    let mut model = model.clone();
    let total_missions = model.missions.len();
    match msg {
        MissionSelectMessage::Nav(NavigationMessage::Down) => {
            model.selection = (model.selection + 1).min(total_missions - 1);
            (model, Cmd::none())
        }
        MissionSelectMessage::Nav(NavigationMessage::Up) => {
            model.selection = model.selection.saturating_sub(1);
            (model, Cmd::none())
        }
        // sends message out
        MissionSelectMessage::Nav(NavigationMessage::Select) => {
            let (name, mission) = &model.missions[model.selection];
            let message = MissionSelectMessage::Selected(MissionExecutionState {
                mission: mission.clone(),
                name: name.clone(),
                abort_sender: None,
            });
            (model, Cmd::pure(message))
        }
        // handled by parent
        MissionSelectMessage::Selected(_) => (model, Cmd::none()),
    }
}

fn update_mission_execution(
    command_unit: &'static impl CommandUnit,
    model: &MissionExecutionState,
    msg: MissionExecutionMessage,
) -> (MissionExecutionState, Cmd<MissionExecutionMessage>) {
    match msg {
        MissionExecutionMessage::StartMission => {
            let mission = model.mission.clone();
            let (sender, mut receiver) = mpsc::channel(64);
            let mission = command_unit.run_mission(mission, async move { receiver.recv().await });

            (
                MissionExecutionState {
                    abort_sender: Some(sender),
                    ..model.clone()
                },
                Cmd::new(mission, |_| MissionExecutionMessage::MissionResult),
            )
        }
        MissionExecutionMessage::MissionResult => (model.clone(), Cmd::none()),
        MissionExecutionMessage::SafeLand => {
            let sender = model.abort_sender.clone();
            match sender {
                None => (model.clone(), Cmd::none()),
                Some(s) => {
                    let signal = async move { s.send(Abort::Land).await };
                    (
                        model.clone(),
                        Cmd::new(signal, |_| MissionExecutionMessage::MissionResult),
                    )
                }
            }
        }
        MissionExecutionMessage::EmergencyAbort => {
            let sender = model.abort_sender.clone();
            match sender {
                None => (model.clone(), Cmd::none()),
                Some(s) => {
                    let signal = async move { s.send(Abort::HardStop).await };
                    (
                        model.clone(),
                        Cmd::new(signal, |_| MissionExecutionMessage::MissionResult),
                    )
                }
            }
        }
    }
}

fn update_key_evt(key_event: KeyEvent, model: Model) -> (Model, Cmd<Msg>) {
    match key_event.code {
        KeyCode::Esc | KeyCode::Char('q') => (model, Cmd::pure(Msg::Quit)),
        KeyCode::Char('c') | KeyCode::Char('C') if key_event.modifiers == KeyModifiers::CONTROL => {
            (model, Cmd::pure(Msg::Quit))
        }
        KeyCode::Char('j') | KeyCode::Down => {
            let next_msg = navigation_cmd(&model.state, NavigationMessage::Down);
            (model, next_msg)
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let next_msg = navigation_cmd(&model.state, NavigationMessage::Up);
            (model, next_msg)
        }
        KeyCode::Char('l') => {
            let next_msg = match model.state {
                State::MissionExecution(_) => {
                    Cmd::pure(Msg::MissionExecution(MissionExecutionMessage::SafeLand))
                }
                _ => Cmd::none(),
            };
            (model, next_msg)
        }
        KeyCode::Char('x') => {
            let next_msg = match model.state {
                State::MissionExecution(_) => Cmd::pure(Msg::MissionExecution(
                    MissionExecutionMessage::EmergencyAbort,
                )),
                _ => Cmd::none(),
            };
            (model, next_msg)
        }
        KeyCode::Enter => {
            let next_msg = navigation_cmd(&model.state, NavigationMessage::Select);
            (model, next_msg)
        }
        _ => (model, Cmd::none()),
    }
}

fn navigation_cmd(state: &State, nav: NavigationMessage) -> Cmd<Msg> {
    match state {
        State::Home(_) => Cmd::pure(Msg::Home(nav)),
        State::MissionSelect(_) => Cmd::pure(Msg::MissionSelect(MissionSelectMessage::Nav(nav))),
        _ => Cmd::none(),
    }
}
