//! Fly autonomous missions on a
//! [Crazyflie](https://www.bitcraze.io/products/crazyflie-2-1-plus/) nano-drone over the radio link.
//!
//! A mission is list of high-level [`Command`]s run by a [`CommandUnit`].
//! [`setup_link`] connects to a drone and returns one, live [`Telemetry`] and [`MissionStatus`]
//! stream while it flies. Example patterns live in [`flight_paths`].
//!
//! # Examples
//! ```no_run
//! use drone_control::{setup_link, CommandUnit, flight_paths::orbit};
//!
//! # async fn run() -> drone_control::errors::Res<()> {
//! let drone = setup_link().await?;
//! drone.run_mission(orbit(), async { None }).await
//! # }
//! ```
mod control;
mod utils;

pub use control::command_unit::{
    Abort, Command, CommandUnit, FlightMode, Meters, MetersPerSecond, MissionStatus, MotionCommand,
    Progress, Reason, SetpointHover, Telemetry, TrajectoryId, Waypoint,
};
pub use control::crazyflie::CrazyflieCommandUnit;
pub use control::crazyflie::setup_link;
pub use control::low_level_engine::Setpoint;
pub use utils::errors;
pub use utils::flight_paths;
