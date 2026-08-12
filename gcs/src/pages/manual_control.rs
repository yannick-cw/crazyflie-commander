use crate::external::mission_service::MissionService;
use crate::pages::manual_control::Movement::*;
use crate::pages::manual_control::Msg::{
    Abort, CommandSet, ExitPage, Move, StartRecording, StopRecording,
};
use Msg::{SendNextMove, TakeOffDone};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use mission_computer::{ManualControl, Meters, MetersPerSecond, Setpoint, SetpointHover};
use ratatea::Cmd;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use tokio::sync::mpsc;

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
    pub motion_sender: mpsc::UnboundedSender<ManualControl>,
    pub is_airborne: bool,
    pub is_recording: bool,
    pub recording: Vec<SetpointRecording>,
    pub speed_setting: MetersPerSecond,
    pub yaw_rate_setting: f32,
}
impl Model {
    pub fn new(motion_sender: mpsc::UnboundedSender<ManualControl>) -> Self {
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
    ExitPage,
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

pub fn update(model: &mut Model, msg: Msg, mission_svc: Rc<dyn MissionService>) -> Cmd<Msg> {
    let sender = model.motion_sender.clone();
    match msg {
        Move(Vx(new_x)) => {
            model.vx = new_x;
            Cmd::pure(SendNextMove)
        }
        Move(Vy(new_y)) => {
            model.vy = new_y;
            Cmd::pure(SendNextMove)
        }
        Move(YawRate(yaw_rate)) => {
            model.yaw_rate = yaw_rate;
            Cmd::pure(SendNextMove)
        }
        Move(Land) => {
            model.vx = MetersPerSecond(0.0);
            model.vy = MetersPerSecond(0.0);
            model.z = Meters(0.0);
            model.is_airborne = false;
            Cmd::new(async move { sender.send(ManualControl::Land) }, |_| {
                CommandSet
            })
        }
        Move(GoHome) => {
            model.vx = MetersPerSecond(0.0);
            model.vy = MetersPerSecond(0.0);
            model.z = Meters(0.0);
            model.is_airborne = false;
            Cmd::new(async move { sender.send(ManualControl::GoHome) }, |_| {
                CommandSet
            })
        }
        Move(Start) => {
            model.vx = MetersPerSecond(0.0);
            model.vy = MetersPerSecond(0.0);
            model.z = Meters(0.5);
            Cmd::new(
                async move { sender.send(ManualControl::TakeOff(Meters(0.5))) },
                |_| TakeOffDone,
            )
        }
        SendNextMove if model.is_airborne => {
            let vx = model.vx;
            let vy = model.vy;
            let z = model.z;
            let yaw_rate = model.yaw_rate;
            Cmd::new(
                async move {
                    sender.send(ManualControl::Move(SetpointHover {
                        vx,
                        vy,
                        z,
                        yaw_rate,
                    }))
                },
                |_| CommandSet,
            )
        }
        SendNextMove => Cmd::none(),
        CommandSet => Cmd::none(),
        Abort => {
            model.vx = MetersPerSecond(0.0);
            model.vy = MetersPerSecond(0.0);
            model.z = Meters(0.0);
            Cmd::new(async move { sender.send(ManualControl::Stop) }, |_| {
                CommandSet
            })
        }
        TakeOffDone => {
            model.is_airborne = true;
            Cmd::none()
        }
        Move(SpeedUp) => {
            model.yaw_rate_setting += 10.0;
            model.speed_setting += MetersPerSecond(0.1);
            Cmd::none()
        }
        Move(SpeedDown) => {
            model.yaw_rate_setting -= 10.0;
            model.speed_setting -= MetersPerSecond(0.1);
            Cmd::none()
        }
        StartRecording => {
            model.is_recording = true;
            Cmd::none()
        }
        StopRecording => {
            let recording = std::mem::take(&mut model.recording);
            model.is_recording = false;
            Cmd::new(
                async move { mission_svc.store_recoding(recording).await },
                |_| CommandSet,
            )
        }
        // handled by parent
        ExitPage => Cmd::none(),
    }
}

pub fn map_key_evt(k: KeyEvent, s: &Model) -> Cmd<Msg> {
    match k.code {
        // movement keys in flight mode
        code if ['w', 'a', 's', 'd', 'h']
            .into_iter()
            .any(|c| code.is_char(c))
            | code.is_left()
            | code.is_right()
            | code.is_down()
            | code.is_up() =>
        {
            movement_cmd_from_key(k, s)
        }
        KeyCode::Char('l') if k.is_press() => Cmd::pure(Move(Land)),
        KeyCode::Char('x') if k.is_press() => Cmd::pure(Abort),
        KeyCode::Char('r') if k.is_press() && !s.is_recording => Cmd::pure(StartRecording),
        KeyCode::Char('r') if k.is_press() && s.is_recording => Cmd::pure(StopRecording),
        KeyCode::Char('t') if k.is_press() => Cmd::pure(Move(Start)),
        KeyCode::Char('b') if k.is_press() && !s.is_airborne => Cmd::pure(ExitPage),
        _ => Cmd::none(),
    }
}

fn movement_cmd_from_key(k: KeyEvent, s: &Model) -> Cmd<Msg> {
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
