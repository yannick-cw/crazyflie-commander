use crate::setup::spawn_app;
use std::error::Error;

#[tokio::test]
async fn health() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;

    let response = client
        .get(format!("{endpoint}/health_check"))
        .send()
        .await?;

    assert!(response.status().is_success());

    Ok(())
}
