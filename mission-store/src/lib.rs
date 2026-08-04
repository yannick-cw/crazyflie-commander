pub mod config;
pub mod domain;
mod routes;
pub mod telemetry;

use crate::routes::flights::{get_flight, post_flight};
use crate::routes::health_check::health_check;
use crate::routes::missions::{get_mission, post_mission};
use axum::http::Request;
use axum::routing::post;
use axum::{Router, routing::get};
use sqlx::PgPool;
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, RequestId, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing::info_span;

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
        .route("/flights/{flight_name}", post(post_flight).get(get_flight))
        .with_state(pg_pool)
        .layer(service);

    axum::serve(listener, app).await
}
