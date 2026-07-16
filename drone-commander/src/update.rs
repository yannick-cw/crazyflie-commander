use crate::messages::FreeFlightMessage::CommandSet;
use crate::messages::{
    FreeFlightMessage, MissionExecutionMessage, MissionSelectMessage, Msg, NavigationMessage,
};
use crate::model::Movement::{GoHome, Land, SpeedDown, SpeedUp, Start, Vx, Vy, YawRate};
use crate::model::{
    FreeFlightState, HomeState, MissionExecutionState, MissionSelectState, ModeSelection, Model,
    Movement, SetpointRecording, State,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use drone_control::{
    Abort, CommandUnit, MetersPerSecond, MotionCommand, Reason, SetpointHover, Telemetry,
};
use drone_control::{Command, Meters};
use futures::StreamExt;
use ratatea::Cmd;
use std::io::Error;
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::fs::DirEntry;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::{ReadDirStream, UnboundedReceiverStream};
use tracing::{info, warn};

pub fn update_all(
    command_unit: &'static impl CommandUnit,
    msg: Msg,
    m: Model,
) -> (Model, Cmd<Msg>) {
    let mut model: Model = m;
    match (&mut model.state, msg) {
        // global message
        // ------------------------------------------------------------
        (
            s,
            Msg::TelemetryUpdate(
                tele @ Telemetry {
                    x,
                    y,
                    z,
                    yaw_degrees: yaw,
                    ..
                },
            ),
        ) => {
            model.telemetry = tele;
            if let State::FreeFlight(flight_state) = s
                && flight_state.is_recording
            {
                // todo this is a bit brittle right now - these setpoints will be replayed at 100hz
                // so this relies on telemetry coming in at 100hz
                flight_state.recording.push(SetpointRecording {
                    x,
                    y,
                    z,
                    yaw_degrees: yaw,
                });
            };
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
        (_, Msg::ToHomeScreen) => (
            Model {
                state: State::default(),
                ..model
            },
            Cmd::none(),
        ),
        // ------------------------------------------------------------
        // communication towards parent to change view
        // ------------------------------------------------------------
        (State::Home(HomeState { selected_mode }), Msg::Home(NavigationMessage::Select)) => {
            let (new_state, cmd) = match selected_mode {
                ModeSelection::MissionSelectItem => (
                    State::MissionSelect(MissionSelectState::default()),
                    Cmd::new(
                        async {
                            (
                                read_missions("missions").await,
                                read_missions("missions/recordings").await,
                            )
                        },
                        |(m, rm)| Msg::MissionSelect(MissionSelectMessage::MissionsLoaded(m, rm)),
                    ),
                ),
                ModeSelection::MissionPlanItem => (model.state, Cmd::none()),
                ModeSelection::FreeFlightItem if model.terminal_supports_enhancements => {
                    let (motion_sender, motion_receiver) = mpsc::unbounded_channel();
                    let commands = UnboundedReceiverStream::new(motion_receiver);
                    (
                        State::FreeFlight(FreeFlightState {
                            vx: Default::default(),
                            vy: Default::default(),
                            z: Default::default(),
                            motion_sender,
                            is_airborne: false,
                            is_recording: false,
                            speed_setting: MetersPerSecond(1.0),
                            yaw_rate: 0.0,
                            yaw_rate_setting: 150.0,
                            recording: vec![],
                        }),
                        Cmd::new(command_unit.fly(commands), |_| Msg::FreeFlight(CommandSet)),
                    )
                }
                ModeSelection::FreeFlightItem => (model.state, Cmd::none()),
            };
            model.state = new_state;
            (model, cmd)
        }
        (_, Msg::MissionSelect(MissionSelectMessage::Selected(mission, name))) => {
            let execution_state = MissionExecutionState {
                mission,
                name,
                abort_sender: None,
                mission_status: drone_control::MissionStatus::Idle,
            };
            model.state = State::MissionExecution(execution_state);
            (model, Cmd::none())
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
    let total_missions = model.missions.len() + model.recorded_missions.len();
    match msg {
        MissionSelectMessage::Nav(NavigationMessage::Down) if total_missions > 0 => {
            model.selection = (model.selection + 1).min(total_missions - 1);
            Cmd::none()
        }
        MissionSelectMessage::Nav(NavigationMessage::Up) if total_missions > 0 => {
            model.selection = model.selection.saturating_sub(1);
            Cmd::none()
        }
        // sends message out
        MissionSelectMessage::Nav(NavigationMessage::Select) if total_missions > 0 => {
            let (name, mission) = model
                .missions
                .iter()
                .chain(&model.recorded_missions)
                .nth(model.selection)
                .unwrap();
            let message = MissionSelectMessage::Selected(mission.clone(), name.clone());
            Cmd::pure(message)
        }
        MissionSelectMessage::Nav(_) => Cmd::none(),
        // handled by parent
        MissionSelectMessage::Selected(_, _) => Cmd::none(),
        MissionSelectMessage::MissionsLoaded(missions, recorded_m) => {
            model.missions = missions;
            model.recorded_missions = recorded_m;
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
        FreeFlightMessage::StartRecording => {
            model.is_recording = true;
            Cmd::none()
        }
        FreeFlightMessage::StopRecording => {
            let recording = std::mem::take(&mut model.recording);
            model.is_recording = false;
            Cmd::new(store_recoding(recording), |_| CommandSet)
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
        KeyCode::Char('c') | KeyCode::Char('C')
            if key_event.modifiers == KeyModifiers::CONTROL && key_event.is_press() =>
        {
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
        KeyCode::Char('r') if key_event.is_press() => match &model.state {
            State::FreeFlight(flight_state) if !flight_state.is_recording => {
                Cmd::pure(Msg::FreeFlight(FreeFlightMessage::StartRecording))
            }
            State::FreeFlight(flight_state) if flight_state.is_recording => {
                Cmd::pure(Msg::FreeFlight(FreeFlightMessage::StopRecording))
            }
            _ => Cmd::none(),
        },
        KeyCode::Char('t') if key_event.is_press() => match &model.state {
            State::MissionExecution(MissionExecutionState {
                mission_status:
                    drone_control::MissionStatus::Idle
                    | drone_control::MissionStatus::Aborted(Reason::Landing),
                ..
            }) => Cmd::pure(Msg::MissionExecution(MissionExecutionMessage::StartMission)),
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

async fn store_recoding(recording: Vec<SetpointRecording>) {
    if let Some(first_p) = recording.first() {
        let z = recording.last().map(|p| p.z.0).unwrap_or(2.0);
        let land_duration = Duration::from_secs_f32(z.clamp(0.0, 2.0));

        let mission = vec![
            Command::Takeoff {
                height: Meters(0.5),
                duration: Duration::from_secs(1),
            },
            Command::MoveToWaypoint {
                x: first_p.x,
                y: first_p.y,
                z: first_p.z,
                duration: Duration::from_secs(2),
            },
            Command::Setpoints {
                points: recording.iter().map(|p| p.to_setpoint()).collect(),
            },
            Command::Land {
                duration: land_duration,
            },
        ];

        let mission_name = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

        match fs::write(
            format!("./drone-commander/missions/recordings/flight-{mission_name}.json"),
            serde_json::to_string(&mission).unwrap(),
        )
        .await
        {
            Ok(_) => info!("stored new recording"),
            Err(err) => warn!("could not safe recording {err}"),
        }
    } else {
        warn!("Trying store empty recording")
    }
}
