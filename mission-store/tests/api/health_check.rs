use crate::setup::{get, spawn_app};
use std::error::Error;

#[tokio::test]
async fn health() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;

    let response = get(format!("{endpoint}/health_check"), &client).await?;

    assert!(response.status().is_success());

    Ok(())
}
