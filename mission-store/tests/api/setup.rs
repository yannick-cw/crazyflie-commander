use mission_store::config::get_config;
use reqwest::{Client, Response};
use serde_json::Value;
use sqlx::PgPool;
use std::error::Error;
use std::path::Path;

pub async fn spawn_app() -> Result<(String, Client), Box<dyn Error>> {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("configuration.yaml");
    let config = get_config(&config_path)?;
    let connection = PgPool::connect(&config.db.connection_string).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    tokio::spawn(mission_store::run(connection, listener));

    let client = Client::new();

    Ok((format!("http://127.0.0.1:{}", port), client))
}

pub async fn post(
    url: String,
    client: &Client,
    json_mission: &Value,
) -> Result<Response, reqwest::Error> {
    client.post(url).json(&json_mission).send().await
}

pub async fn get(url: String, client: &Client) -> Result<Response, Box<dyn Error>> {
    let r = client.get(url).send().await?;
    Ok(r)
}
