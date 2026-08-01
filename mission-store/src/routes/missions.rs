use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use drone_control::Command;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::chrono::Utc;
use std::sync::Arc;

pub async fn post_mission(
    Path(mission_name): Path<String>,
    // todo not the nicest testable form of dependency injection
    // should be rather just something more high level
    State(pg_pool): State<Arc<PgPool>>,
    Json(mission): Json<Vec<Command>>,
) -> StatusCode {
    let mission_json = serde_json::to_value(mission).unwrap();

    sqlx::query!(
        r#"
        INSERT INTO missions (id, name, commands, created_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        mission_name,
        mission_json,
        Utc::now()
    )
    .execute(pg_pool.as_ref())
    .await
    // todo switch to trace logging
    .inspect_err(|err| println!("{err}"))
    .map_or(StatusCode::INTERNAL_SERVER_ERROR, |_| StatusCode::CREATED)
}

pub async fn get_mission(Path(mission_name): Path<String>) -> Json<Option<Vec<Command>>> {
    Json(Some(Vec::new()))
}
