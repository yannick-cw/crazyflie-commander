use crate::setup::spawn_app;
use reqwest::{Client, Response, StatusCode};
use serde_json::Value;
use std::error::Error;

#[tokio::test]
async fn submit_mission() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;

    let mission = r#"
        [
            { "Takeoff": { "height": 0.5, "duration": { "secs": 2, "nanos": 0 } } },
            { "Land": { "duration": { "secs": 2, "nanos": 0 } } }
        ]"#;
    let json_mission: Value = serde_json::from_str(mission)?;

    let response = post(endpoint, client, &json_mission).await?;

    assert_eq!(StatusCode::CREATED, response.status());

    Ok(())
}

#[tokio::test]
async fn fail_submit_invalid_missions() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;

    let broken_format = r#"
        [
            { "TakeXXX": { "height": 0.5, "duration": { "secs": 2, "nanos": 0 } } }
        ]"#;
    let json_mission: Value = serde_json::from_str(broken_format)?;

    let response = post(endpoint, client, &json_mission).await?;

    assert_eq!(StatusCode::UNPROCESSABLE_ENTITY, response.status());

    Ok(())
}

async fn post(
    endpoint: String,
    client: Client,
    json_mission: &Value,
) -> Result<Response, reqwest::Error> {
    client
        .post(format!("{endpoint}/missions/test-mission"))
        .json(&json_mission)
        .send()
        .await
}
