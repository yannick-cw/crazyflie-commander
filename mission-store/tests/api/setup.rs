use mission_store::config::get_config;
use mission_store::telemetry::trace_subscriber;
use reqwest::{Client, Response};
use secrecy::ExposeSecret;
use serde_json::Value;
use sqlx::{AssertSqlSafe, Connection, PgConnection, PgPool};
use std::error::Error;
use std::path::Path;
use std::sync::Once;
use tracing::subscriber::set_global_default;
use uuid::Uuid;

pub async fn spawn_app() -> Result<(String, Client), Box<dyn Error>> {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        set_global_default(trace_subscriber("info", false)).expect("Could not set subscriber")
    });

    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("configuration");
    let mut config = get_config(&config_path)?;
    let db_name = Uuid::new_v4().to_string();
    config.db.name = db_name.clone();
    let mut connection =
        PgConnection::connect(&config.db.connection_string_no_db().expose_secret()).await?;

    // interesting - wrapper is needed to ensure I check on sql injection
    let _ = sqlx::raw_sql(AssertSqlSafe(format!(r#"CREATE DATABASE "{}""#, db_name)))
        .execute(&mut connection)
        .await?;

    let pg_pool = PgPool::connect(&config.db.connection_string().expose_secret()).await?;
    sqlx::migrate!("../migrations").run(&pg_pool).await?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    tokio::spawn(mission_store::run(pg_pool, listener));

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
