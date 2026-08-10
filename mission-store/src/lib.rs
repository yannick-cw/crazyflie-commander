pub mod config;
pub mod domain;
pub mod idempotency;
mod routes;
pub mod telemetry;

use crate::domain::Error::Unauthorized;
use crate::domain::{Label, Res};
use crate::idempotency::{CleanResult, clean_outdated};
use crate::routes::admin::{create_token, revoke_token};
use crate::routes::flights::{get_flight, post_flight};
use crate::routes::health_check::health_check;
use crate::routes::missions::{get_mission, list_missions, post_mission};
use anyhow::Context;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Router, extract, routing::get};
use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use chrono::{Days, Utc};
use sha2::Digest;
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::sleep;
use tower::ServiceBuilder;
use tower_http::classify::ServerErrorsFailureClass;
use tower_http::request_id::{MakeRequestUuid, RequestId, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing::{Instrument, Span, error, info_span, warn};

pub async fn run_server(pg_pool: PgPool, listener: tokio::net::TcpListener) -> Res<()> {
    let service = ServiceBuilder::new()
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|r: &Request<_>| {
                    info_span!(
                        "Mission Request",
                        req_id = %r
                            .extensions()
                            .get::<RequestId>()
                            .and_then(|x| x.header_value().to_str().ok())
                            .unwrap_or("no_id"),
                        status_code = tracing::field::Empty,
                    )
                })
                .on_response(|response: &Response<_>, _: Duration, span: &Span| {
                    span.record("status_code", response.status().as_str());
                })
                .on_failure(
                    |failure_classification: ServerErrorsFailureClass,
                     _latency: Duration,
                     _: &Span| {
                        error!("Failed with {}", failure_classification)
                    },
                ),
        );

    let app = Router::new()
        .route(
            "/missions/{mission_name}",
            post(post_mission).get(get_mission),
        )
        .route("/missions", get(list_missions))
        .route("/flights/{flight_name}", post(post_flight).get(get_flight))
        // everything above is authed
        .layer(from_fn_with_state(pg_pool.clone(), auth_middleware))
        // not authed
        .route("/health_check", get(health_check))
        .route("/admin/tokens/{label}/revoke", post(revoke_token))
        .route("/admin/tokens", post(create_token))
        .with_state(pg_pool)
        .layer(service);

    axum::serve(listener, app)
        .await
        .context("could not run server")?;
    Ok(())
}

#[tracing::instrument(skip(pg_pool, maybe_tkn, request, next))]
async fn auth_middleware(
    maybe_tkn: Option<TypedHeader<Authorization<Bearer>>>,
    State(pg_pool): State<PgPool>,
    mut request: extract::Request,
    next: Next,
) -> Response {
    match auth(maybe_tkn, &pg_pool).await {
        Ok(label) => {
            request.extensions_mut().insert(label);
            next.run(request).await
        }
        Err(err) => {
            warn!("{err:?}");
            err.into_response()
        }
    }
}

async fn auth(
    maybe_tkn: Option<TypedHeader<Authorization<Bearer>>>,
    pg_pool: &PgPool,
) -> Res<Label> {
    let token = maybe_tkn.ok_or(Unauthorized)?;
    let token_hash = sha2::Sha256::digest(token.0.token()).0;
    let response = sqlx::query!(
        r#"
            SELECT label from tokens
            where token_hash = $1 and revoked_at is null
            "#,
        &token_hash,
    )
    .fetch_optional(pg_pool)
    .instrument(info_span!("Checking token.."))
    .await
    .transpose()
    .ok_or(Unauthorized)?
    .context("Failed checking token")?;

    Ok(Label(response.label))
}

pub async fn run_cleanup_loop(pg_pool: PgPool) -> Res<()> {
    loop {
        let cutoff_time = Utc::now().checked_sub_days(Days::new(2)).unwrap();
        match clean_outdated(cutoff_time, &pg_pool).await {
            Ok(CleanResult::Deletion) => {} // keep going
            Ok(CleanResult::EmptyQueue) => sleep(Duration::from_secs(10)).await,
            Err(err) => {
                error!("Failed cleanup {:?}", err);
                sleep(Duration::from_mins(1)).await;
            }
        }
    }
}
