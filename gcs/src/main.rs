use crate::config::{MissionStoreSettings, get_config};
use crate::dev_unit::DevPilot;
use crate::external::mission_service::{FileMission, HttpMission, MissionService};
use crate::ground_data_link::vehicle_link::datalink_client;
use crate::program::Program;
use crossterm::terminal;
use mission_computer::setup_link;
use ratatea::run;
use std::path::Path;
use std::rc::Rc;
use tracing::info;

mod config;
mod dev_unit;
mod external;
mod ground_data_link;
mod pages;
mod program;
#[cfg(test)]
mod test_support;
mod view;

#[tokio::main]
// color_eyre:Result<()> is the alternative to the std lib `Box<dyn Error + Send + Sync + 'static>` case
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
    let config = get_config(&config_path)?;

    let file_appender = tracing_appender::rolling::never("./logs", "commander.log");
    tracing_subscriber::fmt()
        .with_writer(file_appender)
        .with_ansi(false)
        .init();
    let terminal_supports_enhancements = terminal::supports_keyboard_enhancement()?;

    let mission_loader: Rc<dyn MissionService> = match config.mission_store {
        MissionStoreSettings::Local(settings) => Rc::new(FileMission {
            file_path: settings.file_path,
        }),
        MissionStoreSettings::Remote(settings) => Rc::new(HttpMission {
            url: settings.url,
            secret: settings.key,
        }),
    };

    let vehicle_link = Rc::new(datalink_client(config.datalink.address).await?);

    info!("Starting up....");
    match setup_link().await {
        Ok(real_unit) => {
            // selection process
            // this needs to live for the whole program
            let command_unit: &'static _ = Box::leak(Box::new(real_unit));

            let p = Program::new(
                command_unit,
                vehicle_link,
                terminal_supports_enhancements,
                mission_loader,
            );
            run(p).await?;
        }
        _ => {
            // fallback for dev
            let command_unit = &DevPilot;
            let p = Program::new(
                command_unit,
                vehicle_link,
                terminal_supports_enhancements,
                mission_loader,
            );
            run(p).await?;
        }
    };
    Ok(())
}

// TODO:
// - [x] basic telemetry data live
// - [x] first screen: a select mission b plan mission c free flight
// - [x] messages spam into screen
// - [x] mission abort shortcuts + buttons (exit: x)
// - [x] after mission show button to return to home screen - WORKS IFF mission is not ongoing
// - [x] add mission state to telemetry and display + progress
// - [ ] give real time and steps estimates? - Wont do
// - [x] render position in x y z
// - [x] build free flight; wasd, QE for yaw, jk for up down
//       - first step auto take off + w for flying forwards
// - [x] show logs in ~~log window~~ or write to file
// - [x] speed up down modification in flight
// - [x] back from free flight < broken does not finish flying somehow - must make sure to end flying future
// - [x] landing in free flight in place + go home?
// - [ ] x in free flight must interrupt all! (won't do for now; only relevant for landing)
// - [x] TUI stores missions as JSON and reads those on start; control exposes serde decode encode (maybe validation)
// - [x] recording live flight and replay?
//      d- r for record, r again for stop - show recording toggle
//      d- recording window goes to the right size in the column (e.g. red marker during recording)
//      d- r collect all telemetry of actual position with 10ms resolution
//      d- store as json and just replay via low level setpoint commander
//      d- but what if not in same position -> would accelarate to start point very fast -> easy go to start position
//      d=> while ignoring z below 0.1m or min 0.1m, and only then, when hovering at start point -> go
//      - could even blink LEDs before starting beep beep beeeeep and when last setpoint is below 0.1m or so land after
//      d- tui records libs telemetry for recording time and stores as json
//      d- lib gets new command::replay that takes a list of setpoints with timestamps or duration offsets from start and
//      executes as normal thing as always <-> and lib gets the logic of slow fly to start and land or hover at finish
// - [x] paint selected mission before flying it! and then take off t button to start
// - [x] free flight not selectable in terminals that do not support
// - [x] refactor key_to_msg into each update / file
// - [x] proper pubic facing API docs #![warn(missing_docs)] in both libs
// - [x] trajectory generation + upload of offline flying
//   - [x] port orbit -> easyish? MISSING: yaw + UI U button to fly on board if possible (firmware bug fixed)
//   - [x] take off again - fixed - was closing memory in between!
//   - [x] port smooth flying -> easyish?
//   - [x] port smooth flying -> missing yaw in body frame mode
//   - [x] port move / move to -> easy = single point WONT DO
//   - [x] not port billiard, as reactive - relative
//   - [x] example usage to lib as example? https://github.com/bitcraze/crazyflie-lib-rs/blob/main/examples/trajectory.rs extend this
//   - [x] could also use an example in the trajectory.rs
//   - [x] improve compressed docs: mention bezier, its cubic, not quadratic for 3, bc. first point is always dropped, only h1,h2,e and e is start for next segment, trajectory type to be passed to high level commander
//   - [x] maybe refactor whole link mode state approach - could be cleaner - e.g. different checks for if upload is possible, could all be encoded in mission: ... in the model or so
// - [x] make trajectory upload actually happen BEFORE flight and only execute in flight + atomic* for sharing trajectory id?
// - [x] Anti-crash: read the 5 ranges each tick, slow/stop when the travel-direction one drops under ~0.5 m | in free flight for now
// - [x] Spin-scan: hover + slow yaw, accumulate ranges + pose into a 2D room outline at that height - render?? - use then as fence?
//   - no spin, just collecting when flying slowly
// - [x] post mission stops telemetry? - more like when battery abort telemetry stops changing? -- fixed by borrow bug
// - [x] use local backend + postgres (optionally) for missions
// - [x] use local backend + postgres (optionally) for recordings
// ---- NEXT
// --- NEXT
// - [ ] use local backend + postgres (optionally) for storing flights
// - [ ] webserver path: TUI stores no json missions, server does, can /post missions, /get missions and execute, /post mission results (grid + telemetry?), /post replays as missions
//       maybe serve a web page rendering a mission executed log + the grid created for that, /get grid for room id, auth for endpoints and page (bearer tkn - login form)!
// - [ ] new Command: Auto explore room and map it out?
// - [ ] improve grid building - right now very rough probability distribution
// - [ ] buffer some observations about range to not immediately back off?
// - [ ] Localize (maybe, very difficult stretch): pre-scan room with iPhone, match live ranges to the model → inject extPos to correct flow drift -> achieve awesome positioning???
// - [ ] vehicle selection screen first? - just use CLI flag or default
// - [ ] ratatea re-evaluate subscriptions
// - [ ] "connection lost" warning or whatever when unplugged
// - [ ] improve file read / write handling: location, if no dir...
// - [ ] nix for bulding executable
// - [ ] polish: only start with flowdeck, warn on non supporting terminal free flight,
// - [ ] potentially re-center map to match around drone and real room
