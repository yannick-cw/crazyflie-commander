use crate::downlink::KeyframeGrid as RawKeyframe;
use crate::downlink::MissionStatus as RawStatus;
use crate::downlink::changed_cells::ChangedCell;
use crate::downlink::keyframe_grid::ListOfCells;
use crate::downlink::mission_status::aborted::Reason as RawReason;
use crate::downlink::mission_status::running::Progress as RawProgress;
use crate::downlink::{VehicleHealth as RawHealth, vehicle_health};
use crate::downlink::{VehicleState, mission_status};
use crate::uplink;
use derive_more::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::time::Duration;

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    PartialOrd,
    Serialize,
    Deserialize,
    Copy,
    Add,
    Sub,
    Mul,
    Div,
    Neg,
)]
pub struct Meters(pub f32);

impl Display for Meters {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}m", self.0)
    }
}

#[derive(
    Serialize,
    Deserialize,
    Debug,
    Neg,
    Clone,
    Copy,
    PartialEq,
    PartialOrd,
    Default,
    Add,
    AddAssign,
    SubAssign,
    Sub,
    Mul,
)]
pub struct MetersPerSecond(pub f32);
impl Display for MetersPerSecond {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}m/s", self.0)
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct VehicleHealth {
    pub battery_level: BatteryLevel,
}
impl VehicleHealth {
    pub fn is_low_bat(&self) -> bool {
        self.battery_level == BatteryLevel::Low
    }
}

impl From<VehicleHealth> for RawHealth {
    fn from(value: VehicleHealth) -> Self {
        RawHealth {
            battery_level: match value.battery_level {
                BatteryLevel::Low => vehicle_health::BatteryLevel::Low.into(),
                BatteryLevel::High => vehicle_health::BatteryLevel::High.into(),
            },
        }
    }
}

impl From<RawHealth> for VehicleHealth {
    fn from(value: RawHealth) -> Self {
        VehicleHealth {
            battery_level: match value.battery_level() {
                vehicle_health::BatteryLevel::High => BatteryLevel::High,
                _ => BatteryLevel::Low,
            }, //todo fix
        }
    }
}
#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Telemetry {
    pub x: Meters,
    pub y: Meters,
    pub z: Meters,
    pub x_v: MetersPerSecond,
    pub y_v: MetersPerSecond,
    // pub z_v: MetersPerSecond,
    pub yaw_degrees: f32,
    pub range_front: Meters,
    pub range_back: Meters,
    pub range_right: Meters,
    pub range_left: Meters,
    pub range_up: Meters,
}
impl Telemetry {
    pub fn x(&self) -> f32 {
        self.x.0
    }
    pub fn y(&self) -> f32 {
        self.y.0
    }
    pub fn z(&self) -> f32 {
        self.z.0
    }
    pub fn vx(&self) -> f32 {
        self.x_v.0
    }
    pub fn vy(&self) -> f32 {
        self.y_v.0
    }
    // pub fn vz(&self) -> f32 {
    //     self.z_v.0
    // }
    pub fn yaw(&self) -> f32 {
        self.yaw_degrees
    }
    pub fn speed(&self) -> f32 {
        (self.x_v.0.powi(2) + self.y_v.0.powi(2)).sqrt()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Default, PartialOrd, Hash, Serialize, Deserialize)]
pub enum BatteryLevel {
    Low,
    #[default]
    High,
}

impl From<Telemetry> for VehicleState {
    fn from(value: Telemetry) -> Self {
        VehicleState {
            x: value.x.0,
            y: value.y.0,
            z: value.z.0,
            x_v: value.x_v.0,
            y_v: value.y_v.0,
            z_v: 0.0,
            yaw_degrees: value.yaw_degrees,
            range_front: value.range_front.0,
            range_back: value.range_back.0,
            range_right: value.range_right.0,
            range_left: value.range_left.0,
            range_up: value.range_up.0,
        }
    }
}

impl From<VehicleState> for Telemetry {
    fn from(value: VehicleState) -> Self {
        Telemetry {
            x: Meters(value.x),
            y: Meters(value.y),
            z: Meters(value.z),
            x_v: MetersPerSecond(value.x_v),
            y_v: MetersPerSecond(value.y_v),
            yaw_degrees: value.yaw_degrees,
            range_front: Meters(value.range_front),
            range_back: Meters(value.range_back),
            range_right: Meters(value.range_right),
            range_left: Meters(value.range_left),
            range_up: Meters(value.range_up),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, PartialOrd, Serialize, Deserialize)]
pub enum MissionStatus {
    #[default]
    Idle,
    Running(Option<Progress>),
    Aborted(Reason),
}

impl From<MissionStatus> for RawStatus {
    fn from(value: MissionStatus) -> Self {
        Self {
            status: Some(match value {
                MissionStatus::Idle => mission_status::Status::Idle(mission_status::Idle {}),
                MissionStatus::Running(progress) => {
                    mission_status::Status::Running(mission_status::Running {
                        p: progress.map(|p| p.into()),
                    })
                }
                MissionStatus::Aborted(reason) => {
                    mission_status::Status::Aborted(mission_status::Aborted {
                        reason: mission_status::aborted::Reason::from(reason).into(),
                    })
                }
            }),
        }
    }
}

impl From<RawStatus> for MissionStatus {
    fn from(value: RawStatus) -> Self {
        match value.status {
            None => MissionStatus::Idle,
            Some(mission_status::Status::Idle(_)) => MissionStatus::Idle,
            Some(mission_status::Status::Running(running)) => {
                MissionStatus::Running(running.p.map(|p| p.into()))
            }
            Some(mission_status::Status::Aborted(aborted)) => {
                MissionStatus::Aborted(aborted.reason().into())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Progress {
    pub current_command: String,
    pub command_num: usize,
    pub total_commands: usize,
}

impl From<Progress> for RawProgress {
    fn from(
        Progress {
            current_command,
            command_num,
            total_commands,
        }: Progress,
    ) -> Self {
        Self {
            current_command,
            command_num: command_num as u32,
            total_commands: total_commands as u32,
        }
    }
}

impl From<RawProgress> for Progress {
    fn from(value: RawProgress) -> Self {
        Self {
            current_command: value.current_command,
            command_num: value.command_num.try_into().unwrap(),
            total_commands: value.total_commands.try_into().unwrap(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Reason {
    Landing,
    HardStop,
    LowBattery,
}

impl From<RawReason> for Reason {
    fn from(value: RawReason) -> Self {
        match value {
            RawReason::Landing => Reason::Landing,
            RawReason::HardStop => Reason::HardStop,
            RawReason::LowBattery => Reason::LowBattery,
        }
    }
}

impl From<Reason> for RawReason {
    fn from(value: Reason) -> Self {
        match value {
            Reason::Landing => RawReason::Landing,
            Reason::HardStop => RawReason::HardStop,
            Reason::LowBattery => RawReason::LowBattery,
        }
    }
}

pub type OccupancyGrid = Vec<Vec<Cell>>;
impl From<OccupancyGrid> for RawKeyframe {
    fn from(value: OccupancyGrid) -> Self {
        Self {
            lists: value
                .into_iter()
                .map(|cells| ListOfCells {
                    quantized_odds: cells
                        .into_iter()
                        // this maps -5.0..5.0 to -50..50
                        .map(|c| (c.ln_ods * 10.0).round() as i32)
                        .collect(),
                })
                .collect(),
        }
    }
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Deserialize)]
pub struct Cell {
    pub ln_ods: f32,
}

impl From<(Cell, usize, usize)> for ChangedCell {
    fn from((cell, i, j): (Cell, usize, usize)) -> Self {
        Self {
            quantized_odds: (cell.ln_ods * 10.0).round() as i32,
            i: i as i32,
            j: j as i32,
        }
    }
}

impl From<i32> for Cell {
    fn from(quantized_ods: i32) -> Self {
        Cell {
            ln_ods: quantized_ods as f32 / 10.0,
        }
    }
}

impl From<RawKeyframe> for OccupancyGrid {
    fn from(keyframe: RawKeyframe) -> Self {
        keyframe
            .lists
            .into_iter()
            .map(|l| l.quantized_odds.into_iter().map(i32::into).collect())
            .collect()
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Waypoint {
    pub x: Meters,
    pub y: Meters,
    pub z: Meters,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
pub enum FlightMode {
    Strafe,
    BodyFrame,
}

#[derive(
    Debug, Default, Copy, Eq, Ord, Clone, PartialEq, PartialOrd, Hash, Serialize, Deserialize, Add,
)]
pub struct TrajectoryId(pub u8);

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BilliardParams {
    pub bl_x: Meters,
    pub bl_y: Meters,
    pub bl_z: Meters,
    pub tr_x: Meters,
    pub tr_y: Meters,
    pub tr_z: Meters,
    pub vx: MetersPerSecond,
    pub vy: MetersPerSecond,
    pub vz: MetersPerSecond,
    pub hold_for: Duration,
}

/// A single target for the low-level commander, streamed at high rate.
///
/// [`VelocityPoint`](Self::VelocityPoint) sets a body-frame velocity.
/// [`PositionPoint`](Self::PositionPoint) sets an absolute position relative to takeoff.
/// Used to replay a recorded flight via [`crate::MissionItem::Setpoints`].
#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Setpoint {
    VelocityPoint {
        vx: MetersPerSecond,
        vy: MetersPerSecond,
        vz: MetersPerSecond,
        yaw_rate: f32,
    },
    PositionPoint {
        x: Meters,
        y: Meters,
        z: Meters,
        yaw_degrees: f32,
    },
}
impl Default for Setpoint {
    fn default() -> Self {
        Setpoint::PositionPoint {
            x: Default::default(),
            y: Default::default(),
            z: Default::default(),
            yaw_degrees: 0.0,
        }
    }
}

/// A single high-level flight instruction.
///
/// A mission is a list of `MissionItem`s executed by [`Autopilot::run_mission`].
/// Positions are relative to the takeoff point unless a variant states otherwise.
#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MissionItem {
    Takeoff {
        height: Meters,
        duration: Duration,
    },
    // move relative to the current position
    Move {
        x: Meters,
        y: Meters,
        z: Meters,
        duration: Duration,
    },
    // move to a waypoint relative to the takeoff position
    MoveToWaypoint {
        x: Meters,
        y: Meters,
        z: Meters,
        duration: Duration,
    },
    // smooth waypoint - relative to takeoff position
    // important - first setpoint has to be the current position!
    SmoothPath {
        waypoints: Vec<Waypoint>,
        speed: MetersPerSecond,
        flight_mode: FlightMode,
    },
    Setpoints {
        points: Vec<Setpoint>,
    },
    // fly a bouncing pattern in the rectangle define by bl tr
    //   | ------- tr
    //   |         |
    //  bl ------- |
    BilliardBox(BilliardParams),
    Orbit {
        radius: Meters,
        orbital_period: Duration,
        orbits: usize,
        z: Meters,
    },
    Hover {
        duration: Duration,
    },
    Land {
        duration: Duration,
    },
    OnVehicleTrajectory {
        id: TrajectoryId,
        duration: Duration,
        original_command: Box<MissionItem>,
    },
}

impl MissionItem {
    // currently only `Orbit` supports uploading trajectory
    pub fn can_upload_trajectory(&self) -> bool {
        matches!(
            self,
            MissionItem::Orbit { .. } | MissionItem::SmoothPath { .. }
        )
    }
}

impl From<MissionItem> for uplink::MissionItem {
    fn from(value: MissionItem) -> Self {
        use uplink::mission_item::Item;

        let item = match value {
            MissionItem::Takeoff { height, duration } => Item::Takeoff(uplink::Takeoff {
                height: height.0,
                duration: Some(duration.try_into().unwrap()),
            }),
            MissionItem::Move { x, y, z, duration } => Item::Move(uplink::Move {
                x: x.0,
                y: y.0,
                z: z.0,
                duration: Some(duration.try_into().unwrap()),
            }),
            MissionItem::MoveToWaypoint { x, y, z, duration } => {
                Item::MoveToWaypoint(uplink::MoveToWaypoint {
                    x: x.0,
                    y: y.0,
                    z: z.0,
                    duration: Some(duration.try_into().unwrap()),
                })
            }
            MissionItem::SmoothPath {
                waypoints,
                speed,
                flight_mode,
            } => Item::SmoothPath(uplink::SmoothPath {
                waypoints: waypoints
                    .into_iter()
                    .map(|waypoint| uplink::Waypoint {
                        x: waypoint.x.0,
                        y: waypoint.y.0,
                        z: waypoint.z.0,
                    })
                    .collect(),
                speed: speed.0,
                flight_mode: match flight_mode {
                    FlightMode::Strafe => uplink::FlightMode::Strafe,
                    FlightMode::BodyFrame => uplink::FlightMode::BodyFrame,
                }
                .into(),
            }),
            MissionItem::Setpoints { points } => Item::Setpoints(uplink::Setpoints {
                points: points
                    .into_iter()
                    .map(|point| uplink::Setpoint {
                        point: Some(match point {
                            Setpoint::VelocityPoint {
                                vx,
                                vy,
                                vz,
                                yaw_rate,
                            } => uplink::setpoint::Point::Velocity(uplink::VelocityPoint {
                                vx: vx.0,
                                vy: vy.0,
                                vz: vz.0,
                                yaw_rate,
                            }),
                            Setpoint::PositionPoint {
                                x,
                                y,
                                z,
                                yaw_degrees,
                            } => uplink::setpoint::Point::Position(uplink::PositionPoint {
                                x: x.0,
                                y: y.0,
                                z: z.0,
                                yaw_degrees,
                            }),
                        }),
                    })
                    .collect(),
            }),
            MissionItem::BilliardBox(params) => Item::BilliardBox(uplink::BilliardParams {
                bl_x: params.bl_x.0,
                bl_y: params.bl_y.0,
                bl_z: params.bl_z.0,
                tr_x: params.tr_x.0,
                tr_y: params.tr_y.0,
                tr_z: params.tr_z.0,
                vx: params.vx.0,
                vy: params.vy.0,
                vz: params.vz.0,
                hold_for: Some(params.hold_for.try_into().unwrap()),
            }),
            MissionItem::Orbit {
                radius,
                orbital_period,
                orbits,
                z,
            } => Item::Orbit(uplink::Orbit {
                radius: radius.0,
                orbital_period: Some(orbital_period.try_into().unwrap()),
                orbits: orbits as u64,
                z: z.0,
            }),
            MissionItem::Hover { duration } => Item::Hover(uplink::Hover {
                duration: Some(duration.try_into().unwrap()),
            }),
            MissionItem::Land { duration } => Item::Land(uplink::Land {
                duration: Some(duration.try_into().unwrap()),
            }),
            MissionItem::OnVehicleTrajectory {
                id,
                duration,
                original_command,
            } => Item::OnVehicleTrajectory(Box::new(uplink::OnVehicleTrajectory {
                id: id.0.into(),
                duration: Some(duration.try_into().unwrap()),
                original_command: Some(Box::new((*original_command).into())),
            })),
        };

        uplink::MissionItem { item: Some(item) }
    }
}

impl From<uplink::MissionItem> for MissionItem {
    fn from(value: uplink::MissionItem) -> Self {
        use uplink::mission_item::Item;

        match value.item.unwrap() {
            Item::Takeoff(takeoff) => MissionItem::Takeoff {
                height: Meters(takeoff.height),
                duration: takeoff.duration.unwrap().try_into().unwrap(),
            },
            Item::Move(movement) => MissionItem::Move {
                x: Meters(movement.x),
                y: Meters(movement.y),
                z: Meters(movement.z),
                duration: movement.duration.unwrap().try_into().unwrap(),
            },
            Item::MoveToWaypoint(movement) => MissionItem::MoveToWaypoint {
                x: Meters(movement.x),
                y: Meters(movement.y),
                z: Meters(movement.z),
                duration: movement.duration.unwrap().try_into().unwrap(),
            },
            Item::SmoothPath(path) => {
                let mode = path.flight_mode();
                MissionItem::SmoothPath {
                    waypoints: path
                        .waypoints
                        .into_iter()
                        .map(|waypoint| Waypoint {
                            x: Meters(waypoint.x),
                            y: Meters(waypoint.y),
                            z: Meters(waypoint.z),
                        })
                        .collect(),
                    speed: MetersPerSecond(path.speed),
                    flight_mode: match mode {
                        uplink::FlightMode::BodyFrame => FlightMode::BodyFrame,
                        _ => FlightMode::Strafe,
                    },
                }
            }
            Item::Setpoints(setpoints) => MissionItem::Setpoints {
                points: setpoints
                    .points
                    .into_iter()
                    .map(|point| match point.point.unwrap() {
                        uplink::setpoint::Point::Velocity(point) => Setpoint::VelocityPoint {
                            vx: MetersPerSecond(point.vx),
                            vy: MetersPerSecond(point.vy),
                            vz: MetersPerSecond(point.vz),
                            yaw_rate: point.yaw_rate,
                        },
                        uplink::setpoint::Point::Position(point) => Setpoint::PositionPoint {
                            x: Meters(point.x),
                            y: Meters(point.y),
                            z: Meters(point.z),
                            yaw_degrees: point.yaw_degrees,
                        },
                    })
                    .collect(),
            },
            Item::BilliardBox(params) => MissionItem::BilliardBox(BilliardParams {
                bl_x: Meters(params.bl_x),
                bl_y: Meters(params.bl_y),
                bl_z: Meters(params.bl_z),
                tr_x: Meters(params.tr_x),
                tr_y: Meters(params.tr_y),
                tr_z: Meters(params.tr_z),
                vx: MetersPerSecond(params.vx),
                vy: MetersPerSecond(params.vy),
                vz: MetersPerSecond(params.vz),
                hold_for: params.hold_for.unwrap().try_into().unwrap(),
            }),
            Item::Orbit(orbit) => MissionItem::Orbit {
                radius: Meters(orbit.radius),
                orbital_period: orbit.orbital_period.unwrap().try_into().unwrap(),
                orbits: orbit.orbits as usize,
                z: Meters(orbit.z),
            },
            Item::Hover(hover) => MissionItem::Hover {
                duration: hover.duration.unwrap().try_into().unwrap(),
            },
            Item::Land(land) => MissionItem::Land {
                duration: land.duration.unwrap().try_into().unwrap(),
            },
            Item::OnVehicleTrajectory(trajectory) => MissionItem::OnVehicleTrajectory {
                id: TrajectoryId(trajectory.id as u8),
                duration: trajectory.duration.unwrap().try_into().unwrap(),
                original_command: Box::new((*trajectory.original_command.unwrap()).into()),
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Abort {
    FlightTermination,
    Land,
}

impl From<Abort> for uplink::AbortMission {
    fn from(value: Abort) -> Self {
        uplink::AbortMission {
            abort: match value {
                Abort::FlightTermination => uplink::abort_mission::Abort::HardStop.into(),
                Abort::Land => uplink::abort_mission::Abort::Land.into(),
            },
        }
    }
}

impl From<uplink::AbortMission> for Abort {
    fn from(value: uplink::AbortMission) -> Self {
        match value.abort() {
            uplink::abort_mission::Abort::Land => Abort::Land,
            uplink::abort_mission::Abort::HardStop => Abort::FlightTermination,
        }
    }
}
