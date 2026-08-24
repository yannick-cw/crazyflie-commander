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
//! let (_, mut drone) = setup_link().await?;
//! drone.run_mission(orbit(), async { None }).await
//! # }
//! ```
mod air_data_link;
mod control;
mod occupancy;
mod utils;

pub use air_data_link::server;
pub use control::autopilot::{Autopilot, ManualControl, SetpointHover};
pub use control::crazyflie::CrazyPilot;
pub use control::crazyflie::setup_link;
pub use control::vehicle_control::init_vehicle_control;
pub use occupancy::grid::{Cell, OccupancyGrid};
pub use utils::dev_pilot;
pub use utils::errors;
pub use utils::flight_paths;
