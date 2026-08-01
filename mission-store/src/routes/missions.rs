use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use drone_control::Command;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::chrono::Utc;

pub async fn post_mission(
    Path(mission_name): Path<String>,
    // todo not the nicest testable form of dependency injection
    // should be rather just something more high level
    State(pg_pool): State<PgPool>,
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
    .execute(&pg_pool)
    .await
    // todo switch to trace logging
    .inspect_err(|err| println!("{err}"))
    .map_or(StatusCode::INTERNAL_SERVER_ERROR, |_| StatusCode::CREATED)
}

pub async fn get_mission(
    Path(mission_name): Path<String>,
    State(pg_pool): State<PgPool>,
) -> Result<Json<Vec<Command>>, StatusCode> {
    let fetch_res = sqlx::query!(
        r#"
        SELECT commands FROM missions
        WHERE name = $1
        "#,
        mission_name,
    )
    .fetch_optional(&pg_pool)
    .await
    .inspect_err(|err| println!("{err}"))
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);

    fetch_res.and_then(|maybe_row| match maybe_row {
        None => Err(StatusCode::NOT_FOUND),
        Some(record) => serde_json::from_value(record.commands)
            .map(|cmds: Vec<Command>| Json(cmds))
            .inspect_err(|err| println!("{err}"))
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR)),
    })
}
