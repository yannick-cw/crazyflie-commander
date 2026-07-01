use crate::control::command_unit::{Command, CommandUnit, Waypoint};
use crate::utils::errors::MissionError;
use crate::utils::errors::MissionError::FailedToConnect;
use crazyflie_lib::Crazyflie;

pub async fn setup_link() -> Result<CrazyflieCommandUnit, MissionError> {
    let link_context = crazyflie_link::LinkContext::new();
    let found = link_context.scan([0xE7; 5]).await?;

    let uri = found
        .first()
        .ok_or(FailedToConnect("Did not find crazyflie".to_string()))?;

    let cf = Crazyflie::connect_from_uri(&link_context, uri, crazyflie_lib::NoTocCache).await?;

    println!("List of params variables: ");
    cf.param
        .names()
        .iter()
        .for_each(|name| println!(" - {}", name));

    println!("List of log variables: ");
    cf.log
        .names()
        .iter()
        .for_each(|name| println!(" - {}", name));

    Ok(CrazyflieCommandUnit { cf })
}

pub struct CrazyflieCommandUnit {
    cf: Crazyflie,
}

impl CommandUnit for CrazyflieCommandUnit {
    async fn run_mission(&self, _mission: Vec<Command>) -> Result<Vec<Waypoint>, MissionError> {
        self.cf
            .high_level_commander
            .take_off(1.0, None, 3.0, None)
            .await?;

        let _pos_x: u16 = self.cf.param.get("whateverpositionwillbe").await?;
        Ok(vec![Waypoint::default()])
    }
}
