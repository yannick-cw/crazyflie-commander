use crate::external::mission_service::MissionService;
use crate::pages::mission_select::Msg::*;
use crate::program::NavigationMessage;
use crate::program::NavigationMessage::{Down, Select, Up};
use crossterm::event::{KeyCode, KeyEvent};
use mission_computer::MissionItem;
use ratatea::Cmd;
use std::rc::Rc;

// model ------------------------------------
#[derive(Debug, Default)]
pub struct Model {
    pub missions: Vec<(String, Vec<MissionItem>)>,
    pub recorded_missions: Vec<(String, Vec<MissionItem>)>,
    pub selection: usize,
}

// msg ------------------------------------
#[derive(Clone, Debug)]
pub enum Msg {
    LoadMissions,
    MissionsLoaded(
        Vec<(String, Vec<MissionItem>)>,
        Vec<(String, Vec<MissionItem>)>,
    ),
    Nav(NavigationMessage),
    ExitSelected(Vec<MissionItem>, String),
    ExitPage,
}

// update ------------------------------------
pub fn update(model: &mut Model, msg: Msg, mission_loader: Rc<dyn MissionService>) -> Cmd<Msg> {
    let total_missions = model.missions.len() + model.recorded_missions.len();
    match msg {
        Nav(Down) if total_missions > 0 => {
            model.selection = (model.selection + 1).min(total_missions - 1);
            Cmd::none()
        }
        Nav(Up) if total_missions > 0 => {
            model.selection = model.selection.saturating_sub(1);
            Cmd::none()
        }
        // sends message out
        Nav(Select) if total_missions > 0 => {
            let (name, mission) = model
                .missions
                .iter()
                .chain(&model.recorded_missions)
                .nth(model.selection)
                .unwrap();
            let message = ExitSelected(mission.clone(), name.clone());
            Cmd::pure(message)
        }
        Nav(_) => Cmd::none(),
        MissionsLoaded(missions, recorded_m) => {
            model.missions = missions;
            model.recorded_missions = recorded_m;
            Cmd::none()
        }
        LoadMissions => Cmd::new(
            async move {
                (
                    mission_loader.list_missions().await,
                    mission_loader.list_recordings().await,
                )
            },
            |(m, rm)| Msg::MissionsLoaded(m, rm),
        ),
        // ---- handle by parent
        ExitSelected(_, _) => Cmd::none(),
        ExitPage => Cmd::none(),
    }
}

pub fn map_key_evt(k: KeyEvent, _s: &Model) -> Cmd<Msg> {
    match k.code {
        KeyCode::Char('j') | KeyCode::Down if k.is_press() => Cmd::pure(Nav(Down)),
        KeyCode::Char('k') | KeyCode::Up if k.is_press() => Cmd::pure(Nav(Up)),
        KeyCode::Enter if k.is_press() => Cmd::pure(Nav(Select)),
        KeyCode::Char('b') if k.is_press() => Cmd::pure(ExitPage),
        _ => Cmd::none(),
    }
}
