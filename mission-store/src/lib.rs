pub mod config;
pub mod domain;
mod routes;
pub mod telemetry;

use crate::domain::Error::UnexpectedError;
use crate::routes::admin::{create_token, revoke_token};
use crate::routes::flights::{get_flight, post_flight};
use crate::routes::health_check::health_check;
use crate::routes::missions::{get_mission, list_missions, post_mission};
use anyhow::Context;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Router, extract, routing::get};
use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use sha2::Digest;
use sqlx::PgPool;
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, RequestId, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing::{Instrument, info_span};

pub async fn run(pg_pool: PgPool, listener: tokio::net::TcpListener) -> Result<(), std::io::Error> {
    let service = ServiceBuilder::new()
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(TraceLayer::new_for_http().make_span_with(|r: &Request<_>| {
            info_span!(
                "Mission Request",
                req_id = %r
                    .extensions()
                    .get::<RequestId>()
                    .and_then(|x| x.header_value().to_str().ok())
                    .unwrap_or("no_id")
            )
        }));

    let app = Router::new()
        .route("/health_check", get(health_check))
        .route(
            "/missions/{mission_name}",
            post(post_mission).get(get_mission),
        )
        .route("/missions", get(list_missions))
        .route("/flights/{flight_name}", post(post_flight).get(get_flight))
        // everything above is authed
        .layer(from_fn_with_state(pg_pool.clone(), auth_middleware))
        // not authed
        .route("/admin/tokens/{label}/revoke", post(revoke_token))
        .route("/admin/tokens", post(create_token))
        .with_state(pg_pool)
        .layer(service);

    axum::serve(listener, app).await
}

async fn auth_middleware(
    TypedHeader(token): TypedHeader<Authorization<Bearer>>,
    State(pg_pool): State<PgPool>,
    request: extract::Request,
    next: Next,
) -> Response {
    let token_hash = sha2::Sha256::digest(token.0.token()).0;
    let res = sqlx::query!(
        r#"
        SELECT * from tokens
        where token_hash = $1 and revoked_at is null
        "#,
        &token_hash,
    )
    .fetch_optional(&pg_pool)
    .instrument(info_span!("Checking token.."))
    .await
    .context("Failed checking token");

    match res {
        Ok(Some(_)) => next.run(request).await,
        Ok(None) => StatusCode::UNAUTHORIZED.into_response(),
        Err(e) => UnexpectedError(e).into_response(),
    }
}
