use crate::domain::Error::UnexpectedError;
use crate::domain::{Label, Res};
use anyhow::Context;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use axum_extra::headers::{Error, Header};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, PgTransaction};
use tracing::info;

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Hash)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    fn build(&self, scope: &str) -> String {
        format!("{}_{}", self.0, scope)
    }
}

impl TryFrom<String> for IdempotencyKey {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || value.len() >= 50 {
            Err("Idempotency key must be between 1 and 50 characters long".to_string())
        } else {
            Ok(IdempotencyKey(value))
        }
    }
}
impl Header for IdempotencyKey {
    fn name() -> &'static HeaderName {
        static IDEMPOTENCY_KEY: HeaderName = HeaderName::from_static("idempotency-key");
        &IDEMPOTENCY_KEY
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, Error>
    where
        Self: Sized,
        I: Iterator<Item = &'i HeaderValue>,
    {
        values
            .next()
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.to_string().try_into().ok())
            .ok_or_else(Error::invalid)
    }

    fn encode<E: Extend<HeaderValue>>(&self, values: &mut E) {
        values.extend(std::iter::once(HeaderValue::from_str(&self.0).unwrap()));
    }
}

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "header_pair")]
struct HeaderPair {
    k: String,
    v: Vec<u8>,
}

#[tracing::instrument(skip(pool, idempotency_key))]
pub async fn try_processing(
    pool: &mut PgTransaction<'_>,
    scope: &str,
    idempotency_key: &IdempotencyKey,
    label: &Label,
) -> Res<Option<Response>> {
    let try_start = sqlx::query!(
        r#"
                  INSERT INTO idempotency (label, idempotency_key, created_at)
                  VALUES ($1, $2, $3)
                  on conflict do nothing

        "#,
        label.0,
        idempotency_key.build(scope),
        Utc::now()
    )
    .execute(&mut **pool)
    .await
    .context("failed inserting idempotency key to DB")?;

    // inserted and it's our turn, no response yet
    if try_start.rows_affected() > 0 {
        Ok(None)
    } else {
        match fetch_saved_response(pool, scope, idempotency_key, label).await? {
            None => Err(UnexpectedError(anyhow::anyhow!(
                "There should be a saved response"
            ))),
            Some(res) => Ok(Some(res)),
        }
    }
}

#[tracing::instrument(skip(pool, idempotency_key))]
async fn fetch_saved_response(
    pool: &mut PgTransaction<'_>,
    scope: &str,
    idempotency_key: &IdempotencyKey,
    label: &Label,
) -> Res<Option<Response>> {
    let saved_res = sqlx::query!(
        r#"
            SELECT response_status_code as "response_status_code!",
                   response_body,
                   response_headers as "headers!: Vec<HeaderPair>"
            from idempotency
            where idempotency_key = $1 and
                  label = $2
        "#,
        idempotency_key.build(scope),
        label.0
    )
    .fetch_optional(&mut **pool)
    .await
    .context(format!(
        "Failed fetching idempotency key `{}` for label `{}`",
        idempotency_key.build(scope),
        label.0
    ))?;

    if let Some(r) = saved_res {
        let builder = Response::builder().status(
            StatusCode::from_u16(r.response_status_code as u16).context("invalid status code")?,
        );

        let with_headers = r
            .headers
            .into_iter()
            .fold(builder, |b, header| b.header(header.k, header.v));

        Ok(Some(
            with_headers
                .body(r.response_body.unwrap_or(vec![]).into())
                .context("Could not build body")?,
        ))
    } else {
        Ok(None)
    }
}

#[tracing::instrument(skip(pool, idempotency_key))]
pub async fn update_idempotent_response(
    pool: &mut PgTransaction<'_>,
    scope: &str,
    idempotency_key: &IdempotencyKey,
    label: &Label,
    response: Response,
) -> Res<Response> {
    let (head, body) = response.into_parts();
    let headers: Vec<HeaderPair> = head
        .headers
        .iter()
        .map(|(k, v)| HeaderPair {
            k: k.as_str().to_string(),
            v: v.as_bytes().to_vec(),
        })
        .collect();
    let body = to_bytes(body, usize::MAX)
        .await
        .context("could not read body")?;

    let _ = sqlx::query!(
        r#"
                  UPDATE idempotency
                  SET
                      response_status_code = $3,
                      response_headers = $4,
                      response_body = $5
                  WHERE
                      label = $1 AND
                      idempotency_key = $2
        "#,
        label.0,
        idempotency_key.build(scope),
        head.status.as_u16() as i16,
        headers as Vec<HeaderPair>,
        body.to_vec(),
    )
    .execute(&mut **pool)
    .await
    .context("failed inserting idempotency key to DB")?;

    Ok(Response::from_parts(head, Body::from(body)))
}

#[derive(Debug, PartialEq, PartialOrd)]
pub enum CleanResult {
    Deletion,
    EmptyQueue,
}
#[tracing::instrument(skip(pool))]
pub async fn clean_outdated(clean_older_since: DateTime<Utc>, pool: &PgPool) -> Res<CleanResult> {
    let mut tx = pool.begin().await.context("failed to start tx")?;

    // this is just for fun a few step process
    // FOR update locks this row during the transaction for other workers
    // SKIP locked skips rows locked by the same mechanism by other workers
    let maybe_row = sqlx::query!(
        r#"
        SELECT label, idempotency_key from idempotency
        WHERE created_at <= $1
        FOR UPDATE
        SKIP LOCKED
        LIMIT 1
    "#,
        clean_older_since
    )
    .fetch_optional(&mut *tx)
    .await
    .context("failed getting outdated idempotency key")?;

    if let Some(r) = maybe_row {
        info!(
            "Going to delete idempotency entry for `{}` `{}`",
            r.idempotency_key, r.label
        );

        sqlx::query!(
            r#"
        DELETE FROM idempotency
        WHERE label = $1 AND idempotency_key = $2
        "#,
            r.label,
            r.idempotency_key
        )
        .execute(&mut *tx)
        .await
        .context("failed to delete idempotency key")?;

        tx.commit().await.context("failed commiting tx")?;
        Ok(CleanResult::Deletion)
    } else {
        Ok(CleanResult::EmptyQueue)
    }
}
