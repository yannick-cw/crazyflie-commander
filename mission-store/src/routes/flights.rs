use crate::domain::{Flight, ValidName};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use drone_control::Telemetry;
use sqlx::error::ErrorKind::ForeignKeyViolation;
use sqlx::types::Uuid;
use sqlx::{Error, PgPool};
use tracing::{Instrument, error, info, info_span, warn};

#[tracing::instrument(skip(pg_pool, flight))]
pub async fn post_flight(
    Path(flight_name): Path<ValidName>,
    State(pg_pool): State<PgPool>,
    Json(flight): Json<Flight>,
) -> StatusCode {
    let tele_json = serde_json::to_value(flight.telemetry).unwrap();
    let response = sqlx::query!(
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
    .await
    .map_or_else(
        |err| match err {
            Error::Database(db_err) if db_err.kind() == ForeignKeyViolation => {
                warn!(
                    "Failed to save flight with mission: {:?} does not exist",
                    flight.mission,
                );
                StatusCode::BAD_REQUEST
            }
            err => {
                error!("Failed to save flight with: {err:?}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        },
        |_| StatusCode::CREATED,
    );
    info!("New flight saved");
    response
}

#[tracing::instrument(skip(pg_pool))]
pub async fn get_flight(
    Path(flight_name): Path<ValidName>,
    State(pg_pool): State<PgPool>,
) -> Result<Json<Flight>, StatusCode> {
    sqlx::query!(
        r#"
        SELECT date, telemetry AS "telemetry: sqlx::types::Json<Vec<Telemetry>>", mission as "mission: ValidName" FROM flights
        WHERE name = $1
        "#,
        flight_name.as_ref(),
    )
        .fetch_optional(&pg_pool)
        .instrument(info_span!("Fetch from db"))
        .await
        .inspect_err(|err| error!("{err}"))
        .map_or(Err(StatusCode::INTERNAL_SERVER_ERROR), |res| {
            res.map_or(
                Err(StatusCode::NOT_FOUND), |record|
                    Ok(Json(Flight {
                        date: record.date,
                        telemetry: record.telemetry.0,
                        mission: record.mission,
                    })))
        })
}
