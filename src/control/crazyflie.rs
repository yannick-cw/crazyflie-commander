use crate::control::command_unit::{Command, CommandUnit, Telemetry, Waypoint};
use crate::utils::errors::MissionError::FailedToConnect;
use crate::utils::errors::Res;
use crazyflie_lib::Crazyflie;
use crazyflie_lib::subsystems::log::LogPeriod;
use std::time::Duration;
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
        .start(LogPeriod::from_millis(500).unwrap())
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
    async fn run_mission(&self, _mission: Vec<Command>) -> Res<Vec<Waypoint>> {
        self.cf
            .high_level_commander
            .take_off(0.5, None, 3.0, None)
            .await?;
        println!("Flying...");
        tokio::time::sleep(Duration::from_secs(6)).await;
        println!("forward...");
        self.cf
            .high_level_commander
            .go_to(1.0, 0.0, 0.2, 0.0, 2.0, true, true, None)
            .await?;
        tokio::time::sleep(Duration::from_secs(4)).await;
        println!("back...");
        self.cf
            .high_level_commander
            .go_to(-1.0, 0.0, 0.0, 0.0, 2.0, true, true, None)
            .await?;
        tokio::time::sleep(Duration::from_secs(6)).await;
        println!("Landing...");
        self.cf
            .high_level_commander
            .land(0.0, None, 2.0, None)
            .await?;
        tokio::time::sleep(Duration::from_secs(4)).await;

        let _pos_x: u16 = self.cf.param.get("whateverpositionwillbe").await?;
        Ok(vec![Waypoint::default()])
    }

    fn telemetry(&self) -> Receiver<Telemetry> {
        self.telemetry_sender.subscribe()
    }
}
