# drone-control

> Work in progress, API not final.

A Rust library for autonomous missions on a [Crazyflie](https://www.bitcraze.io/products/crazyflie-2-1-plus/)
nano-drone. It's based on [`crazyflie-lib`](https://crates.io/crates/crazyflie-lib) (async, radio link).
A flight is a list of high-level `Command`s (take off, go to waypoint, orbit, smooth path, billiard-box, land). The
library runs the mission, streams live telemetry, and takes an abort signal (e.g. keypress) that lands or
emergency-stops.

## Example

A runnable example lives in [`examples/fly.rs`](examples/fly.rs):

```sh
cargo run --example fly
```

It connects to a Crazyflie over the radio and flies an orbit, with `x` = emergency stop and `l` = land.

## Usage

A mission is a `Vec<Command>`, built by hand or from the ready-made paths in `flight_paths`, then run against a
connected drone:

```rust
use drone_control::{setup_link, CommandUnit, flight_paths::orbit};

#[tokio::main]
async fn main() -> drone_control::errors::Res<()> {
    let drone = setup_link().await?;          // scan + connect over the radio
    let mission = orbit();                     // a Vec<Command>
    let abort = std::future::pending();        // no abort in this minimal example
    drone.run_mission(mission, abort).await
}
```

Assembled directly:

```rust
use drone_control::{Command, Meters};
use std::time::Duration;

fn mission() -> Vec<Command> {
    vec![
        Command::Takeoff { height: Meters(0.5), duration: Duration::from_secs(2) },
        Command::MoveToWaypoint { x: Meters(0.5), y: Meters(0.5), z: Meters(0.5), duration: Duration::from_secs(3) },
        Command::Land { duration: Duration::from_secs(2) },
    ]
}
```
