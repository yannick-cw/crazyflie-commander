
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum MissionError {
    #[error("Failed to establish connection :{0}")]
    FailedToConnect(String),
}
