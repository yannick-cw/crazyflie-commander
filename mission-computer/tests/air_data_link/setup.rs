use datalink::wire::stream_telemetry_client::StreamTelemetryClient;
use datalink::wire::stream_telemetry_server::StreamTelemetryServer;
use mission_computer::dev_pilot::DevPilot;
use mission_computer::server::MissionServer;
use std::error::Error;
use std::sync::Once;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Channel, Server};

pub async fn spawn_server() -> Result<StreamTelemetryClient<Channel>, Box<dyn Error>> {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        tracing_subscriber::fmt::init();
    });

    let server = Server::builder().add_service(StreamTelemetryServer::new(MissionServer {
        autopilot: DevPilot,
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    tokio::spawn(server.serve_with_incoming(TcpListenerStream::new(listener)));

    let endpoint = format!("http://127.0.0.1:{}", port);
    let channel = Channel::from_shared(endpoint)?.connect().await?;
    let client = StreamTelemetryClient::new(channel);
    Ok(client)
}
