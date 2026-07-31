use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router, routing::get};
use drone_control::Command;

pub async fn run(listener: tokio::net::TcpListener) -> Result<(), std::io::Error> {
    let app = Router::new()
        .route("/health_check", get(health_check))
        .route("/missions/{mission_name}", post(post_mission));

    axum::serve(listener, app).await
}

async fn health_check() -> StatusCode {
    StatusCode::OK
}

async fn post_mission(
    Path(mission_name): Path<String>,
    Json(mission): Json<Vec<Command>>,
) -> StatusCode {
    println!("{:?}", mission);
    println!("{mission_name}");
    StatusCode::CREATED
}
