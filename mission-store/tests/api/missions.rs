use crate::setup::{get, post, spawn_app};
use reqwest::StatusCode;
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

    let response = post(
        format!("{endpoint}/missions/test-mission"),
        &client,
        &json_mission,
    )
    .await?;

    assert_eq!(StatusCode::CREATED, response.status());

    Ok(())
}

#[tokio::test]
async fn retrieve_mission() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;

    let mission = r#"
        [
            { "Takeoff": { "height": 0.5, "duration": { "secs": 2, "nanos": 0 } } },
            { "Land": { "duration": { "secs": 2, "nanos": 0 } } }
        ]"#;
    let json_mission: Value = serde_json::from_str(mission)?;

    let _ = post(
        format!("{endpoint}/missions/test-mission"),
        &client,
        &json_mission,
    )
    .await?;
    let mission: Value = get(format!("{endpoint}/missions/test-mission"), &client)
        .await?
        .json()
        .await?;

    assert_eq!(json_mission, mission);

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

    let response = post(
        format!("{endpoint}/missions/test-mission"),
        &client,
        &json_mission,
    )
    .await?;

    assert_eq!(StatusCode::UNPROCESSABLE_ENTITY, response.status());

    Ok(())
}
