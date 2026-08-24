use crate::control::autopilot::{
    Autopilot, ManualControl, SetpointHover, VehicleDownlink, health_from_log, telemetry_from_log,
};
use crate::control::patterns::billiard_box::run_billiard_loop;
use crate::control::patterns::orbit::run_orbit;
use crate::control::patterns::setpoints::run_setpoints;
use crate::control::patterns::smooth_path::run_smooth_path;
use crate::control::trajectory::orbit_trajectory::orbit_to_trajectory;
use crate::control::trajectory::setpoint_trajectory::waypoints_to_trajectory;
use crate::control::vehicle::Vehicle;
use crate::control::vehicle_control::ProgressEvent;
use crate::occupancy::grid::{OccupancyGrid, update_grid};
use crate::utils::errors::MissionError::FailedToConnect;
use crate::utils::errors::Res;
use crazyflie_lib::Crazyflie;
use crazyflie_lib::subsystems::log::LogPeriod;
use datalink::domain_types::{
    Abort, FlightMode, Meters, MetersPerSecond, MissionItem, Progress, Telemetry, TrajectoryId,
    VehicleHealth, VehicleStatus, Waypoint,
};
use futures::{Stream, StreamExt, TryFutureExt};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::{MissedTickBehavior, sleep};
use tokio::{select, time};
use tracing::info;

pub const RANGE_FRONT: &str = "range.front";
pub const RANGE_BACK: &str = "range.back";
pub const RANGE_LEFT: &str = "range.left";
pub const RANGE_RIGHT: &str = "range.right";
pub const RANGE_UP: &str = "range.up";
pub const PM_STATE: &str = "pm.state";
pub const STATE_ESTIMATE_X: &str = "stateEstimate.x";
pub const STATE_ESTIMATE_Y: &str = "stateEstimate.y";
pub const STATE_ESTIMATE_Z: &str = "stateEstimate.z";
pub const STATE_ESTIMATE_VX: &str = "stateEstimate.vx";
pub const STATE_ESTIMATE_VY: &str = "stateEstimate.vy";
pub const STATE_ESTIMATE_YAW: &str = "stateEstimate.yaw";

/// Scan the radio for a Crazyflie, connect, reset its state estimate, and start telemetry logging.
///
/// Returns a [`CrazyPilot`] ready to fly.
///
/// # Errors
/// Fails if no drone is found or the connection or logging setup fails.
pub async fn setup_link() -> Res<(VehicleDownlink, CrazyPilot, mpsc::Receiver<ProgressEvent>)> {
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
    let mut log_block_range = cf.log.create_block().await?;
    let mut log_block_health = cf.log.create_block().await?;
    log_block_health.add_variable(PM_STATE).await?;

    let range_logs = [RANGE_FRONT, RANGE_BACK, RANGE_LEFT, RANGE_RIGHT, RANGE_UP];

    let state_estimate_logs = [
        STATE_ESTIMATE_X,
        STATE_ESTIMATE_Y,
        STATE_ESTIMATE_Z,
        STATE_ESTIMATE_VX,
        STATE_ESTIMATE_VY,
        STATE_ESTIMATE_YAW,
    ];

    for var_name in state_estimate_logs {
        log_block_telemetry.add_variable(var_name).await?;
    }

    for var_name in range_logs {
        log_block_range.add_variable(var_name).await?;
    }

    let log_stream_telemetry = log_block_telemetry
        .start(LogPeriod::from_millis(10).unwrap())
        .await?;

    let log_stream_range = log_block_range
        .start(LogPeriod::from_millis(10).unwrap())
        .await?;

    let log_stream_health = log_block_health
        .start(LogPeriod::from_millis(1000).unwrap())
        .await?;

    let (tx, _rx) = broadcast::channel(1024);
    let (health_sender, _h_r) = broadcast::channel(1024);
    let (health_watch, _) = watch::channel(VehicleHealth::default());
    let (sender_tx, r) = watch::channel(Telemetry::default());
    let (grid_sender, _) = broadcast::channel(64);
    let local_sender_tx = tx.clone();
    let local_watch_tx = sender_tx.clone();
    let local_grid_sender = grid_sender.clone();
    let local_health_sender = health_sender.clone();
    let local_watch_health = health_watch.clone();

    tokio::spawn(async move {
        let mut ticks = time::interval(Duration::from_millis(100));
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut grid = OccupancyGrid::new();
        loop {
            ticks.tick().await;
            let telemetry = *r.borrow();
            update_grid(&mut grid, &telemetry);
            let _ = local_grid_sender.send(grid.clone());
        }
    });
    tokio::spawn(async move {
        loop {
            let (tele_block, battery_block) =
                tokio::join!(log_stream_telemetry.next(), log_stream_range.next());
            match (tele_block, battery_block) {
                (Ok(tele_log), Ok(range_log)) => {
                    let telemetry = telemetry_from_log(&tele_log, &range_log);
                    let _ = local_sender_tx.send(telemetry);
                    let _ = local_watch_tx.send_replace(telemetry);
                }
                _ => break,
            }
        }
    });
    tokio::spawn(async move {
        while let Ok(health_update) = log_stream_health.next().await {
            let health = health_from_log(&health_update);
            let _ = local_health_sender.send(health);
            let _ = local_watch_health.send_replace(health);
        }
    });
    let (status_sender, _) = watch::channel(VehicleStatus::Idle);
    let (progress_sender, progress_receiver) = mpsc::channel(64);

    Ok((
        VehicleDownlink::new(tx.clone(), health_sender, status_sender, grid_sender),
        CrazyPilot {
            vehicle: Vehicle::new(cf, sender_tx.subscribe(), health_watch.subscribe()),
            mission_status: progress_sender,
        },
        progress_receiver,
    ))
}

/// A connected Crazyflie driving one drone over the radio link.
///
/// Created by [`setup_link`]; the [`Autopilot`] implementation is how you fly it.
#[derive(Debug)]
pub struct CrazyPilot {
    vehicle: Vehicle,
    pub mission_status: mpsc::Sender<ProgressEvent>,
}

impl CrazyPilot {
    async fn start_mission(&self, mission: Vec<MissionItem>) -> Res<()> {
        let vehicle = &self.vehicle;

        let total_commands = mission.len();

        for (i, command) in mission.into_iter().enumerate() {
            let _ = self
                .mission_status
                .send(ProgressEvent::Progress(Progress {
                    current_command: format!("{:?}", command),
                    command_num: i,
                    total_commands,
                }))
                .await;

            match command {
                MissionItem::Takeoff { height, duration } => {
                    info!("Take Off...");
                    vehicle.take_off(height, duration).await?;
                }
                MissionItem::Move { x, y, z, duration } => {
                    info!("Moving...");
                    vehicle.go_to(x, y, z, 0.0, duration, true, false).await?;
                }
                MissionItem::MoveToWaypoint { x, y, z, duration } => {
                    info!("Moving to point...");
                    vehicle.go_to(x, y, z, 0.0, duration, false, false).await?;
                }
                MissionItem::Land { duration } => {
                    info!("Landing...");
                    vehicle.land(duration).await?;
                }
                MissionItem::Hover { duration } => sleep(duration).await,
                MissionItem::BilliardBox(params) => run_billiard_loop(params, vehicle).await?,
                MissionItem::SmoothPath {
                    waypoints,
                    speed,
                    flight_mode,
                } => run_smooth_path(waypoints, vehicle, speed, flight_mode).await?,
                MissionItem::Setpoints { points } => run_setpoints(points, vehicle).await?,
                MissionItem::Orbit {
                    radius,
                    orbital_period,
                    orbits,
                    z,
                } => run_orbit(radius, orbital_period, orbits, z, vehicle).await?,
                MissionItem::OnVehicleTrajectory { duration, id, .. } => {
                    vehicle.run_trajectory(id, duration).await?
                }
            }
        }
        Ok(())
    }

    async fn abort_mission(&self, abort: Abort) -> Res<()> {
        match abort {
            Abort::FlightTermination => {
                info!("HARD STOP..");
                self.vehicle.emergency_stop().await?;
                Ok(())
            }
            Abort::Land => {
                info!("Abort Land..");
                self.vehicle.return_home().await?;
                Ok(())
            }
        }
    }
}

impl Autopilot for CrazyPilot {
    async fn run_mission(
        &mut self,
        mission: Vec<MissionItem>,
        abort_signal: impl Future<Output = Option<Abort>>,
    ) -> Res<()> {
        let mut health_rx = self.vehicle.health.clone();
        let is_low_bat = health_rx.wait_for(VehicleHealth::is_low_bat).map_ok(|_| ());

        // runs mission or aborts on keypress or on low battery
        select! {
            mission = self.start_mission(mission) => {
                info!("Mission complete");
                mission?
            }
            Some(abort) = abort_signal => {
                self.abort_mission(abort).await?
            }
            _ = is_low_bat=> {
                info!("Low battery - returning home");
                let _ = self.mission_status
                    .send(ProgressEvent::LowBatLanding).await;
                self.vehicle.return_home().await?;
            }
        }
        Ok(())
    }

    async fn upload_orbit(
        &mut self,
        radius: Meters,
        orbital_period: Duration,
        orbits: usize,
        z: Meters,
    ) -> Res<(TrajectoryId, Duration)> {
        let c = orbit_to_trajectory(radius, orbital_period, orbits, z)?;
        let duration = c.duration;
        let id = self.vehicle.upload_compressed_trajectory(c).await?;
        Ok((id, duration))
    }

    async fn upload_smooth_path(
        &mut self,
        waypoints: Vec<Waypoint>,
        speed: MetersPerSecond,
        flight_mode: FlightMode,
    ) -> Res<(TrajectoryId, Duration)> {
        let t = waypoints_to_trajectory(waypoints, speed, flight_mode)?;
        let duration = t.duration;
        let id = self.vehicle.upload_trajectory(t).await?;
        Ok((id, duration))
    }

    async fn fly(&mut self, commands: impl Stream<Item = ManualControl>) -> Res<()> {
        tokio::pin!(commands);

        let mut health_tx = self.vehicle.health.clone();
        let too_close_rx = self.vehicle.telemetry.clone();
        let mut ticks = time::interval(Duration::from_millis(20));
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut last_setpoint: Option<SetpointHover> = None;

        loop {
            select! {
                // in case we do not have something new from the stream
                // we repeat the last setpoint motion
                _ = ticks.tick() => {
                    let telemetry = *too_close_rx.borrow();
                    match (telemetry, last_setpoint) {
                        (t, Some(s)) if Vehicle::is_too_close(&t)=> {
                            let accelerate_away_setpoint = Vehicle::avoid_obstacle_move(s.z, &t);
                            self.vehicle.send_relative_speed(accelerate_away_setpoint).await?;
                        }
                        (_, None) => {}
                        (_, Some(s)) => {
                            self.vehicle.send_relative_speed(s).await?;
                        }}
                },
                // the `map_ok` is crucial - without it the sender towards telemetry_rx is blocked for the entire return_home
                // as the select arm is basically just the future that would still have the ref to telemetry open
                _ = health_tx.wait_for(VehicleHealth::is_low_bat).map_ok(|_|()) => {
                    info!("Low battery - returning home");
                    self.vehicle.return_home().await?;
                    break;
                },
                maybe_motion = commands.next() =>
                    match maybe_motion {
                        //stream ended - land
                        None => {
                            if last_setpoint.is_some() {
                                self.vehicle.return_home().await?;
                            }
                            // free flight over - stopping
                            break;
                        }
                        Some(ManualControl::Land) => {
                            last_setpoint = None;
                            self.vehicle.notify_setpoint_stop().await?;
                            self.vehicle.land(Duration::from_secs(2)).await?;
                        }
                        Some(ManualControl::TakeOff(z) )=> {
                            self.vehicle.take_off(z, Duration::from_secs(2)).await?;
                            last_setpoint = Some(SetpointHover { vx: MetersPerSecond(0.0),vy: MetersPerSecond(0.0),z,yaw_rate: 0.0, });
                        }
                        Some(ManualControl::Move(setpoint)) => {
                            last_setpoint = Some(setpoint);
                            self.vehicle.send_relative_speed(setpoint).await?;
                        }
                        Some(ManualControl::Stop) => {
                            self.vehicle.emergency_stop().await?;
                            // free flight over - stopping
                            break;
                        }
                        Some(ManualControl::GoHome) => {
                            last_setpoint = None;
                            self.vehicle.return_home().await?;
                        }},
            }
        }
        Ok(())
    }
}
