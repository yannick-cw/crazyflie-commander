use crate::program::NavigationMessage;
use crate::program::NavigationMessage::{Down, Select, Up};
use Msg::Nav;
use crossterm::event::{KeyCode, KeyEvent};
use ratatea::Cmd;

// model -----------------------------------------
#[derive(Debug)]
pub struct Model {
    pub selected_mode: ModeSelection,
}

#[derive(Debug, PartialEq)]
pub enum ModeSelection {
    MissionSelect,
    MissionPlan,
    FreeFlight,
}

impl ModeSelection {
    pub fn next(&self) -> Self {
        match self {
            ModeSelection::MissionSelect => ModeSelection::MissionPlan,
            ModeSelection::MissionPlan => ModeSelection::FreeFlight,
            ModeSelection::FreeFlight => ModeSelection::MissionSelect,
        }
    }
    pub fn prev(&self) -> Self {
        match self {
            ModeSelection::MissionSelect => ModeSelection::FreeFlight,
            ModeSelection::MissionPlan => ModeSelection::MissionSelect,
            ModeSelection::FreeFlight => ModeSelection::MissionPlan,
        }
    }
}

// msg -----------------------------------------
#[derive(Debug, Clone)]
pub enum Msg {
    Nav(NavigationMessage),
}

// update -----------------------------------------
pub fn update(model: &mut Model, msg: Msg) -> Cmd<Msg> {
    match msg {
        Nav(NavigationMessage::Up) => {
            model.selected_mode = model.selected_mode.prev();
            Cmd::none()
        }
        Nav(NavigationMessage::Down) => {
            model.selected_mode = model.selected_mode.next();
            Cmd::none()
        }
        // handled by parent - transition out
        Nav(NavigationMessage::Select) => Cmd::none(),
    }
}

pub fn map_key_evt(k: KeyEvent, _s: &Model) -> Cmd<Msg> {
    match k.code {
        KeyCode::Char('j') | KeyCode::Down if k.is_press() => Cmd::pure(Nav(Down)),
        KeyCode::Char('k') | KeyCode::Up if k.is_press() => Cmd::pure(Nav(Up)),
        KeyCode::Enter if k.is_press() => Cmd::pure(Nav(Select)),
        _ => Cmd::none(),
    }
}
