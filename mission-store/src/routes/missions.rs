use crate::domain::Error::NotFound;
use crate::domain::{MissionResponse, Res, ValidName};
use anyhow::Context;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use drone_control::Command;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::chrono::Utc;
use tracing::{Instrument, info, info_span};

#[tracing::instrument(skip(pg_pool, mission))]
pub async fn post_mission(
    Path(mission_name): Path<ValidName>,
    State(pg_pool): State<PgPool>,
    Json(mission): Json<Vec<Command>>,
) -> Res<StatusCode> {
    let mission_json = serde_json::to_value(mission).unwrap();

    sqlx::query!(
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
    .context(format!(
        "Failed to insert mission {} to db",
        mission_name.as_ref()
    ))?;

    info!("New mission saved");
    Ok(StatusCode::CREATED)
}

#[tracing::instrument(skip(pg_pool))]
pub async fn get_mission(
    Path(mission_name): Path<ValidName>,
    State(pg_pool): State<PgPool>,
) -> Res<Json<Vec<Command>>> {
    let res = sqlx::query!(
        r#"
        SELECT commands as "commands: sqlx::types::Json<Vec<Command>>" FROM missions
        WHERE name = $1
        "#,
        mission_name.as_ref(),
    )
    .fetch_optional(&pg_pool)
    .instrument(info_span!("Fetch from db"))
    .await
    .context("Failed fetching mission from db")?;

    res.map_or(
        Err(NotFound(format!("mission: {}", mission_name.as_ref()))),
        |record| Ok(Json(record.commands.0)),
    )
}
#[tracing::instrument(skip(pg_pool))]
pub async fn list_missions(State(pg_pool): State<PgPool>) -> Res<Json<Vec<MissionResponse>>> {
    let res = sqlx::query!(
        r#"
        SELECT name as "name: ValidName", commands as "commands: sqlx::types::Json<Vec<Command>>" FROM missions
        "#,
    )
    .fetch_all(&pg_pool)
    .instrument(info_span!("Fetch from db"))
    .await
    .context("Failed fetching missions from db")?;

    Ok(Json(
        res.into_iter()
            .map(|res| MissionResponse {
                name: res.name,
                mission: res.commands.0,
            })
            .collect(),
    ))
}
