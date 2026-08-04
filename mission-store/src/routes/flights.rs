use crate::domain::Error::{NotFound, UnexpectedError, ValidationError};
use crate::domain::{Flight, Res, ValidName};
use anyhow::Context;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use drone_control::Telemetry;
use sqlx::PgPool;
use sqlx::error::ErrorKind::ForeignKeyViolation;
use sqlx::types::Uuid;
use tracing::{Instrument, info, info_span};

#[tracing::instrument(skip(pg_pool, flight))]
pub async fn post_flight(
    Path(flight_name): Path<ValidName>,
    State(pg_pool): State<PgPool>,
    Json(flight): Json<Flight>,
) -> Res<StatusCode> {
    let tele_json = serde_json::to_value(flight.telemetry).unwrap();
    let insert_res = sqlx::query!(
        r#"
        INSERT INTO flights (id, name, date, telemetry, mission)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        Uuid::new_v4(),
        flight_name.as_ref(),
        flight.date,
        tele_json,
        flight.mission.as_ref().map(|a| a.as_ref())
    )
    .execute(&pg_pool)
    .instrument(info_span!("INSERT to db"))
    .await;

    insert_res.map_err(|err| match err {
        sqlx::Error::Database(db_err) if db_err.kind() == ForeignKeyViolation => {
            ValidationError(format!(
                "Referenced mission `{}` does not exist",
                flight.mission.as_deref().unwrap_or("")
            ))
        }
        err => UnexpectedError(anyhow::Error::new(err).context("Failed inserting flight into db.")),
    })?;

    info!("New flight saved");
    Ok(StatusCode::CREATED)
}

#[tracing::instrument(skip(pg_pool))]
pub async fn get_flight(
    Path(flight_name): Path<ValidName>,
    State(pg_pool): State<PgPool>,
) -> Res<Json<Flight>> {
    let res = sqlx::query!(
        r#"
        SELECT date, telemetry AS "telemetry: sqlx::types::Json<Vec<Telemetry>>", mission as "mission: ValidName" FROM flights
        WHERE name = $1
        "#,
        flight_name.as_ref(),
    )
        .fetch_optional(&pg_pool)
        .instrument(info_span!("Fetch from db"))
        .await
        .context("Failed fetching flight from db")?;

    res.map_or(
        Err(NotFound(format!("flight: {}", flight_name.as_ref()))),
        |record| {
            Ok(Json(Flight {
                date: record.date,
                telemetry: record.telemetry.0,
                mission: record.mission,
            }))
        },
    )
}
