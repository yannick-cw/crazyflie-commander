use datalink::compression_adapter::decompressed_grid_stream;
use datalink::domain_types::{MissionStatus, Telemetry, VehicleHealth};
use datalink::downlink::stream_telemetry_client::StreamTelemetryClient;
use datalink::{domain_types, downlink};
use futures::TryStreamExt;
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tonic::transport::{Channel, Uri};
use tracing::error;

pub async fn datalink_client(address: Uri) -> color_eyre::Result<VehicleLink> {
    let channel = Channel::builder(address).connect().await?;
    let mut payload_client = StreamTelemetryClient::new(channel.clone());
    let mut telemetry_client = StreamTelemetryClient::new(channel);
    let (latest_telemetry, _) = watch::channel(Telemetry::default());
    let local_sender = latest_telemetry.clone();
    let (latest_health, _) = watch::channel(VehicleHealth::default());
    let local_health = latest_health.clone();
    let (latest_status, _) = watch::channel(MissionStatus::default());
    let local_status = latest_status.clone();
    let (latest_grid, _) = watch::channel(vec![]);
    let local_grid = latest_grid.clone();

    tokio::spawn(async move {
        let mut stream = telemetry_client
            .stream_telemetry(())
            .await
            .expect("could not subscribe to telemetry")
            .into_inner();

        while let Some(res) = stream.next().await {
            match res {
                Ok(downlink::Message {
                    msg: Some(downlink::message::Msg::State(tele)),
                }) => {
                    local_sender.send_replace(tele.into());
                }
                Ok(downlink::Message {
                    msg: Some(downlink::message::Msg::Health(health)),
                }) => {
                    local_health.send_replace(health.into());
                }
                Ok(downlink::Message {
                    msg: Some(downlink::message::Msg::Status(status)),
                }) => {
                    local_status.send_replace(status.into());
                }
                Err(err) => {
                    error!("failed data link {:?}", err);
                }
                Ok(downlink::Message { msg: None }) => {}
            }
        }
    });

    tokio::spawn(async move {
        let mut stream = decompressed_grid_stream(
            payload_client
                .stream_payload(())
                .await
                .expect("could not subscribe to payload")
                .into_inner()
                .inspect_err(|err| error!("Error streaming grid with status {}", err))
                .filter_map(|res| res.ok()),
        );

        while let Some(res) = stream.next().await {
            local_grid.send_replace(res);
        }
    });

    Ok(VehicleLink {
        latest_telemetry,
        latest_health,
        latest_status,
        latest_grid,
    })
}

pub struct VehicleLink {
    pub latest_telemetry: watch::Sender<Telemetry>,
    pub latest_health: watch::Sender<VehicleHealth>,
    pub latest_status: watch::Sender<MissionStatus>,
    pub latest_grid: watch::Sender<domain_types::OccupancyGrid>,
}
