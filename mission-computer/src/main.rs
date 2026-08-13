use datalink::downlink::stream_telemetry_server::StreamTelemetryServer;
use mission_computer::dev_pilot::DevPilot;
use mission_computer::server::MissionServer;
use mission_computer::setup_link;
use std::net::SocketAddr;
use tonic::transport::Server;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();

    info!("Starting up....");
    let server = match setup_link().await {
        Ok(real_unit) => Server::builder().add_service(StreamTelemetryServer::new(MissionServer {
            autopilot: real_unit,
        })),
        _ => {
            // fallback for dev
            Server::builder().add_service(StreamTelemetryServer::new(MissionServer {
                autopilot: DevPilot,
            }))
        }
    };
    let address = "127.0.0.1:50051".parse::<SocketAddr>()?;
    server.serve(address).await?;
    Ok(())
}

// TODO
// - [ ] instrument?
// - [ ] start from devenv
// - [ ] reset/start fn or sth like this
// - [ ] test server
