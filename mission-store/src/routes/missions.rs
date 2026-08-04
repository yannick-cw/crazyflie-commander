use crate::domain::ValidName;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use drone_control::Command;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::chrono::Utc;
use tracing::{Instrument, error, info, info_span};

#[tracing::instrument(skip(pg_pool, mission))]
pub async fn post_mission(
    Path(mission_name): Path<ValidName>,
    // todo not the nicest testable form of dependency injection
    // should be rather just something more high level
    State(pg_pool): State<PgPool>,
    Json(mission): Json<Vec<Command>>,
) -> StatusCode {
    let mission_json = serde_json::to_value(mission).unwrap();

    let response = sqlx::query!(
        r#"
        INSERT INTO missions (id, name, commands, created_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        mission_name.as_ref(),
        mission_json,
        Utc::now()
    )
    .execute(&pg_pool)
    .instrument(info_span!("INSERT to db"))
    .await
    .inspect_err(|err| error!("Failed to save mission with: {err:?}"))
    .map_or(StatusCode::INTERNAL_SERVER_ERROR, |_| StatusCode::CREATED);

    info!("New mission saved");
    response
}

#[tracing::instrument(skip(pg_pool))]
pub async fn get_mission(
    Path(mission_name): Path<ValidName>,
    State(pg_pool): State<PgPool>,
) -> Result<Json<Vec<Command>>, StatusCode> {
    sqlx::query!(
        r#"
        SELECT commands as "commands: sqlx::types::Json<Vec<Command>>" FROM missions
        WHERE name = $1
        "#,
        mission_name.as_ref(),
    )
    .fetch_optional(&pg_pool)
    .instrument(info_span!("Fetch from db"))
    .await
    .inspect_err(|err| error!("{err}"))
    .map_or(Err(StatusCode::INTERNAL_SERVER_ERROR), |res| {
        res.map_or(Err(StatusCode::NOT_FOUND), |record| {
            Ok(Json(record.commands.0))
        })
    })
}
