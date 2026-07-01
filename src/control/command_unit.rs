use crate::utils::errors::MissionError;
use derive_more::{Add, Mul, Sub};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Add, Sub, Mul)]
pub struct Meters(pub f64);

pub enum Command {
    TakeOff {
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
    // move to a waypoint relative to the take off position
    MoveToWaypoint {
        x: Meters,
        y: Meters,
        z: Meters,
        duration: Duration,
    },
    Land {
        duration: Duration,
    },
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Waypoint {
    x: Meters,
    y: Meters,
    z: Meters,
    visited_at: Instant,
}
impl Waypoint {
    fn create(visited_at: Instant) -> Self {
        Self {
            x: Default::default(),
            y: Default::default(),
            z: Default::default(),
            visited_at,
        }
    }
}

#[allow(async_fn_in_trait)]
pub trait CommandUnit {
    async fn run_mission(&self, mission: Vec<Command>) -> Result<Vec<Waypoint>, MissionError>;
}

pub struct TestCommandUnit {
    pub current_time: Instant,
}

impl CommandUnit for TestCommandUnit {
    async fn run_mission(&self, mission: Vec<Command>) -> Result<Vec<Waypoint>, MissionError> {
        let waypoints: Vec<Waypoint> = mission
            .into_iter()
            .scan(
                Waypoint::create(self.current_time),
                |last_waypoint, command| {
                    let next_waypoint = match command {
                        Command::TakeOff { height, duration } => Waypoint {
                            x: Meters(0.0),
                            y: Meters(0.0),
                            z: height,
                            visited_at: last_waypoint.visited_at + duration,
                        },
                        Command::Move { x, y, z, duration } => Waypoint {
                            x: last_waypoint.x + x,
                            y: last_waypoint.y + y,
                            z: last_waypoint.z + z,
                            visited_at: last_waypoint.visited_at + duration,
                        },
                        Command::MoveToWaypoint { x, y, z, duration } => Waypoint {
                            x,
                            y,
                            z,
                            visited_at: last_waypoint.visited_at + duration,
                        },
                        Command::Land { duration } => Waypoint {
                            x: last_waypoint.x,
                            y: last_waypoint.y,
                            z: Meters(0.0),
                            visited_at: last_waypoint.visited_at + duration,
                        },
                    };

                    *last_waypoint = next_waypoint;
                    Some(next_waypoint)
                },
            )
            .collect();
        Ok(waypoints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::drone_command::Command::{Land, MoveToWaypoint, TakeOff};

    #[tokio::test]
    async fn simple_mission() {
        let commands = vec![
            TakeOff {
                height: Meters(2.0),
                duration: Duration::from_secs(3),
            },
            MoveToWaypoint {
                x: Meters(1.0),
                y: Meters(1.0),
                z: Meters(1.0),
                duration: Duration::from_secs(5),
            },
            Land {
                duration: Duration::from_secs(2),
            },
        ];

        let current_time = Instant::now();

        let mission_result = TestCommandUnit { current_time }.run_mission(commands).await;

        let expected_waypoints = vec![
            Waypoint {
                x: Meters(0.0),
                y: Meters(0.0),
                z: Meters(2.0),
                visited_at: current_time + Duration::from_secs(3),
            },
            Waypoint {
                x: Meters(1.0),
                y: Meters(1.0),
                z: Meters(1.0),
                visited_at: current_time + Duration::from_secs(3 + 5),
            },
            Waypoint {
                x: Meters(1.0),
                y: Meters(1.0),
                z: Meters(0.0),
                visited_at: current_time + Duration::from_secs(3 + 5 + 2),
            },
        ];

        assert_eq!(mission_result, Ok(expected_waypoints))
    }
}
