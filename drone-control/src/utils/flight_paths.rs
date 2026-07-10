use std::time::Duration;
use crate::control::command_unit::Command::{
    BilliardBox, Land, MoveToWaypoint, Orbit, SmoothPath, Takeoff,
};
use crate::control::command_unit::FlightMode::{BodyFrame, Strafe};
use crate::control::command_unit::{BilliardParams, Command, Meters, MetersPerSecond, Waypoint};

pub fn haus_nikolaus() -> Vec<Command> {
    let go_to_corner = |x: f32, y: f32| MoveToWaypoint {
        x: Meters(x),
        y: Meters(y),
        z: Meters(0.5),
        duration: Duration::from_secs(2),
    };
    vec![
        Takeoff {
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

pub fn orbit() -> Vec<Command> {
    vec![
        Takeoff {
            height: Meters(0.5),
            duration: Duration::from_secs(2),
        },
        Orbit {
            radius: Meters(0.7),
            orbital_period: Duration::from_secs(4),
            orbits: 30,
            z: Meters(0.5),
        },
        MoveToWaypoint {
            x: Meters(0.0),
            y: Meters(0.0),
            z: Meters(0.5),
            duration: Duration::from_secs(2),
        },
        Land {
            duration: Duration::from_secs(2),
        },
    ]
}

pub fn smooth_curves() -> Vec<Command> {
    let one_loop = vec![
        Waypoint {
            x: Meters(0.5),
            y: Meters(0.0),
            z: Meters(1.0),
        },
        Waypoint {
            x: Meters(2.0),
            y: Meters(0.0),
            z: Meters(0.5),
        },
        Waypoint {
            x: Meters(1.5),
            y: Meters(1.0),
            z: Meters(0.5),
        },
        Waypoint {
            x: Meters(0.0),
            y: Meters(1.5),
            z: Meters(0.5),
        },
        Waypoint {
            x: Meters(0.0),
            y: Meters(0.0),
            z: Meters(0.5),
        },
    ];
    vec![
        Takeoff {
            height: Meters(0.5),
            duration: Duration::from_secs(2),
        },
        SmoothPath {
            waypoints: one_loop.repeat(2),
            speed: MetersPerSecond(1.5),
            flight_mode: Strafe,
        },
        MoveToWaypoint {
            x: Meters(0.0),
            y: Meters(0.0),
            z: Meters(0.5),
            duration: Duration::from_secs(3),
        },
        Land {
            duration: Duration::from_secs(2),
        },
    ]
}

pub fn body_frame_smooth() -> Vec<Command> {
    let one_loop = vec![
        Waypoint {
            x: Meters(0.75),
            y: Meters(0.0),
            z: Meters(1.0),
        },
        Waypoint {
            x: Meters(1.5),
            y: Meters(0.0),
            z: Meters(0.5),
        },
        Waypoint {
            x: Meters(1.5),
            y: Meters(1.0),
            z: Meters(0.5),
        },
        Waypoint {
            x: Meters(0.3),
            y: Meters(1.5),
            z: Meters(0.5),
        },
        Waypoint {
            x: Meters(0.0),
            y: Meters(0.3),
            z: Meters(0.5),
        },
    ];
    vec![
        Takeoff {
            height: Meters(0.5),
            duration: Duration::from_secs(2),
        },
        SmoothPath {
            waypoints: one_loop.repeat(2),
            speed: MetersPerSecond(1.7),
            flight_mode: BodyFrame,
        },
        MoveToWaypoint {
            x: Meters(0.0),
            y: Meters(0.0),
            z: Meters(0.5),
            duration: Duration::from_secs(3),
        },
        Land {
            duration: Duration::from_secs(2),
        },
    ]
}

pub fn lawn_mower() -> Vec<Command> {
    fn lane(y: f32) -> Vec<Waypoint> {
        vec![
            Waypoint {
                x: Meters(0.0),
                y: Meters(y),
                z: Meters(0.3),
            },
            Waypoint {
                x: Meters(1.3),
                y: Meters(y),
                z: Meters(0.3),
            },
            Waypoint {
                x: Meters(1.3),
                y: Meters(y + 0.2),
                z: Meters(0.3),
            },
            Waypoint {
                x: Meters(0.0),
                y: Meters(y + 0.2),
                z: Meters(0.3),
            },
        ]
    }
    let points: Vec<_> = (0..4).flat_map(|i| lane(i as f32 * 0.4)).collect();
    vec![
        Takeoff {
            height: Meters(0.3),
            duration: Duration::from_secs(2),
        },
        SmoothPath {
            waypoints: points,
            speed: MetersPerSecond(0.8),
            flight_mode: BodyFrame,
        },
        MoveToWaypoint {
            x: Meters(0.0),
            y: Meters(0.0),
            z: Meters(0.3),
            duration: Duration::from_secs(3),
        },
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
        Takeoff {
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
