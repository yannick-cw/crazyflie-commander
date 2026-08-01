use mission_store::config::get_config;
use mission_store::run;
use sqlx::PgPool;
use std::error::Error;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("configuration.yaml");
    let config = get_config(&config_path)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8000").await?;
    let connection = PgPool::connect(&config.db.connection_string()).await?;

    run(connection, listener).await?;
    Ok(())
}

// - [ ] webserver path: TUI stores no json missions, server does, can /post missions, /get missions and execute, /post mission results (grid + telemetry?), /post replays as missions
//       maybe serve a web page rendering a mission executed log + the grid created for that, /get grid for room id, auth for endpoints and page (bearer tkn - login form)!

// - [ ] POST /missions to store a mission (mission name + json; date?)
// - [ ] validate a to be stored mission
// - [ ] auth the POST /mission endpoint (cookie, client facing)
