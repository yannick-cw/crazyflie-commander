use crate::control::command_unit::{Abort, Command, CommandUnit, Telemetry};
use crate::control::patterns::billiard_box::run_billiard_loop;
use crate::control::patterns::orbit::run_orbit;
use crate::control::patterns::smooth_path::run_smooth_path;
use crate::control::vehicle::Vehicle;
use crate::utils::errors::MissionError::FailedToConnect;
use crate::utils::errors::Res;
use crazyflie_lib::Crazyflie;
use crazyflie_lib::subsystems::log::LogPeriod;
use tokio::select;
use tokio::sync::{broadcast, watch};
use tokio::time::sleep;
use tracing::info;

pub async fn setup_link() -> Res<CrazyflieCommandUnit> {
    let link_context = crazyflie_link::LinkContext::new();
    let found = link_context.scan([0xE7; 5]).await?;

    let uri = found
        .first()
        .ok_or(FailedToConnect("Did not find crazyflie".to_string()))?;

    let cf = Crazyflie::connect_from_uri(&link_context, uri, crazyflie_lib::NoTocCache).await?;

    let mut log_block_telemetry = cf.log.create_block().await?;
    let mut log_stream_battery = cf.log.create_block().await?;
    log_stream_battery.add_variable("pm.state").await?;

    let log_var_names = [
        "stateEstimate.x",
        "stateEstimate.y",
        "stateEstimate.z", // seems to be broken?
        "stateEstimate.vx",
        "stateEstimate.vy",
        "stateEstimate.yaw",
        // "stateEstimate.vz",
    ];

    for var_name in log_var_names {
        log_block_telemetry.add_variable(var_name).await?;
    }

    let log_stream_telemetry = log_block_telemetry
        .start(LogPeriod::from_millis(10).unwrap())
        .await?;

    let log_stream_battery = log_stream_battery
        .start(LogPeriod::from_millis(10).unwrap())
        .await?;

    let (tx, _rx) = broadcast::channel(64);
    let (watch_tx, _watch_rx) = watch::channel(Telemetry::default());
    let local_sender_tx = tx.clone();
    let local_watch_tx = watch_tx.clone();
    tokio::spawn(async move {
        loop {
            let (tele_block, battery_block) =
                tokio::join!(log_stream_telemetry.next(), log_stream_battery.next());
            match (tele_block, battery_block) {
                (Ok(tele_log), Ok(bat_log)) => {
                    let telemetry = Telemetry::from_log_data(&tele_log, &bat_log);
                    let _ = local_sender_tx.send(telemetry);
                    let _ = local_watch_tx.send(telemetry);
                }
                _ => break,
            }
        }
    });
    Ok(CrazyflieCommandUnit {
        autopilot: Vehicle::new(cf, watch_tx.subscribe()),
        telemetry_sender: tx,
        telemetry_latest: watch_tx,
    })
}

pub struct CrazyflieCommandUnit {
    autopilot: Vehicle,
    telemetry_sender: broadcast::Sender<Telemetry>,
    telemetry_latest: watch::Sender<Telemetry>,
}

impl CrazyflieCommandUnit {
    async fn start_mission(&self, mission: Vec<Command>) -> Res<()> {
        let vehicle = &self.autopilot;

        vehicle.reset_estimator().await?;

        for command in mission {
            match command {
                Command::Takeoff { height, duration } => {
                    info!("Take Off...");
                    vehicle.take_off(height, duration).await?;
                }
                Command::Move { x, y, z, duration } => {
                    info!("Moving...");
                    vehicle.go_to(x, y, z, 0.0, duration, true, false).await?;
                }
                Command::MoveToWaypoint { x, y, z, duration } => {
                    info!("Moving to point...");
                    vehicle.go_to(x, y, z, 0.0, duration, false, false).await?;
                }
                Command::Land { duration } => {
                    info!("Landing...");
                    vehicle.land(duration).await?;
                }
                Command::Hover { duration } => sleep(duration).await,
                Command::BilliardBox(params) => run_billiard_loop(params, vehicle).await?,
                Command::SmoothPath {
                    waypoints,
                    speed,
                    flight_mode,
                } => run_smooth_path(waypoints, vehicle, speed, flight_mode).await?,
                Command::Orbit {
                    radius,
                    orbital_period,
                    orbits,
                    z,
                } => run_orbit(radius, orbital_period, orbits, z, vehicle).await?,
            }
        }
        Ok(())
    }

    async fn abort_mission(&self, abort: Abort) -> Res<()> {
        match abort {
            Abort::HardStop => {
                info!("HARD STOP..");
                self.autopilot.emergency_stop().await
            }
            Abort::Land => {
                info!("Abort Land..");
                self.autopilot.return_home().await
            }
        }
    }
}

impl CommandUnit for CrazyflieCommandUnit {
    async fn run_mission(
        &self,
        mission: Vec<Command>,
        abort_signal: impl Future<Output = Option<Abort>>,
    ) -> Res<()> {
        let mut telemetry_rx = self.autopilot.telemetry.clone();
        let is_low_bat = telemetry_rx.wait_for(Telemetry::is_low_bat);

        // runs mission or aborts on keypress or on low battery
        Ok(select! {
            mission = self.start_mission(mission) => {
                info!("Mission complete");
                mission?
            }
            Some(abort) = abort_signal => {
                self.abort_mission(abort).await?
            }
            _ = is_low_bat=> {
                info!("Low battery - returning home");
                self.autopilot.return_home().await?
            }
        })
    }

    fn telemetry(&self) -> broadcast::Receiver<Telemetry> {
        self.telemetry_sender.subscribe()
    }

    fn latest_telemetry(&self) -> watch::Receiver<Telemetry> {
        self.telemetry_latest.subscribe()
    }
}
