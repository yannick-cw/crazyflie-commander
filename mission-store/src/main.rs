use mission_store::config::get_config;
use mission_store::telemetry::trace_subscriber;
use mission_store::{run_cleanup_loop, run_server};
use secrecy::ExposeSecret;
use sqlx::PgPool;
use std::error::Error;
use std::path::Path;
use tokio::select;
use tracing::subscriber::set_global_default;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("configuration");
    let config = get_config(&config_path)?;

    set_global_default(trace_subscriber(
        &config.log_settings.log_filter,
        config.log_settings.log_structured,
    ))
    .expect("Could not set subscriber");

    info!("starting service...");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8000").await?;
    let connection = PgPool::connect(&config.db.connection_string().expose_secret()).await?;
    let worker_pool = PgPool::connect(&config.db.connection_string().expose_secret()).await?;

    let cleanup_handle = tokio::spawn(run_cleanup_loop(worker_pool));
    let server_handle = tokio::spawn(run_server(connection, listener));

    let report_outcome = |o, name| match o {
        Ok(Err(err)) => error!("Failed running task: {name} {err:?}"),
        Err(err) => error!("Failed task: {name} {err:?}"),
        Ok(_) => info!("Exited: {name}"),
    };
    select! {
        res = cleanup_handle => report_outcome(res, "cleanup"),
        res = server_handle => report_outcome(res, "server"),
    };
    Ok(())
}

// - [x] background worker tasks to clean up old idempotency keys
// - [x] webserver path: TUI stores no json missions, server does, can /post missions, /get missions and execute, /post mission results (grid + telemetry?), /post replays as missions
//       maybe serve a web page rendering a mission executed log + the grid created for that, /get grid for room id, auth for endpoints and page (bearer tkn - login form)!
// - [x] POST /missions to store a mission (mission name + json; date?)
// - [x] POST /flight to store a flight with (telemetry) with an optional mission id
// - [ ] validate a to be stored mission
// - [x] auth the POST /mission endpoint (cookie, client facing)
