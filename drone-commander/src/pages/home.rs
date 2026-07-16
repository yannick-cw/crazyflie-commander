use crate::program::NavigationMessage;
use ratatea::Cmd;

// model -----------------------------------------
#[derive(Debug)]
pub struct Model {
    pub selected_mode: ModeSelection,
}

#[derive(Debug, PartialEq)]
pub enum ModeSelection {
    MissionSelectItem,
    MissionPlanItem,
    FreeFlightItem,
}

impl ModeSelection {
    pub fn next(&self) -> Self {
        match self {
            ModeSelection::MissionSelectItem => ModeSelection::MissionPlanItem,
            ModeSelection::MissionPlanItem => ModeSelection::FreeFlightItem,
            ModeSelection::FreeFlightItem => ModeSelection::MissionSelectItem,
        }
    }
    pub fn prev(&self) -> Self {
        match self {
            ModeSelection::MissionSelectItem => ModeSelection::FreeFlightItem,
            ModeSelection::MissionPlanItem => ModeSelection::MissionSelectItem,
            ModeSelection::FreeFlightItem => ModeSelection::MissionPlanItem,
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
        Msg::Nav(NavigationMessage::Up) => {
            model.selected_mode = model.selected_mode.prev();
            Cmd::none()
        }
        Msg::Nav(NavigationMessage::Down) => {
            model.selected_mode = model.selected_mode.next();
            Cmd::none()
        }
        // handled by parent - transition out
        Msg::Nav(NavigationMessage::Select) => Cmd::none(),
    }
}
