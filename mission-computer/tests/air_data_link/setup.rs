use datalink::downlink::stream_telemetry_client::StreamTelemetryClient;
use datalink::downlink::stream_telemetry_server::StreamTelemetryServer;
use mission_computer::dev_pilot::dev_downlink;
use mission_computer::server::DownlinkServer;
use std::error::Error;
use std::sync::Once;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Channel, Server};

pub async fn spawn_server() -> Result<StreamTelemetryClient<Channel>, Box<dyn Error>> {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        tracing_subscriber::fmt::init();
    });

    let downlink_server =
        Server::builder().add_service(StreamTelemetryServer::new(DownlinkServer {
            vehicle_downlink: dev_downlink(),
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    tokio::spawn(downlink_server.serve_with_incoming(TcpListenerStream::new(listener)));

    let endpoint = format!("http://127.0.0.1:{}", port);
    let channel = Channel::from_shared(endpoint)?.connect().await?;
    let client = StreamTelemetryClient::new(channel);
    Ok(client)
}
