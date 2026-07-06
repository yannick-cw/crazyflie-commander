use crate::Duration;
use crate::Meters;
use crate::control::command_unit::Command::{BilliardBox, Land, MoveToWaypoint, TakeOff};
use crate::control::command_unit::{BilliardParams, Command, MetersPerSecond};

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

pub fn billiard_box() -> Vec<Command> {
    let base_box = BilliardParams {
        bl_x: Meters(0.0),
        bl_y: Meters(0.0),
        bl_z: Meters(0.5),
        tr_x: Meters(0.8),
        tr_y: Meters(0.8),
        tr_z: Meters(1.3),
        vx: Default::default(),
        vy: Default::default(),
        vz: Default::default(),
        hold_for: Duration::from_secs(10),
    };
    vec![
        TakeOff {
            height: Meters(0.5),
            duration: Duration::from_secs(2),
        },
        // BilliardBox(BilliardParams {
        //     vx: MetersPerSecond(0.7),
        //     vy: MetersPerSecond(0.6),
        //     vz: MetersPerSecond(0.2),
        //     ..base_box
        // }),
        BilliardBox(BilliardParams {
            vx: MetersPerSecond(0.0),
            vy: MetersPerSecond(0.0),
            vz: MetersPerSecond(1.0),
            ..base_box
        }),
        BilliardBox(BilliardParams {
            vx: MetersPerSecond(0.0),
            vy: MetersPerSecond(0.5),
            vz: MetersPerSecond(0.5),
            ..base_box
        }),
        BilliardBox(BilliardParams {
            vx: MetersPerSecond(0.6),
            vy: MetersPerSecond(0.45),
            vz: MetersPerSecond(0.0),
            ..base_box
        }),
        Land {
            duration: Duration::from_secs(2),
        },
    ]
}
