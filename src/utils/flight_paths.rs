use crate::Duration;
use crate::Meters;
use crate::control::command_unit::Command;
use crate::control::command_unit::Command::{Land, MoveToWaypoint, TakeOff};

pub fn haus_nikolaus() -> Vec<Command> {
    let go_to_corner = |x: f32, y: f32| MoveToWaypoint {
        x: Meters(x),
        y: Meters(y),
        z: Meters(0.5),
        duration: Duration::from_secs(2),
    };
    vec![
        TakeOff {
            height: Meters(0.5),
            duration: Duration::from_secs(2),
        },
        go_to_corner(-0.5, 0.0), // go to BL for start
        go_to_corner(0.5, 0.0),  // BL → BR
        go_to_corner(0.5, 0.7),  // BR → TR
        go_to_corner(-0.5, 0.7), // TR → TL
        go_to_corner(-0.5, 0.0), // TL → BL
        go_to_corner(0.5, 0.7),  // BL → TR (diagonal)
        go_to_corner(0.0, 1.0),  // TR → Peak
        go_to_corner(-0.5, 0.7), // Peak → TL
        go_to_corner(0.5, 0.0),  // TL → BR (diagonal)
        go_to_corner(0.0, 0.0),  // TL → BR (diagonal)
        Land {
            duration: Duration::from_secs(2),
        },
    ]
}
