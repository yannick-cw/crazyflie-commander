use crate::pages::free_flight::Movement::*;
use crate::pages::free_flight::Msg::CommandSet;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use drone_control::{Command, Meters, MetersPerSecond, MotionCommand, Setpoint, SetpointHover};
use ratatea::Cmd;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::fs;
use tokio::sync::mpsc;
use tracing::{info, warn};

// model -----------------
#[derive(Debug, Serialize, Deserialize)]
pub struct SetpointRecording {
    pub x: Meters,
    pub y: Meters,
    pub z: Meters,
    pub yaw_degrees: f32,
}
impl SetpointRecording {
    pub fn to_setpoint(&self) -> Setpoint {
        Setpoint::PositionPoint {
            x: self.x,
            y: self.y,
            z: self.z,
            yaw_degrees: self.yaw_degrees,
        }
    }
}

#[derive(Debug)]
pub struct Model {
    pub vx: MetersPerSecond,
    pub vy: MetersPerSecond,
    pub yaw_rate: f32,
    pub z: Meters,
    pub motion_sender: mpsc::UnboundedSender<MotionCommand>,
    pub is_airborne: bool,
    pub is_recording: bool,
    pub recording: Vec<SetpointRecording>,
    pub speed_setting: MetersPerSecond,
    pub yaw_rate_setting: f32,
}
impl Model {
    pub fn new(motion_sender: mpsc::UnboundedSender<MotionCommand>) -> Self {
        Self {
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
        }
    }
}

// msg -----------------

#[derive(Clone, Debug)]
pub enum Msg {
    Move(Movement),
    Abort,
    SendNextMove,
    CommandSet,
    TakeOffDone,
    StartRecording,
    StopRecording,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Movement {
    Vx(MetersPerSecond),
    Vy(MetersPerSecond),
    YawRate(f32),
    Land,
    GoHome,
    Start,
    SpeedUp,
    SpeedDown,
}

// update -----------------

pub fn update(model: &mut Model, msg: Msg) -> Cmd<Msg> {
    let sender = model.motion_sender.clone();
    match msg {
        Msg::Move(Vx(new_x)) => {
            model.vx = new_x;
            Cmd::pure(Msg::SendNextMove)
        }
        Msg::Move(Vy(new_y)) => {
            model.vy = new_y;
            Cmd::pure(Msg::SendNextMove)
        }
        Msg::Move(YawRate(yaw_rate)) => {
            model.yaw_rate = yaw_rate;
            Cmd::pure(Msg::SendNextMove)
        }
        Msg::Move(Land) => {
            model.vx = MetersPerSecond(0.0);
            model.vy = MetersPerSecond(0.0);
            model.z = Meters(0.0);
            model.is_airborne = false;
            Cmd::new(async move { sender.send(MotionCommand::Land) }, |_| {
                CommandSet
            })
        }
        Msg::Move(GoHome) => {
            model.vx = MetersPerSecond(0.0);
            model.vy = MetersPerSecond(0.0);
            model.z = Meters(0.0);
            model.is_airborne = false;
            Cmd::new(async move { sender.send(MotionCommand::GoHome) }, |_| {
                CommandSet
            })
        }
        Msg::Move(Start) => {
            model.vx = MetersPerSecond(0.0);
            model.vy = MetersPerSecond(0.0);
            model.z = Meters(0.5);
            Cmd::new(
                async move { sender.send(MotionCommand::TakeOff(Meters(0.5))) },
                |_| Msg::TakeOffDone,
            )
        }
        Msg::SendNextMove if model.is_airborne => {
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
        Msg::SendNextMove => Cmd::none(),
        CommandSet => Cmd::none(),
        Msg::Abort => {
            model.vx = MetersPerSecond(0.0);
            model.vy = MetersPerSecond(0.0);
            model.z = Meters(0.0);
            Cmd::new(async move { sender.send(MotionCommand::Stop) }, |_| {
                CommandSet
            })
        }
        Msg::TakeOffDone => {
            model.is_airborne = true;
            Cmd::none()
        }
        Msg::Move(SpeedUp) => {
            model.yaw_rate_setting += 10.0;
            model.speed_setting += MetersPerSecond(0.1);
            Cmd::none()
        }
        Msg::Move(SpeedDown) => {
            model.yaw_rate_setting -= 10.0;
            model.speed_setting -= MetersPerSecond(0.1);
            Cmd::none()
        }
        Msg::StartRecording => {
            model.is_recording = true;
            Cmd::none()
        }
        Msg::StopRecording => {
            let recording = std::mem::take(&mut model.recording);
            model.is_recording = false;
            Cmd::new(store_recoding(recording), |_| CommandSet)
        }
    }
}

// util -----------------------------------------------
async fn store_recoding(recording: Vec<SetpointRecording>) {
    if let Some(first_p) = recording.first() {
        let z = recording.last().map(|p| p.z.0).unwrap_or(2.0);
        // z=1m => 2s, z=0.5m => 1s
        let land_duration = Duration::from_secs_f32((z.max(0.0) / 0.5).min(3.0));

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

pub fn movement_cmd_from_key(k: KeyEvent, s: &Model) -> Cmd<Msg> {
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
    .map(|m| Cmd::pure(Msg::Move(m)))
    .unwrap_or(Cmd::none())
}
