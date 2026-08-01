pub mod config;
mod routes;

use crate::routes::health_check::health_check;
use crate::routes::missions::{get_mission, post_mission};
use axum::routing::post;
use axum::{Router, routing::get};
use sqlx::PgPool;

pub async fn run(pg_pool: PgPool, listener: tokio::net::TcpListener) -> Result<(), std::io::Error> {
    let app = Router::new()
        .route("/health_check", get(health_check))
        .route(
            "/missions/{mission_name}",
            post(post_mission).get(get_mission),
        )
        .with_state(pg_pool);

    axum::serve(listener, app).await
}
