use crate::control::command_unit::{Command, CommandUnit, Telemetry};
use crate::utils::errors::MissionError::FailedToConnect;
use crate::utils::errors::Res;
use crazyflie_lib::Crazyflie;
use crazyflie_lib::subsystems::log::LogPeriod;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Receiver;

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
        "stateEstimate.vz",
    ];

    for var_name in log_var_names {
        log_block.add_variable(var_name).await?;
    }

    let log_stream = log_block
        .start(LogPeriod::from_millis(100).unwrap())
        .await?;

    let (tx, _rx) = broadcast::channel(64);
    let local_sender_tx = tx.clone();
    tokio::spawn(async move {
        loop {
            match log_stream.next().await {
                Ok(data) => {
                    let _ = local_sender_tx.send(Telemetry::from_log_data(data));
                }
                Err(_) => break,
            }
        }
    });
    Ok(CrazyflieCommandUnit {
        cf,
        telemetry_sender: tx,
    })
}

pub struct CrazyflieCommandUnit {
    cf: Crazyflie,
    telemetry_sender: broadcast::Sender<Telemetry>,
}

impl CommandUnit for CrazyflieCommandUnit {
    async fn run_mission(&self, mission: Vec<Command>) -> Res<()> {
        let high_level_commander = &self.cf.high_level_commander;
        for command in mission {
            match command {
                Command::TakeOff { height, duration } => {
                    println!("Take Off...");
                    high_level_commander
                        .take_off(height.0, None, duration.as_secs_f32(), None)
                        .await?;
                    tokio::time::sleep(duration).await;
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
                    tokio::time::sleep(duration).await;
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
                    tokio::time::sleep(duration).await;
                }
                Command::Land { duration } => {
                    println!("Landing...");
                    high_level_commander
                        .land(0.0, None, duration.as_secs_f32(), None)
                        .await?;
                    tokio::time::sleep(duration).await;
                }
                Command::Hover { duration } => tokio::time::sleep(duration).await,
            }
        }
        Ok(())
    }

    fn telemetry(&self) -> Receiver<Telemetry> {
        self.telemetry_sender.subscribe()
    }
}
