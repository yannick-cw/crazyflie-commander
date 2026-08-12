use datalink::domain_types::Telemetry;
use datalink::wire::stream_telemetry_client::StreamTelemetryClient;
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tonic::transport::{Channel, Uri};
use tracing::error;

pub async fn datalink_client(address: Uri) -> color_eyre::Result<VehicleLink> {
    let channel = Channel::builder(address).connect().await?;
    let mut client = StreamTelemetryClient::new(channel);
    let (sender, _) = watch::channel(Telemetry::default());
    let local_sender = sender.clone();

    tokio::spawn(async move {
        let mut stream = client
            .stream_telemetry(())
            .await
            .expect("could not subscribe to telemetry")
            .into_inner()
            .filter_map(|res| match res {
                Ok(tele) => Some(Telemetry::from(tele)),
                Err(err) => {
                    error!("failed data link {:?}", err);
                    None
                }
            });
        while let Some(tele) = stream.next().await {
            local_sender.send_replace(tele);
        }
    });

    Ok(VehicleLink {
        latest_telemetry: sender,
    })
}

pub struct VehicleLink {
    pub latest_telemetry: watch::Sender<Telemetry>,
}
