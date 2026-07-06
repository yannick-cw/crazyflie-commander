use crate::control::billiard_box::run_billiard_loop;
use crate::control::command_unit::{Command, CommandUnit, Telemetry};
use crate::control::smooth_path::run_smooth_path;
use crate::utils::errors::MissionError::FailedToConnect;
use crate::utils::errors::Res;
use crazyflie_lib::Crazyflie;
use crazyflie_lib::subsystems::log::LogPeriod;
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tokio::time::sleep;

pub async fn setup_link() -> Res<CrazyflieCommandUnit> {
    let link_context = crazyflie_link::LinkContext::new();
    let found = link_context.scan([0xE7; 5]).await?;

    let uri = found
        .first()
        .ok_or(FailedToConnect("Did not find crazyflie".to_string()))?;

    let cf = Crazyflie::connect_from_uri(&link_context, uri, crazyflie_lib::NoTocCache).await?;

    let mut log_block = cf.log.create_block().await?;
    let log_var_names = [
        "stateEstimate.x",
        "stateEstimate.y",
        "stateEstimate.z",
        "stateEstimate.vx",
        "stateEstimate.vy",
        "stateEstimate.yaw",
        // "stateEstimate.vz",
    ];

    for var_name in log_var_names {
        log_block.add_variable(var_name).await?;
    }

    let log_stream = log_block.start(LogPeriod::from_millis(10).unwrap()).await?;

    let (tx, _rx) = broadcast::channel(64);
    let (watch_tx, _watch_rx) = watch::channel(Telemetry::default());
    let local_sender_tx = tx.clone();
    let local_watch_tx = watch_tx.clone();
    tokio::spawn(async move {
        loop {
            match log_stream.next().await {
                Ok(data) => {
                    let telemetry = Telemetry::from_log_data(data);
                    let _ = local_sender_tx.send(telemetry);
                    let _ = local_watch_tx.send(telemetry);
                }
                Err(_) => break,
            }
        }
    });
    Ok(CrazyflieCommandUnit {
        cf,
        telemetry_sender: tx,
        telemetry_watch_sender: watch_tx,
    })
}

pub struct CrazyflieCommandUnit {
    cf: Crazyflie,
    telemetry_sender: broadcast::Sender<Telemetry>,
    telemetry_watch_sender: watch::Sender<Telemetry>,
}

impl CommandUnit for CrazyflieCommandUnit {
    async fn run_mission(&self, mission: Vec<Command>) -> Res<()> {
        let high_level_commander = &self.cf.high_level_commander;
        let commander = &self.cf.commander;
        // Reset the x,y,z,yaw estimated values before a new flight
        self.cf
            .param
            .set_lossy("kalman.resetEstimation", 1.0)
            .await?;
        sleep(Duration::from_millis(100)).await;
        self.cf
            .param
            .set_lossy("kalman.resetEstimation", 0.0)
            .await?;

        for command in mission {
            match command {
                Command::Takeoff { height, duration } => {
                    println!("Take Off...");
                    high_level_commander
                        .take_off(height.0, None, duration.as_secs_f32(), None)
                        .await?;
                    sleep(duration).await;
                }
                Command::Move { x, y, z, duration } => {
                    println!("Moving...");
                    high_level_commander
                        .go_to(
                            x.0,
                            y.0,
                            z.0,
                            0.0,
                            duration.as_secs_f32(),
                            true,
                            false,
                            None,
                        )
                        .await?;
                    sleep(duration).await;
                }
                Command::MoveToWaypoint { x, y, z, duration } => {
                    println!("Moving to point...");
                    high_level_commander
                        .go_to(
                            x.0,
                            y.0,
                            z.0,
                            0.0,
                            duration.as_secs_f32(),
                            false,
                            false,
                            None,
                        )
                        .await?;
                    sleep(duration).await;
                }
                Command::Land { duration } => {
                    println!("Landing...");
                    high_level_commander
                        .land(0.0, None, duration.as_secs_f32(), None)
                        .await?;
                    sleep(duration).await;
                }
                Command::Hover { duration } => sleep(duration).await,
                Command::BilliardBox(params) => {
                    run_billiard_loop(
                        params,
                        high_level_commander,
                        commander,
                        self.telemetry_watch_sender.subscribe(),
                    )
                    .await?
                }
                Command::SmoothPath { waypoints, speed } => {
                    run_smooth_path(
                        waypoints,
                        commander,
                        speed,
                        self.telemetry_watch_sender.subscribe(),
                    )
                    .await?
                }
            }
        }
        Ok(())
    }

    fn telemetry(&self) -> broadcast::Receiver<Telemetry> {
        self.telemetry_sender.subscribe()
    }

    fn latest_telemetry(&self) -> watch::Receiver<Telemetry> {
        self.telemetry_watch_sender.subscribe()
    }
}
