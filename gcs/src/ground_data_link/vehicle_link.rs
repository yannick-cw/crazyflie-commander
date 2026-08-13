use datalink::domain_types::{Telemetry, VehicleHealth};
use datalink::downlink;
use datalink::downlink::stream_telemetry_client::StreamTelemetryClient;
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tonic::transport::{Channel, Uri};
use tracing::error;

pub async fn datalink_client(address: Uri) -> color_eyre::Result<VehicleLink> {
    let channel = Channel::builder(address).connect().await?;
    let mut client = StreamTelemetryClient::new(channel);
    let (latest_telemetry, _) = watch::channel(Telemetry::default());
    let local_sender = latest_telemetry.clone();
    let (latest_health, _) = watch::channel(VehicleHealth::default());
    let local_health = latest_health.clone();

    tokio::spawn(async move {
        let mut stream = client
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
                Ok(_) => {}
                Err(err) => {
                    error!("failed data link {:?}", err);
                }
            }
        }
    });

    Ok(VehicleLink {
        latest_telemetry,
        latest_health,
    })
}

pub struct VehicleLink {
    pub latest_telemetry: watch::Sender<Telemetry>,
    pub latest_health: watch::Sender<VehicleHealth>,
}
