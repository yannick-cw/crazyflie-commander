use crate::setup::spawn_app_pg;
use axum::http::StatusCode;
use chrono::Utc;
use mission_store::idempotency::{CleanResult, clean_outdated};
use reqwest::header;
use serde_json::json;
use std::error::Error;

#[tokio::test]
async fn cleanup_old_keys() -> Result<(), Box<dyn Error>> {
    let (endpoint, client, pool) = spawn_app_pg().await?;
    let json_flight = json!({
        "date": "2026-08-04T08:23:42.508923Z",
        "telemetry": [],
    });

    let (k1, k2, k3) = ("a", "b", "c");

    let insert = |name: &str| {
        let idemp_header = header::HeaderValue::from_str(name);
        client
            .post(format!("{endpoint}/flights/{name}"))
            .header("Idempotency-Key", idemp_header.unwrap())
            .json(&json_flight)
            .send()
    };

    insert(k1).await?;
    insert(k2).await?;
    let cutoff_time = Utc::now();
    insert(k3).await?;

    // cleans idempotency keys for k1 and k2
    // (cleans one at a time)
    let clean_res1 = clean_outdated(cutoff_time, &pool).await?;
    let clean_res2 = clean_outdated(cutoff_time, &pool).await?;
    let clean_res3 = clean_outdated(cutoff_time, &pool).await?;
    assert_eq!(CleanResult::Deletion, clean_res1);
    assert_eq!(CleanResult::Deletion, clean_res2);
    assert_eq!(CleanResult::EmptyQueue, clean_res3);

    // idempotency key still in DB - meaning a retry still gives 201
    let still_fine = insert(k3).await?;
    // idempotency key gone from DB - retry gives 409
    let now_duplicate1 = insert(k1).await?;
    let now_duplicate2 = insert(k2).await?;

    assert_eq!(StatusCode::CONFLICT, now_duplicate1.status());
    assert_eq!(StatusCode::CONFLICT, now_duplicate2.status());
    assert_eq!(StatusCode::CREATED, still_fine.status());

    Ok(())
}
