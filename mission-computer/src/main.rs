use datalink::domain_types::VehicleStatus;
use datalink::downlink::stream_telemetry_server::StreamTelemetryServer;
use datalink::uplink::uplink_service_server::UplinkServiceServer;
use mission_computer::dev_pilot::{DevPilot, dev_downlink};
use mission_computer::server::{MissionServer, UplinkServer};
use mission_computer::{init_vehicle_control, setup_link};
use std::net::SocketAddr;
use tokio::sync::watch;
use tonic::transport::Server;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();

    info!("Starting up....");
    let (server, _handle) = match setup_link().await {
        Ok((vehicle_downlink, real_unit)) => {
            let status_publish = vehicle_downlink.status.clone();
            let (control, vehicle_handle) = init_vehicle_control(real_unit, status_publish);
            (
                Server::builder()
                    .add_service(StreamTelemetryServer::new(MissionServer {
                        vehicle_downlink,
                    }))
                    .add_service(UplinkServiceServer::new(UplinkServer { vehicle_handle })),
                tokio::spawn(control.run()),
            )
        }
        _ => {
            // fallback for dev
            let dummy_sender = watch::channel(VehicleStatus::Idle).0;
            let (control, vehicle_handle) = init_vehicle_control(DevPilot, dummy_sender);
            (
                Server::builder()
                    .add_service(StreamTelemetryServer::new(MissionServer {
                        vehicle_downlink: dev_downlink(),
                    }))
                    .add_service(UplinkServiceServer::new(UplinkServer { vehicle_handle })),
                tokio::spawn(control.run()),
            )
        }
    };
    let address = "127.0.0.1:50051".parse::<SocketAddr>()?;
    server.serve(address).await?;
    Ok(())
}
