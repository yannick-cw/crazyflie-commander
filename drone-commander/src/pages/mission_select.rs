use crate::pages::mission_select::Msg::*;
use crate::program::NavigationMessage;
use crate::program::NavigationMessage::{Down, Select, Up};
use crossterm::event::{KeyCode, KeyEvent};
use drone_control::Command;
use futures::StreamExt;
use ratatea::Cmd;
use std::io::Error;
use std::path::Path;
use tokio::fs;
use tokio::fs::DirEntry;
use tokio_stream::wrappers::ReadDirStream;
use tracing::warn;

// model ------------------------------------
#[derive(Debug)]
pub struct Model {
    pub missions: Vec<(String, Vec<Command>)>,
    pub recorded_missions: Vec<(String, Vec<Command>)>,
    pub selection: usize,
}
impl Default for Model {
    fn default() -> Self {
        Model {
            missions: Vec::new(),
            recorded_missions: Vec::new(),
            selection: 0,
        }
    }
}

// msg ------------------------------------
#[derive(Clone, Debug)]
pub enum Msg {
    LoadMissions,
    MissionsLoaded(Vec<(String, Vec<Command>)>, Vec<(String, Vec<Command>)>),
    Nav(NavigationMessage),
    ExitSelected(Vec<Command>, String),
    ExitPage,
}

// update ------------------------------------
pub fn update(model: &mut Model, msg: Msg) -> Cmd<Msg> {
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
            async {
                (
                    read_missions("missions").await,
                    read_missions("missions/recordings").await,
                )
            },
            |(m, rm)| Msg::MissionsLoaded(m, rm),
        ),
        // ---- handle by parent
        ExitSelected(_, _) => Cmd::none(),
        ExitPage => Cmd::none(),
    }
}

// utility -------------------------------
// relative path e.g. missions
async fn read_missions(path: &str) -> Vec<(String, Vec<Command>)> {
    match fs::read_dir(Path::new("./drone-commander").join(path)).await {
        Ok(dir) => {
            ReadDirStream::new(dir)
                .filter_map(|entry| async {
                    match read_file(&entry.ok()?).await {
                        Ok(Some(mission)) => Some(mission),
                        Ok(None) => None,
                        Err(e) => {
                            warn!("skipping: {e}");
                            None
                        }
                    }
                })
                .collect()
                .await
        }
        Err(err) => {
            warn!("Could not load any missions {err}");
            vec![]
        }
    }
}

async fn read_file(entry: &DirEntry) -> Result<Option<(String, Vec<Command>)>, Error> {
    let file_path = entry.path();
    if entry.file_type().await?.is_file() && file_path.extension() == Some("json".as_ref()) {
        let file_content = fs::read_to_string(&file_path).await?;

        let file_name = file_path.file_stem().and_then(|s| s.to_str()).unwrap();

        let mission: Vec<Command> = serde_json::from_str(&file_content)?;
        Ok(Some((file_name.to_owned(), mission)))
    } else {
        Ok(None)
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
