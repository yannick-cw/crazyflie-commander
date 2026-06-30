use utils::errors::MissionError::FailedToConnect;

pub mod utils;
pub mod control;

fn main() {
    let a = FailedToConnect("ASSSSDSASD 438i952".to_string());
    println!("Err: {}", a);
}
