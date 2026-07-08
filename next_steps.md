# Next steps

## 1. Extract a library with a clean public API

- convert the binary crate into a **library crate** (keep a thin binary that runs a demo mission)
- hide `crazyflie-lib` behind own domain types; public error type (`thiserror`)
- `Mission`: typed step sequence + `serde` derive — **JSON read/write lives here**, in the lib
- validate for correctness (parse-don't-validate: `Mission` -> `ValidatedMission`, `run` takes the
  validated type): starts takeoff, ends land, nothing after land, bounds + altitude sane — **currently missing**
- expose: `Autopilot` (command surface), `Mission`, `run(mission, abort)`, telemetry stream
  (`watch`/`broadcast`), progress stream (current step / total)
- connection: split `scan() -> [uri]` from `connect(uri) -> Autopilot`; radio address/channel/datarate as params
- connection stays live across missions: connect once -> run many (idle/land between), not one-shot
- add hardcoded config as lib config with defaults: radio params, geofence bounds, low-battery threshold
- geofence as a safety feature (generalizes billiard bounds + low-bat): enforce at
  (a) mission validation (reject out-of-bounds targets), (b) runtime telemetry monitor ->
  breach fires the abort seam (return-inside / land)
  config = bounds + breach policy. only as reliable as the position estimate (drift)
- newtypes for radians/degrees if needed
- commands to add for parity: body-frame moves (`forward/back/left/right/up/down`) + `turn`,
  `spiral` (HLC primitive), `go_home` (expose existing return-home)

## 2. TUI: load a mission, run it, observe

- **own crate** in a workspace, depends on the lib crate
- stack: `ratatui` (on top of the `crossterm` already used)
- connect screen: `scan()` -> list -> pick -> `connect(uri)`; show connection status
- load a JSON mission file -> deserialize into the lib's `Mission` -> submit via `run()`
- panels: top-down map (port the existing braille renderer) + telemetry + progress bar
- TUI holds only a **telemetry receiver + a command/abort sender** (keeps transport swappable for step 5)

## 3. Mission planning in the TUI

- waypoint editor: place / move points on the map -> build a `Mission` -> run
- save edited mission back to JSON (reuses lib serde)
- later: manual (arrow keys stream setpoints, no mission active)
- much later: interject into a running mission (needs control arbitration: operator vs mission)

## 4. Upload-and-execute missions (survives link loss)

- upload a full mission/trajectory to the drone, execute onboard -> runs even if the link drops
- check first: does `crazyflie-lib` expose trajectory-memory write + high-level define/start trajectory
  (if not, add it to the lib)
- host-side trajectory generation: waypoints -> polynomial segments
- lib API: `Trajectory`, `upload()`, `start()`; TUI: choose "upload & run" vs "stream & run"

## 5. Split control + TUI into two processes over MQTT

- drone-control service (owns radio + lib) and TUI client, talking over a broker
- shared **protocol crate**: wire messages (`Telemetry`, `Progress`, `Command`, `Abort`)
- MQTT adapter bridges the lib's in-process channels <-> topics; TUI swaps its receiver/sender to MQTT-backed
- broker-loss handling: reconnect, buffer, last-will (dead-service detection)
- crates: `rumqttc` (client), `rumqttd`/mosquitto (broker)

## 6. AI Deck extension

- stream jpeg via Wi-Fi via MQTT to base
- obstacle detection / avoidance via base
- follow mode
- on-board decision-making exploration