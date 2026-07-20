/// Possible Errors are also (`Sync`, `Send, 'static`)
#[derive(Debug, thiserror::Error)]
pub enum MissionError {
    #[error("Failed to establish connection :{0}")]
    FailedToConnect(String),
    #[error("Failed link discovery :{0}")]
    LinkFailure(#[from] crazyflie_link::Error),
    #[error("Failed to establish connection :{0}")]
    ConnectionFailure(#[from] crazyflie_lib::Error),
    #[error("Failed uploading trajectory :{0}")]
    UploadError(String),
    #[error("Failed while rendering")]
    RenderFailure,
}
/// Result type for interacting with this crate.
pub type Res<A> = Result<A, MissionError>;
