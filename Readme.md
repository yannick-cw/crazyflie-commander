# Crazyflie Commander

A terminal ground station for flying a [Crazyflie][] 2.1(+) nano-drone from your keyboard.
It shows live telemetry and a top-down map, runs saved missions, lets you construct missions, and lets you fly the
drone by hand and record what you fly back as a replayable mission.

https://github.com/yannick-cw/crazyflie-commander/raw/refs/heads/main/media/crazyflight_example.mp4

## Features

### Executing missions

A mission is a list of high-level commands (take off, go to waypoint, orbit, smooth path, land, …). Pick one on the
mission screen, press `t` to take off and run it, and watch the planned route, live position and heading, and progress
on the map. `l` lands, `h` returns to the takeoff point, `x` is an emergency stop.

Missions come from three places:

- **In code** build a `Vec<Command>` directly (see the ready-made patterns in `drone-control`'s `flight_paths`, and
  the runnable `cargo run --example fly`)
- **As JSON** put a `*.json` mission file into `drone-commander/missions/`; it appears on the selection screen
  automatically
- **Recorded** fly manually and record (below), the recording is saved as a JSON mission and replays exactly like any
  other

### Free flight & recording

Fly the drone by hand with the keyboard. Press `r` to start recording and `r` again to stop, the captured flight is
written to `drone-commander/missions/recordings/` and can be replayed from the mission screen.

| Key     | Action                    |
|---------|---------------------------|
| `w`/`s` | forward / back            |
| `a`/`d` | left / right              |
| `←`/`→` | yaw left / right          |
| `↑`/`↓` | increase / decrease speed |
| `t`     | take off                  |
| `l`     | land                      |
| `h`     | return to takeoff point   |
| `r`     | start / stop recording    |
| `x`     | emergency stop            |

Free flight needs to observe key press and key release key events, and that requires terminal keyboard enhancement (see
Requirements). On terminals without it, free flight is disabled, running missions still works.

### Mission planning

TBD: a future feature for building missions from waypoints inside the TUI.

## Requirements

**Hardware**

- A [Crazyflie][] 2.1 or 2.1+ with a **Flow deck v2**
- A **Crazyradio** USB dongle to talk to the drone over the radio link

**Software**

- Rust (the repo also ships a Nix flake: `nix develop`)
- For free flight: a terminal that supports the **Kitty keyboard protocol** (key press/release), e.g. Ghostty

**Good to know**

- Position and heading are measured from **takeoff**: the map centres on where the drone took off, and "forward" is the
  direction it faced then, on restart of the App, position is reset at 0,0 in the center again
- **Do not move the drone by hand between flights** while the app is running - this corrupts the telemetry data

## Running

```sh
cargo run
```

## Crates

- [`ratatea`](ratatea): an [Elm Architecture][tea] runtime for [ratatui][] TUIs.
- [`drone-control`](drone-control): library for flying missions on a Crazyflie over the radio link.
- [`drone-commander`](drone-commander): the terminal UI, built on the two crates above.

[Crazyflie]: https://www.bitcraze.io/products/crazyflie-2-1-plus/

[tea]: https://guide.elm-lang.org/architecture/

[ratatui]: https://ratatui.rs
