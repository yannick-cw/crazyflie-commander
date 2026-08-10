use crate::setup::{get, post, spawn_app};
use mission_store::domain::MissionResponse;
use reqwest::StatusCode;
use serde_json::Value;
use std::error::Error;

pub fn simple_mission() -> &'static str {
    r#"
        [
            { "Takeoff": { "height": 0.5, "duration": { "secs": 2, "nanos": 0 } } },
            { "Land": { "duration": { "secs": 2, "nanos": 0 } } }
        ]"#
}

#[tokio::test]
async fn submit_mission() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;

    let json_mission: Value = serde_json::from_str(simple_mission())?;

    let response = post(
        format!("{endpoint}/missions/test_mission"),
        &client,
        &json_mission,
    )
    .await?;

    assert_eq!(StatusCode::CREATED, response.status());

    Ok(())
}

#[tokio::test]
async fn get_409_conflict_on_double_submit() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;

    let json_mission: Value = serde_json::from_str(simple_mission())?;

    let create = post(
        format!("{endpoint}/missions/test_mission"),
        &client,
        &json_mission,
    )
    .await?;

    let conflict = post(
        format!("{endpoint}/missions/test_mission"),
        &client,
        &json_mission,
    )
    .await?;

    assert_eq!(StatusCode::CREATED, create.status());
    assert_eq!(StatusCode::CONFLICT, conflict.status());

    Ok(())
}

#[tokio::test]
async fn retrieve_mission() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;

    let mission_name = "test_mission";

    let json_mission: Value = serde_json::from_str(simple_mission())?;

    let response = post(
        format!("{endpoint}/missions/{mission_name}"),
        &client,
        &json_mission,
    )
    .await?;
    assert_eq!(StatusCode::CREATED, response.status());

    let mission: Value = get(format!("{endpoint}/missions/{mission_name}"), &client)
        .await?
        .json()
        .await?;

    assert_eq!(json_mission, mission);

    Ok(())
}

#[tokio::test]
async fn retrieve_all_mission() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;

    let mission_name = "test_mission";
    let json_mission: Value = serde_json::from_str(simple_mission())?;
    let mission_names: Vec<_> = (1..10).map(|i| format!("{mission_name}{i}")).collect();

    for n in mission_names.clone() {
        post(format!("{endpoint}/missions/{n}"), &client, &json_mission).await?;
    }

    let missions: Vec<MissionResponse> = get(format!("{endpoint}/missions"), &client)
        .await?
        .json()
        .await?;

    let mut result_names: Vec<_> = missions.into_iter().map(|m| m.name.inner()).collect();
    result_names.sort();

    assert_eq!(mission_names, result_names);

    Ok(())
}

#[tokio::test]
async fn get_404() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;
    let mission_name = "no_mission";

    let response = get(format!("{endpoint}/missions/{mission_name}"), &client).await?;

    assert_eq!(StatusCode::NOT_FOUND, response.status());
    assert_eq!("Did not find `mission: no_mission`", response.text().await?);

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
    assert_eq!(
        "Failed to deserialize the JSON body into the target type: \
        [0]: unknown variant `TakeXXX`, expected one of `Takeoff`, \
        `Move`, `MoveToWaypoint`, `SmoothPath`, \
        `Setpoints`, `BilliardBox`, `Orbit`, `Hover`, `Land`, \
        `OnVehicleTrajectory` at line 1 column 11",
        response.text().await?
    );

    Ok(())
}

#[tokio::test]
async fn reject_invalid_mission_names() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;
    let json_mission: Value = serde_json::from_str(simple_mission())?;

    let bad_names = [("%><", "weird chars"), (&"x".repeat(300), "too long name")];

    for (name, descr) in bad_names {
        let response = post(
            format!("{endpoint}/missions/{name}"),
            &client,
            &json_mission,
        )
        .await?;

        assert_eq!(
            StatusCode::BAD_REQUEST,
            response.status(),
            "should reject name: {} as {}",
            name,
            descr
        );
        assert!(&response.text().await?.starts_with("Invalid URL"));
    }

    Ok(())
}
