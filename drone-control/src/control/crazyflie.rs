use crate::control::command_unit::{
    Abort, Command, CommandUnit, MissionStatus, MotionCommand, SetpointHover, Telemetry,
};
use crate::control::patterns::billiard_box::run_billiard_loop;
use crate::control::patterns::orbit::run_orbit;
use crate::control::patterns::setpoints::run_setpoints;
use crate::control::patterns::smooth_path::run_smooth_path;
use crate::control::trajectory::orbit_trajectory::orbit_to_trajectory;
use crate::control::trajectory::setpoint_trajectory::waypoints_to_trajectory;
use crate::control::vehicle::Vehicle;
use crate::utils::errors::MissionError::FailedToConnect;
use crate::utils::errors::Res;
use crate::{LinkMode, MetersPerSecond, Progress, Reason};
use crazyflie_lib::Crazyflie;
use crazyflie_lib::subsystems::log::LogPeriod;
use futures::{Stream, StreamExt};
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tokio::time::{MissedTickBehavior, sleep};
use tokio::{select, time};
use tracing::info;

/// Scan the radio for a Crazyflie, connect, reset its state estimate, and start telemetry logging.
///
/// Returns a [`CrazyflieCommandUnit`] ready to fly.
///
/// # Errors
/// Fails if no drone is found or the connection or logging setup fails.
pub async fn setup_link() -> Res<CrazyflieCommandUnit> {
    let link_context = crazyflie_link::LinkContext::new();
    let found = link_context.scan([0xE7; 5]).await?;

    let uri = found
        .first()
        .ok_or(FailedToConnect("Did not find crazyflie".to_string()))?;

    let cf = Crazyflie::connect_from_uri(&link_context, uri, crazyflie_lib::NoTocCache).await?;

    // Reset the x,y,z,yaw estimated values before a new flight
    cf.param.set_lossy("kalman.resetEstimation", 1.0).await?;
    sleep(Duration::from_millis(50)).await;
    cf.param.set_lossy("kalman.resetEstimation", 0.0).await?;

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
    let (status_sender, _) = watch::channel(MissionStatus::Idle);
    let mission_status = status_sender.clone();
    Ok(CrazyflieCommandUnit {
        autopilot: Vehicle::new(cf, watch_tx.subscribe()),
        telemetry_sender: tx,
        telemetry_latest: watch_tx,
        mission_status,
    })
}

/// A connected Crazyflie driving one drone over the radio link.
///
/// Created by [`setup_link`]; the [`CommandUnit`] implementation is how you fly it.
#[derive(Debug)]
pub struct CrazyflieCommandUnit {
    autopilot: Vehicle,
    telemetry_sender: broadcast::Sender<Telemetry>,
    telemetry_latest: watch::Sender<Telemetry>,
    mission_status: watch::Sender<MissionStatus>,
}

impl CrazyflieCommandUnit {
    // TODO upload should happen before takeoff!! Not in the air
    async fn start_mission(&self, mission: Vec<Command>, link_mode: LinkMode) -> Res<()> {
        let vehicle = &self.autopilot;

        let total_commands = mission.len();

        for (i, command) in mission.into_iter().enumerate() {
            self.mission_status
                .send(MissionStatus::Running(Some(Progress {
                    current_command: command.clone(),
                    command_num: i,
                    total_commands,
                })))
                .unwrap();

            match (command, link_mode) {
                (Command::Takeoff { height, duration }, _) => {
                    info!("Take Off...");
                    vehicle.take_off(height, duration).await?;
                }
                (Command::Move { x, y, z, duration }, _) => {
                    info!("Moving...");
                    vehicle.go_to(x, y, z, 0.0, duration, true, false).await?;
                }
                (Command::MoveToWaypoint { x, y, z, duration }, _) => {
                    info!("Moving to point...");
                    vehicle.go_to(x, y, z, 0.0, duration, false, false).await?;
                }
                (Command::Land { duration }, _) => {
                    info!("Landing...");
                    vehicle.land(duration).await?;
                }
                (Command::Hover { duration }, _) => sleep(duration).await,
                (Command::BilliardBox(params), _) => run_billiard_loop(params, vehicle).await?,
                (
                    Command::SmoothPath {
                        waypoints,
                        speed,
                        flight_mode,
                    },
                    LinkMode::StreamToVehicle,
                ) => run_smooth_path(waypoints, vehicle, speed, flight_mode).await?,
                (
                    Command::SmoothPath {
                        waypoints,
                        speed,
                        flight_mode,
                    },
                    LinkMode::OnVehicle,
                ) => {
                    let t = waypoints_to_trajectory(waypoints, speed, flight_mode)?;
                    let id = vehicle.upload_trajectory(&t).await?;
                    vehicle.run_trajectory(id, t.duration).await?
                }
                (Command::Setpoints { points }, _) => run_setpoints(points, vehicle).await?,
                (
                    Command::Orbit {
                        radius,
                        orbital_period,
                        orbits,
                        z,
                    },
                    LinkMode::OnVehicle,
                ) => {
                    let c = orbit_to_trajectory(radius, orbital_period, orbits, z)?;
                    let id = vehicle.upload_compressed_trajectory(&c).await?;
                    vehicle.run_trajectory(id, c.duration).await?
                }
                (
                    Command::Orbit {
                        radius,
                        orbital_period,
                        orbits,
                        z,
                    },
                    LinkMode::StreamToVehicle,
                ) => run_orbit(radius, orbital_period, orbits, z, vehicle).await?,
            }
        }
        Ok(())
    }

    async fn abort_mission(&self, abort: Abort) -> Res<()> {
        match abort {
            Abort::HardStop => {
                info!("HARD STOP..");
                self.autopilot.emergency_stop().await?;

                self.mission_status
                    .send(MissionStatus::Aborted(Reason::HardStop))
                    .unwrap();

                Ok(())
            }
            Abort::Land => {
                info!("Abort Land..");
                self.autopilot.return_home().await?;

                self.mission_status
                    .send(MissionStatus::Aborted(Reason::Landing))
                    .unwrap();

                Ok(())
            }
        }
    }
}

impl CommandUnit for CrazyflieCommandUnit {
    async fn run_mission(
        &self,
        mission: Vec<Command>,
        link_mode: LinkMode,
        abort_signal: impl Future<Output = Option<Abort>>,
    ) -> Res<()> {
        let mut telemetry_rx = self.autopilot.telemetry.clone();
        let is_low_bat = telemetry_rx.wait_for(Telemetry::is_low_bat);

        // runs mission or aborts on keypress or on low battery
        select! {
            mission = self.start_mission(mission, link_mode) => {
                info!("Mission complete");
                self.mission_status
                    .send(MissionStatus::Idle)
                    .unwrap();
                mission?
            }
            Some(abort) = abort_signal => {
                self.abort_mission(abort).await?
            }
            _ = is_low_bat=> {
                info!("Low battery - returning home");
                self.autopilot.return_home().await?;

                self.mission_status
                    .send(MissionStatus::Aborted(Reason::Landing))
                    .unwrap();
            }
        };
        Ok(())
    }

    async fn fly(&self, commands: impl Stream<Item = MotionCommand>) -> Res<()> {
        tokio::pin!(commands);

        let mut telemetry_rx = self.autopilot.telemetry.clone();
        let mut ticks = time::interval(Duration::from_millis(10));
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut last_setpoint: Option<SetpointHover> = None;

        loop {
            select! {
                // in case we do not have something new from the stream
                // we repeat the last setpoint motion
                _ = ticks.tick() => {
                    match last_setpoint {
                        None => {}
                        Some(s) => {
                            self.autopilot.send_relative_speed(s).await?;
                        }}
                },
                _ = telemetry_rx.wait_for(Telemetry::is_low_bat) => {
                    info!("Low battery - returning home");
                    self.autopilot.return_home().await?;
                    break;
                },
                maybe_motion = commands.next() => match maybe_motion {
                    //stream ended - land
                    None => {
                        if last_setpoint.is_some() {
                            self.autopilot.return_home().await?;
                        }
                        // free flight over - stopping
                        break;
                    }
                    Some(MotionCommand::Land) => {
                        last_setpoint = None;
                        self.autopilot.notify_setpoint_stop().await?;
                        self.autopilot.land(Duration::from_secs(2)).await?;
                    }
                    Some(MotionCommand::TakeOff(z) )=> {
                        self.autopilot.take_off(z, Duration::from_secs(2)).await?;
                        last_setpoint = Some(SetpointHover { vx: MetersPerSecond(0.0),vy: MetersPerSecond(0.0),z,yaw_rate: 0.0, });
                    }
                    Some(MotionCommand::Move(setpoint)) => {
                        last_setpoint = Some(setpoint);
                        self.autopilot.send_relative_speed(setpoint).await?;
                    }
                    Some(MotionCommand::Stop) => {
                        self.autopilot.emergency_stop().await?;
                        // free flight over - stopping
                        break;
                    }
                    Some(MotionCommand::GoHome) => {
                        last_setpoint = None;
                        self.autopilot.return_home().await?;
                    }},
            }
        }
        Ok(())
    }

    fn telemetry(&self) -> broadcast::Receiver<Telemetry> {
        self.telemetry_sender.subscribe()
    }

    fn latest_telemetry(&self) -> watch::Receiver<Telemetry> {
        self.telemetry_latest.subscribe()
    }

    fn mission_status(&self) -> watch::Receiver<MissionStatus> {
        self.mission_status.subscribe()
    }
}
