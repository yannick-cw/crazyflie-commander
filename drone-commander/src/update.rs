use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::event::{Message, MissionSelectMessage, NavigationMessage};
use crate::model::{HomeState, MissionPlan, MissionSelectState, Model, State};

fn navigation_msg(state: &State, nav: NavigationMessage) -> Option<Message> {
    match state {
        State::Home(_) => Some(Message::Home(nav)),
        State::MissionSelect(_) => Some(Message::MissionSelect(MissionSelectMessage::Nav(nav))),
        _ => None,
    }
}

pub fn update(model: &Model, msg: Message) -> (Model, Option<Message>) {
    let mut model: Model = model.clone();
    match (&model.state, msg) {
        // global message
        // ------------------------------------------------------------
        (_, Message::Tick(tele)) => {
            model.telemetry = tele;
            (model, None)
        }
        (_, Message::Key(key_event)) => match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => (model, Some(Message::Quit)),
            KeyCode::Char('c') | KeyCode::Char('C')
                if key_event.modifiers == KeyModifiers::CONTROL =>
            {
                (model, Some(Message::Quit))
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let next_msg = navigation_msg(&model.state, NavigationMessage::Down);
                (model, next_msg)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let next_msg = navigation_msg(&model.state, NavigationMessage::Up);
                (model, next_msg)
            }
            KeyCode::Enter => {
                let next_msg = navigation_msg(&model.state, NavigationMessage::Select);
                (model, next_msg)
            }
            _ => (model, None),
        },
        (_, Message::Quit) => {
            model.exit = true;
            (model, None)
        }
        // ------------------------------------------------------------
        // communication towards parent to change view
        // ------------------------------------------------------------
        (_, Message::Home(NavigationMessage::Select)) => {
            model.state = State::MissionSelect(MissionSelectState::default());
            (model, None)
        }
        (_, Message::MissionSelect(MissionSelectMessage::Selected(mission))) => {
            model.state = State::MissionExecution(mission);
            (model, None)
        }
        // ------------------------------------------------------------
        (State::Home(home_state), Message::Home(msg)) => {
            let (new_home, next_home_msg) = update_home(&home_state, msg);
            model.state = State::Home(new_home);
            (model, next_home_msg)
        }
        (State::MissionSelect(select_state), Message::MissionSelect(msg)) => {
            let (select, next_home_msg) = update_mission_select(&select_state, msg);
            model.state = State::MissionSelect(select);
            (model, next_home_msg.map(|m| Message::MissionSelect(m)))
        }
        (&State::MissionExecution(_), Message::MissionSelect(_))
        | (&State::MissionPlan(), Message::MissionSelect(_))
        | (&State::FreeFlight(), Message::MissionSelect(_)) => (model, None),
        _ => (model, None),
    }
}

fn update_home(model: &HomeState, msg: NavigationMessage) -> (HomeState, Option<Message>) {
    let mut model = model.clone();
    match msg {
        NavigationMessage::Up => {
            model.selected_mode = model.selected_mode.prev();
            (model, None)
        }
        NavigationMessage::Down => {
            model.selected_mode = model.selected_mode.next();
            (model, None)
        }
        // handled by parent - transition out
        NavigationMessage::Select => (model, None),
    }
}

fn update_mission_select(
    model: &MissionSelectState,
    msg: MissionSelectMessage,
) -> (MissionSelectState, Option<MissionSelectMessage>) {
    let mut model = model.clone();
    let total_missions = model.missions.len();
    match msg {
        MissionSelectMessage::Nav(NavigationMessage::Down) => {
            model.selection = (model.selection + 1).min(total_missions - 1);
            (model, None)
        }
        MissionSelectMessage::Nav(NavigationMessage::Up) => {
            model.selection = model.selection.saturating_sub(1);
            (model, None)
        }
        // sends message out
        MissionSelectMessage::Nav(NavigationMessage::Select) => {
            let (name, mission) = &model.missions[model.selection];
            let message = MissionSelectMessage::Selected(MissionPlan {
                mission: mission.clone(),
                name: name.clone(),
            });
            (model, Some(message))
        }
        // handled by parent
        MissionSelectMessage::Selected(_) => (model, None),
    }
}
