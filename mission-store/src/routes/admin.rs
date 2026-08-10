use crate::domain::Error::{NotFound, UnexpectedError};
use crate::domain::Res;
use anyhow::Context;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use sqlx::PgPool;
use tracing::{Instrument, info, info_span};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize)]
pub struct TokenReq {
    label: String,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize)]
pub struct TokenRes {
    label: String,
    token: String,
}

#[tracing::instrument(skip(pg_pool, tkn_req))]
pub async fn create_token(
    State(pg_pool): State<PgPool>,
    Json(tkn_req): Json<TokenReq>,
) -> Res<(StatusCode, Json<TokenRes>)> {
    let raw_token_bytes: [u8; 32] = rand::random();
    let base64_token = BASE64_STANDARD.encode(raw_token_bytes);
    let token_hash = sha2::Sha256::digest(&base64_token).0;

    // transaction to delete and insert
    let mut tx = pg_pool
        .begin()
        .await
        .context("could not start transaction")?;

    sqlx::query!(r#"DELETE FROM tokens where label = $1"#, tkn_req.label)
        .execute(&mut *tx)
        .instrument(info_span!("DELETE old token to db"))
        .await
        .context("Failed deletion")?;

    sqlx::query!(
        r#"
        INSERT INTO tokens (id, label, token_hash, created_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        tkn_req.label,
        &token_hash,
        Utc::now()
    )
    .execute(&mut *tx)
    .instrument(info_span!("INSERT token to db"))
    .await
    .context(format!(
        "Failed to insert token for label {} to db",
        tkn_req.label
    ))?;

    tx.commit().await.context("could not commit transaction")?;

    info!("New token created");
    Ok((
        StatusCode::CREATED,
        Json(TokenRes {
            label: tkn_req.label,
            token: base64_token,
        }),
    ))
}

#[tracing::instrument(skip(pg_pool))]
pub async fn revoke_token(
    State(pg_pool): State<PgPool>,
    Path(label): Path<String>,
) -> Res<StatusCode> {
    let res = sqlx::query!(
        r#"
        UPDATE tokens
        SET revoked_at = $1
        WHERE label = $2
        "#,
        Utc::now(),
        label,
    )
    .execute(&pg_pool)
    .instrument(info_span!("Revoke token"))
    .await
    .context(format!("Failed to revoke token for label {} to db", label))?;
    if res.rows_affected() == 1 {
        info!("Revoked token");
        Ok(StatusCode::OK)
    } else if res.rows_affected() == 0 {
        Err(NotFound(format!(
            "Did not find token for label `{}`",
            label
        )))
    } else {
        Err(UnexpectedError(anyhow::anyhow!(
            "Unreasonable number of rows affected - what?"
        )))
    }
}
