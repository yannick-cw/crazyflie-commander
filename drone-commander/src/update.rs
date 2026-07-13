use crate::messages::{MissionExecutionMessage, MissionSelectMessage, Msg, NavigationMessage};
use crate::model::{
    HomeState, MissionExecutionState, MissionSelectState, ModeSelection, Model, State,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use drone_control::{Abort, CommandUnit};
use ratatea::Cmd;
use tokio::sync::oneshot;

pub fn update_all(
    command_unit: &'static impl CommandUnit,
    msg: Msg,
    m: Model,
) -> (Model, Cmd<Msg>) {
    let mut model: Model = m;
    match (&mut model.state, msg) {
        // global message
        // ------------------------------------------------------------
        (_, Msg::TelemetryUpdate(tele)) => {
            model.telemetry = tele;
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
        (
            State::Home(HomeState {
                selected_mode: ModeSelection::MissionSelectItem,
            }),
            Msg::Home(NavigationMessage::Select),
        ) => {
            model.state = State::MissionSelect(MissionSelectState::default());
            (model, Cmd::none())
        }
        (_, Msg::MissionSelect(MissionSelectMessage::Selected(mission, name))) => {
            let execution_state = MissionExecutionState {
                mission,
                name,
                abort_sender: None,
            };
            model.state = State::MissionExecution(execution_state);
            (
                model,
                Cmd::pure(Msg::MissionExecution(MissionExecutionMessage::StartMission)),
            )
        }
        // sub state updates
        // ------------------------------------------------------------
        (State::Home(home_state), Msg::Home(msg)) => {
            let next_home_msg = update_home(home_state, msg);
            (model, next_home_msg)
        }
        (State::MissionSelect(select_state), Msg::MissionSelect(msg)) => {
            let next_home_msg = update_mission_select(select_state, msg);
            let next_msg = next_home_msg.lift_msg(Msg::MissionSelect);
            (model, next_msg)
        }
        (State::MissionExecution(state), Msg::MissionExecution(msg)) => {
            let next_msg = update_mission_execution(command_unit, state, msg);
            let next_msg = next_msg.lift_msg(Msg::MissionExecution);
            (model, next_msg)
        }
        (State::MissionPlan(), Msg::MissionSelect(_))
        | (State::FreeFlight(), Msg::MissionSelect(_)) => (model, Cmd::none()),
        _ => (model, Cmd::none()),
    }
}

fn update_home(model: &mut HomeState, msg: NavigationMessage) -> Cmd<Msg> {
    match msg {
        NavigationMessage::Up => {
            model.selected_mode = model.selected_mode.prev();
            Cmd::none()
        }
        NavigationMessage::Down => {
            model.selected_mode = model.selected_mode.next();
            Cmd::none()
        }
        // handled by parent - transition out
        NavigationMessage::Select => Cmd::none(),
    }
}

fn update_mission_select(
    model: &mut MissionSelectState,
    msg: MissionSelectMessage,
) -> Cmd<MissionSelectMessage> {
    let total_missions = model.missions.len();
    match msg {
        MissionSelectMessage::Nav(NavigationMessage::Down) => {
            model.selection = (model.selection + 1).min(total_missions - 1);
            Cmd::none()
        }
        MissionSelectMessage::Nav(NavigationMessage::Up) => {
            model.selection = model.selection.saturating_sub(1);
            Cmd::none()
        }
        // sends message out
        MissionSelectMessage::Nav(NavigationMessage::Select) => {
            let (name, mission) = &model.missions[model.selection];
            let message = MissionSelectMessage::Selected(mission.clone(), name.clone());
            Cmd::pure(message)
        }
        // handled by parent
        MissionSelectMessage::Selected(_, _) => Cmd::none(),
    }
}

fn update_mission_execution(
    command_unit: &'static impl CommandUnit,
    model: &mut MissionExecutionState,
    msg: MissionExecutionMessage,
) -> Cmd<MissionExecutionMessage> {
    match msg {
        MissionExecutionMessage::StartMission => {
            let mission = model.mission.clone();
            let (sender, receiver) = oneshot::channel();
            let mission =
                command_unit.run_mission(mission, async move { Some(receiver.await.unwrap()) });
            model.abort_sender = Some(sender);

            Cmd::new(mission, |_| MissionExecutionMessage::MissionResult)
        }
        MissionExecutionMessage::MissionResult => Cmd::none(),
        MissionExecutionMessage::SafeLand => match model.abort_sender.take() {
            None => Cmd::none(),
            Some(s) => {
                let signal = async move { s.send(Abort::Land) };
                Cmd::new(signal, |_| MissionExecutionMessage::MissionResult)
            }
        },
        MissionExecutionMessage::EmergencyAbort => match model.abort_sender.take() {
            None => Cmd::none(),
            Some(s) => {
                let signal = async move { s.send(Abort::HardStop) };
                Cmd::new(signal, |_| MissionExecutionMessage::MissionResult)
            }
        },
    }
}

fn update_key_evt(key_event: KeyEvent, model: &Model) -> Cmd<Msg> {
    match key_event.code {
        KeyCode::Esc | KeyCode::Char('q') => Cmd::pure(Msg::Quit),
        KeyCode::Char('c') | KeyCode::Char('C') if key_event.modifiers == KeyModifiers::CONTROL => {
            Cmd::pure(Msg::Quit)
        }
        KeyCode::Char('j') | KeyCode::Down => {
            let next_msg = navigation_cmd(&model.state, NavigationMessage::Down);
            next_msg
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let next_msg = navigation_cmd(&model.state, NavigationMessage::Up);
            next_msg
        }
        KeyCode::Char('l') => {
            let next_msg = match model.state {
                State::MissionExecution(_) => {
                    Cmd::pure(Msg::MissionExecution(MissionExecutionMessage::SafeLand))
                }
                _ => Cmd::none(),
            };
            next_msg
        }
        KeyCode::Char('x') => {
            let next_msg = match model.state {
                State::MissionExecution(_) => Cmd::pure(Msg::MissionExecution(
                    MissionExecutionMessage::EmergencyAbort,
                )),
                _ => Cmd::none(),
            };
            next_msg
        }
        KeyCode::Enter => {
            let next_msg = navigation_cmd(&model.state, NavigationMessage::Select);
            next_msg
        }
        _ => Cmd::none(),
    }
}

fn navigation_cmd(state: &State, nav: NavigationMessage) -> Cmd<Msg> {
    match state {
        State::Home(_) => Cmd::pure(Msg::Home(nav)),
        State::MissionSelect(_) => Cmd::pure(Msg::MissionSelect(MissionSelectMessage::Nav(nav))),
        _ => Cmd::none(),
    }
}
