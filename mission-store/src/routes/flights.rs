use crate::domain::Error::{Conflict, NotFound, UnexpectedError, ValidationError};
use crate::domain::{Flight, Label, Res, ValidName};
use crate::idempotency::{IdempotencyKey, try_processing, update_idempotent_response};
use anyhow::Context;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use axum_extra::TypedHeader;
use mission_computer::Telemetry;
use sqlx::error::ErrorKind::ForeignKeyViolation;
use sqlx::types::Uuid;
use sqlx::{PgPool, PgTransaction};
use tracing::{Instrument, info, info_span};

#[tracing::instrument(skip(pg_pool, flight, maybe_idem))]
pub async fn post_flight(
    Path(flight_name): Path<ValidName>,
    label: Extension<Label>,
    maybe_idem: Option<TypedHeader<IdempotencyKey>>,
    State(pg_pool): State<PgPool>,
    Json(flight): Json<Flight>,
) -> Res<Response> {
    let mut transaction = pg_pool
        .begin()
        .await
        .context("could not init transaction")?;

    let res = match maybe_idem {
        None => insert_flight(&flight_name, &mut transaction, flight).await,
        Some(idem) => {
            match try_processing(&mut transaction, &flight_name, &idem.0, &label.0).await? {
                None => {
                    let response = insert_flight(&flight_name, &mut transaction, flight).await?;
                    update_idempotent_response(
                        &mut transaction,
                        &flight_name,
                        &idem.0,
                        &label.0,
                        response,
                    )
                    .await
                }
                // this is a retry - return response from DB
                Some(res) => Ok(res),
            }
        }
    }?;

    transaction
        .commit()
        .await
        .context("could not commit transaction")?;
    Ok(res)
}

async fn insert_flight(
    flight_name: &ValidName,
    pool: &mut PgTransaction<'_>,
    flight: Flight,
) -> Res<Response> {
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
    .execute(&mut **pool)
    .instrument(info_span!("INSERT to db"))
    .await;

    insert_res.map_err(|err| match err {
        sqlx::Error::Database(db_err) if db_err.kind() == ForeignKeyViolation => {
            ValidationError(format!(
                "Referenced mission `{}` does not exist",
                flight.mission.as_deref().unwrap_or("")
            ))
        }
        sqlx::Error::Database(db_err)
            if db_err.is_unique_violation() && db_err.constraint() == Some("flights_name_key") =>
        {
            Conflict(format!("flight name `{}` exists.", flight_name.as_ref()))
        }
        err => UnexpectedError(anyhow::Error::new(err).context("Failed inserting flight into db.")),
    })?;

    info!("New flight saved");
    Ok(StatusCode::CREATED.into_response())
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
