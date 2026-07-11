use crate::messages::{MissionSelectMessage, Msg, NavigationMessage};
use crate::model::{HomeState, MissionPlan, MissionSelectState, Model, State};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatea::Cmd;

fn navigation_cmd(state: &State, nav: NavigationMessage) -> Cmd<Msg> {
    match state {
        State::Home(_) => Cmd::pure(Msg::Home(nav)),
        State::MissionSelect(_) => Cmd::pure(Msg::MissionSelect(MissionSelectMessage::Nav(nav))),
        _ => Cmd::none(),
    }
}

pub fn update_all(msg: Msg, model: Model) -> (Model, Cmd<Msg>) {
    let mut model: Model = model.clone();
    match (&model.state, msg) {
        // global message
        // ------------------------------------------------------------
        (_, Msg::TelemetryUpdate(tele)) => {
            model.telemetry = tele;
            (model, Cmd::none())
        }
        (_, Msg::Key(key_event)) => match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => (model, Cmd::pure(Msg::Quit)),
            KeyCode::Char('c') | KeyCode::Char('C')
                if key_event.modifiers == KeyModifiers::CONTROL =>
            {
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
            KeyCode::Enter => {
                let next_msg = navigation_cmd(&model.state, NavigationMessage::Select);
                (model, next_msg)
            }
            _ => (model, Cmd::none()),
        },
        (_, Msg::Quit) => {
            model.exit = true;
            (model, Cmd::none())
        }
        // ------------------------------------------------------------
        // communication towards parent to change view
        // ------------------------------------------------------------
        (_, Msg::Home(NavigationMessage::Select)) => {
            model.state = State::MissionSelect(MissionSelectState::default());
            (model, Cmd::none())
        }
        (_, Msg::MissionSelect(MissionSelectMessage::Selected(mission))) => {
            model.state = State::MissionExecution(mission);
            (model, Cmd::none())
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

// todo ergonomics of returning nested Cmd<MissionSelectMessage> are not nice yet
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
            let message = MissionSelectMessage::Selected(MissionPlan {
                mission: mission.clone(),
                name: name.clone(),
            });
            (model, Cmd::pure(message))
        }
        // handled by parent
        MissionSelectMessage::Selected(_) => (model, Cmd::none()),
    }
}
