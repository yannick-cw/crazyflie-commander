use crate::missions::simple_mission;
use crate::setup::{get, post, spawn_app};
use drone_control::Telemetry;
use reqwest::StatusCode;
use serde_json::{Value, json};
use std::error::Error;

fn test_telemetry() -> Vec<Telemetry> {
    vec![
        Telemetry {
            x: Default::default(),
            y: Default::default(),
            z: Default::default(),
            x_v: Default::default(),
            y_v: Default::default(),
            yaw_degrees: 0.0,
            battery_level: Default::default(),
            range_front: Default::default(),
            range_back: Default::default(),
            range_right: Default::default(),
            range_left: Default::default(),
            range_up: Default::default(),
        };
        200
    ]
}

#[tokio::test]
async fn upload_flight() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;
    let json_flight = json!({
        "date": "2026-08-04T08:23:42.508923Z",
        "telemetry": test_telemetry(),
        "mission_id": None::<String>
    });

    let response = post(
        format!("{endpoint}/flights/test_flight"),
        &client,
        &json_flight,
    )
    .await?;

    assert_eq!(StatusCode::CREATED, response.status());

    Ok(())
}

#[tokio::test]
async fn duplicate_upload_409() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;
    let json_flight = json!({
        "date": "2026-08-04T08:23:42.508923Z",
        "telemetry": test_telemetry(),
        "mission_id": None::<String>
    });

    let created = post(
        format!("{endpoint}/flights/test_flight"),
        &client,
        &json_flight,
    )
    .await?;

    let conflict = post(
        format!("{endpoint}/flights/test_flight"),
        &client,
        &json_flight,
    )
    .await?;

    assert_eq!(StatusCode::CREATED, created.status());
    assert_eq!(StatusCode::CONFLICT, conflict.status());

    Ok(())
}

#[tokio::test]
async fn upload_flight_with_mission() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;

    let mission = "test_mission";

    let json_flight = json!({
        "date": "2026-08-04T08:23:42.508923Z",
        "telemetry": test_telemetry(),
        "mission": Some(mission)
    });

    // set up a mission
    let json_mission: Value = serde_json::from_str(simple_mission())?;

    let _ = post(
        format!("{endpoint}/missions/{mission}"),
        &client,
        &json_mission,
    )
    .await?;

    let response = post(
        format!("{endpoint}/flights/test_flight"),
        &client,
        &json_flight,
    )
    .await?;

    assert_eq!(StatusCode::CREATED, response.status());

    Ok(())
}

#[tokio::test]
async fn reject_non_existing_mission() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;

    let json_flight = json!({
        "date": "2026-08-04T08:23:42.508923Z",
        "telemetry": test_telemetry(),
        "mission": Some("non_existing_mission")
    });

    let response = post(
        format!("{endpoint}/flights/test_flight"),
        &client,
        &json_flight,
    )
    .await?;

    assert_eq!(StatusCode::BAD_REQUEST, response.status());
    assert_eq!(
        "Referenced mission `non_existing_mission` does not exist",
        response.text().await?
    );

    Ok(())
}

#[tokio::test]
async fn get_flight() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;

    let json_flight = json!({
        "date": "2026-08-04T08:23:42.508923Z",
        "telemetry": test_telemetry(),
        "mission": None::<String>
    });

    let _ = post(
        format!("{endpoint}/flights/test_flight"),
        &client,
        &json_flight,
    )
    .await?;

    let flight: Value = get(format!("{endpoint}/flights/test_flight"), &client)
        .await?
        .json()
        .await?;

    assert_eq!(json_flight, flight);

    Ok(())
}

#[tokio::test]
async fn get_404() -> Result<(), Box<dyn Error>> {
    let (endpoint, client) = spawn_app().await?;
    let flight_name = "no_flight";

    let response = get(format!("{endpoint}/flights/{flight_name}"), &client).await?;

    assert_eq!(StatusCode::NOT_FOUND, response.status());
    assert_eq!("Did not find `flight: no_flight`", response.text().await?);

    Ok(())
}
