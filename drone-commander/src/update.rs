use crate::messages::FreeFlightMessage::CommandSet;
use crate::messages::{
    FreeFlightMessage, MissionExecutionMessage, MissionSelectMessage, Msg, NavigationMessage,
};
use crate::model::Movement::{GoHome, Land, SpeedDown, SpeedUp, Start, Vx, Vy, YawRate};
use crate::model::{
    FreeFlightState, HomeState, MissionExecutionState, MissionSelectState, ModeSelection, Model,
    Movement, State,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use drone_control::{Abort, CommandUnit, MetersPerSecond, MotionCommand, SetpointHover};
use drone_control::{Command, Meters};
use futures::StreamExt;
use ratatea::Cmd;
use std::io::Error;
use tokio::fs;
use tokio::fs::DirEntry;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::{ReadDirStream, UnboundedReceiverStream};
use tracing::warn;

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
        (_, Msg::ToHomeScreen) => (Model::default(), Cmd::none()),
        // ------------------------------------------------------------
        // communication towards parent to change view
        // ------------------------------------------------------------
        (State::Home(HomeState { selected_mode }), Msg::Home(NavigationMessage::Select)) => {
            let (new_state, cmd) = match selected_mode {
                ModeSelection::MissionSelectItem => (
                    State::MissionSelect(MissionSelectState::default()),
                    Cmd::new(read_missions(), |m| {
                        Msg::MissionSelect(MissionSelectMessage::MissionsLoaded(m))
                    }),
                ),
                ModeSelection::MissionPlanItem => (model.state, Cmd::none()),
                ModeSelection::FreeFlightItem => {
                    let (motion_sender, motion_receiver) = mpsc::unbounded_channel();
                    let commands = UnboundedReceiverStream::new(motion_receiver);
                    (
                        State::FreeFlight(FreeFlightState {
                            vx: Default::default(),
                            vy: Default::default(),
                            z: Default::default(),
                            motion_sender,
                            is_airborne: false,
                            speed_setting: MetersPerSecond(1.0),
                            yaw_rate: 0.0,
                            yaw_rate_setting: 150.0,
                        }),
                        Cmd::new(command_unit.fly(commands), |_| Msg::FreeFlight(CommandSet)),
                    )
                }
            };
            model.state = new_state;
            (model, cmd)
        }
        (_, Msg::MissionSelect(MissionSelectMessage::Selected(mission, name))) => {
            let execution_state = MissionExecutionState {
                mission,
                name,
                abort_sender: None,
                mission_status: drone_control::MissionStatus::Running(None),
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
        (State::FreeFlight(state), Msg::FreeFlight(msg)) => {
            let next_msg = update_free_flight(state, msg);
            let next_msg = next_msg.lift_msg(Msg::FreeFlight);
            (model, next_msg)
        }
        (State::MissionPlan(), Msg::MissionSelect(_)) => (model, Cmd::none()),
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
        MissionSelectMessage::MissionsLoaded(missions) => {
            model.missions = missions;
            Cmd::none()
        }
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
        MissionExecutionMessage::SafeLand => abort_mission(model, Abort::Land),
        MissionExecutionMessage::EmergencyAbort => abort_mission(model, Abort::HardStop),
        MissionExecutionMessage::MissionUpdate(update) => {
            model.mission_status = update;
            Cmd::none()
        }
    }
}

fn abort_mission(model: &mut MissionExecutionState, signal: Abort) -> Cmd<MissionExecutionMessage> {
    match model.abort_sender.take() {
        None => Cmd::none(),
        Some(s) => {
            let signal = async move { s.send(signal) };
            Cmd::new(signal, |_| MissionExecutionMessage::MissionResult)
        }
    }
}

fn update_free_flight(
    model: &mut FreeFlightState,
    msg: FreeFlightMessage,
) -> Cmd<FreeFlightMessage> {
    let sender = model.motion_sender.clone();
    match msg {
        FreeFlightMessage::Move(Vx(new_x)) => {
            model.vx = new_x;
            Cmd::pure(FreeFlightMessage::SendNextMove)
        }
        FreeFlightMessage::Move(Vy(new_y)) => {
            model.vy = new_y;
            Cmd::pure(FreeFlightMessage::SendNextMove)
        }
        FreeFlightMessage::Move(YawRate(yaw_rate)) => {
            model.yaw_rate = yaw_rate;
            Cmd::pure(FreeFlightMessage::SendNextMove)
        }
        FreeFlightMessage::Move(Land) => {
            model.vx = MetersPerSecond(0.0);
            model.vy = MetersPerSecond(0.0);
            model.z = Meters(0.0);
            model.is_airborne = false;
            Cmd::new(async move { sender.send(MotionCommand::Land) }, |_| {
                CommandSet
            })
        }
        FreeFlightMessage::Move(GoHome) => {
            model.vx = MetersPerSecond(0.0);
            model.vy = MetersPerSecond(0.0);
            model.z = Meters(0.0);
            model.is_airborne = false;
            Cmd::new(async move { sender.send(MotionCommand::GoHome) }, |_| {
                CommandSet
            })
        }
        FreeFlightMessage::Move(Start) => {
            model.vx = MetersPerSecond(0.0);
            model.vy = MetersPerSecond(0.0);
            model.z = Meters(0.5);
            Cmd::new(
                async move { sender.send(MotionCommand::TakeOff(Meters(0.5))) },
                |_| FreeFlightMessage::TakeOffDone,
            )
        }
        FreeFlightMessage::SendNextMove if model.is_airborne => {
            let vx = model.vx;
            let vy = model.vy;
            let z = model.z;
            let yaw_rate = model.yaw_rate;
            Cmd::new(
                async move {
                    sender.send(MotionCommand::Move(SetpointHover {
                        vx,
                        vy,
                        z,
                        yaw_rate,
                    }))
                },
                |_| CommandSet,
            )
        }
        FreeFlightMessage::SendNextMove => Cmd::none(),
        CommandSet => Cmd::none(),
        FreeFlightMessage::Abort => {
            model.vx = MetersPerSecond(0.0);
            model.vy = MetersPerSecond(0.0);
            model.z = Meters(0.0);
            Cmd::new(async move { sender.send(MotionCommand::Stop) }, |_| {
                CommandSet
            })
        }
        FreeFlightMessage::TakeOffDone => {
            model.is_airborne = true;
            Cmd::none()
        }
        FreeFlightMessage::Move(SpeedUp) => {
            model.yaw_rate_setting += 10.0;
            model.speed_setting += MetersPerSecond(0.1);
            Cmd::none()
        }
        FreeFlightMessage::Move(SpeedDown) => {
            model.yaw_rate_setting -= 10.0;
            model.speed_setting -= MetersPerSecond(0.1);
            Cmd::none()
        }
    }
}

fn movement_from_key(k: KeyEvent, s: &FreeFlightState) -> Option<Movement> {
    let axis_speed = s.speed_setting;
    let zero_ms = MetersPerSecond(0.0);
    let yaw_rate = s.yaw_rate_setting;
    match (k.code, k.kind) {
        (KeyCode::Char('w'), KeyEventKind::Press) if s.vx <= zero_ms => Some(Vx(axis_speed)),
        (KeyCode::Char('w'), KeyEventKind::Release) => Some(Vx(zero_ms)),
        (KeyCode::Char('a'), KeyEventKind::Press) if s.vy <= zero_ms => Some(Vy(axis_speed)),
        (KeyCode::Char('a'), KeyEventKind::Release) => Some(Vy(zero_ms)),
        (KeyCode::Char('s'), KeyEventKind::Press) if s.vx >= zero_ms => Some(Vx(-axis_speed)),
        (KeyCode::Char('s'), KeyEventKind::Release) => Some(Vx(zero_ms)),
        (KeyCode::Char('d'), KeyEventKind::Press) if s.vy >= zero_ms => Some(Vy(-axis_speed)),
        (KeyCode::Char('d'), KeyEventKind::Release) => Some(Vy(zero_ms)),
        (KeyCode::Char('h'), KeyEventKind::Press) => Some(GoHome),
        (KeyCode::Left, KeyEventKind::Press) => Some(YawRate(yaw_rate)),
        (KeyCode::Right, KeyEventKind::Press) => Some(YawRate(-yaw_rate)),
        (KeyCode::Left, KeyEventKind::Release) => Some(YawRate(0.0)),
        (KeyCode::Right, KeyEventKind::Release) => Some(YawRate(0.0)),
        (KeyCode::Up, KeyEventKind::Press) => Some(SpeedUp),
        (KeyCode::Down, KeyEventKind::Press) => Some(SpeedDown),
        _ => None,
    }
}

fn update_key_evt(key_event: KeyEvent, model: &Model) -> Cmd<Msg> {
    match key_event.code {
        // movement keys in flight mode
        k if ['w', 'a', 's', 'd', 'h'].into_iter().any(|c| k.is_char(c))
            | k.is_left()
            | k.is_right()
            | k.is_down()
            | k.is_up() =>
        {
            match &model.state {
                State::FreeFlight(s) => movement_from_key(key_event, s)
                    .map(|m| Cmd::pure(Msg::FreeFlight(FreeFlightMessage::Move(m))))
                    .unwrap_or(Cmd::none()),
                _ => Cmd::none(),
            }
        }
        KeyCode::Esc | KeyCode::Char('q') if key_event.is_press() => Cmd::pure(Msg::Quit),
        KeyCode::Char('c') | KeyCode::Char('C') if key_event.modifiers == KeyModifiers::CONTROL => {
            Cmd::pure(Msg::Quit)
        }
        KeyCode::Char('j') | KeyCode::Down if key_event.is_press() => {
            navigation_cmd(&model.state, NavigationMessage::Down)
        }
        KeyCode::Char('k') | KeyCode::Up if key_event.is_press() => {
            navigation_cmd(&model.state, NavigationMessage::Up)
        }
        KeyCode::Char('l') if key_event.is_press() => match model.state {
            State::MissionExecution(_) => {
                Cmd::pure(Msg::MissionExecution(MissionExecutionMessage::SafeLand))
            }
            State::FreeFlight(_) => Cmd::pure(Msg::FreeFlight(FreeFlightMessage::Move(Land))),
            _ => Cmd::none(),
        },
        KeyCode::Char('b') if key_event.is_press() => match &model.state {
            State::MissionExecution(MissionExecutionState {
                mission_status:
                    drone_control::MissionStatus::Idle | drone_control::MissionStatus::Aborted(_),
                ..
            }) => Cmd::pure(Msg::ToHomeScreen),
            State::FreeFlight(FreeFlightState {
                is_airborne: false, ..
            }) => Cmd::pure(Msg::ToHomeScreen),
            State::MissionSelect(_) => Cmd::pure(Msg::ToHomeScreen),
            _ => Cmd::none(),
        },
        KeyCode::Char('x') if key_event.is_press() => match model.state {
            State::MissionExecution(_) => Cmd::pure(Msg::MissionExecution(
                MissionExecutionMessage::EmergencyAbort,
            )),
            State::FreeFlight(_) => Cmd::pure(Msg::FreeFlight(FreeFlightMessage::Abort)),
            _ => Cmd::none(),
        },
        KeyCode::Char('t') if key_event.is_press() => match model.state {
            State::FreeFlight(_) => Cmd::pure(Msg::FreeFlight(FreeFlightMessage::Move(Start))),
            _ => Cmd::none(),
        },
        KeyCode::Enter if key_event.is_press() => {
            navigation_cmd(&model.state, NavigationMessage::Select)
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

async fn read_missions() -> Vec<(String, Vec<Command>)> {
    match fs::read_dir("./drone-commander/missions").await {
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
