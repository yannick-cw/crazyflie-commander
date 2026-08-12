//! Fly autonomous missions on a
//! [Crazyflie](https://www.bitcraze.io/products/crazyflie-2-1-plus/) nano-drone over the radio link.
//!
//! A mission is list of high-level [`MissionItem`]s run by a [`Autopilot`].
//! [`setup_link`] connects to a drone and returns one, live [`Telemetry`] and [`MissionStatus`]
//! stream while it flies. Example patterns live in [`flight_paths`].
//!
//! # Examples
//! ```no_run
//! use mission_computer::{setup_link, Autopilot, flight_paths::orbit};
//!
//! # async fn run() -> mission_computer::errors::Res<()> {
//! let drone = setup_link().await?;
//! drone.run_mission(orbit(), async { None }).await
//! # }
//! ```
mod control;
mod occupancy;
mod utils;

pub use control::command_unit::{
    Abort, Autopilot, FlightMode, ManualControl, Meters, MetersPerSecond, MissionItem,
    MissionStatus, Progress, Reason, SetpointHover, Telemetry, TrajectoryId, Waypoint,
};
pub use control::crazyflie::CrazyPilot;
pub use control::crazyflie::setup_link;
pub use control::low_level_engine::Setpoint;
pub use occupancy::grid::{Cell, OccupancyGrid};
pub use utils::errors;
pub use utils::flight_paths;
