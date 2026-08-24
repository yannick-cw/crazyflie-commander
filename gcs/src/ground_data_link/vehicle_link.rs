use datalink::compression_adapter::decompressed_grid_stream;
use datalink::domain_types::{
    Abort, MissionItem, Telemetry, TrajectoryId, VehicleHealth, VehicleStatus,
};
use datalink::downlink::stream_telemetry_client::StreamTelemetryClient;
use datalink::uplink::uplink_service_client::UplinkServiceClient;
use datalink::{domain_types, downlink, uplink};
use futures::TryStreamExt;
use mission_computer::errors::MissionError::UploadError;
use mission_computer::errors::Res;
use std::time::Duration;
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tonic::transport::{Channel, Uri};
use tracing::error;

pub async fn datalink_client(address: Uri) -> color_eyre::Result<VehicleLink> {
    let channel = Channel::builder(address).connect().await?;
    let mut payload_client = StreamTelemetryClient::new(channel.clone());
    let mut telemetry_client = StreamTelemetryClient::new(channel.clone());
    let uplink_client = UplinkServiceClient::new(channel);
    let (latest_telemetry, _) = watch::channel(Telemetry::default());
    let local_sender = latest_telemetry.clone();
    let (latest_health, _) = watch::channel(VehicleHealth::default());
    let local_health = latest_health.clone();
    let (latest_status, _) = watch::channel(VehicleStatus::default());
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
        uplink_client,
    })
}

pub struct VehicleLink {
    pub latest_telemetry: watch::Sender<Telemetry>,
    pub latest_health: watch::Sender<VehicleHealth>,
    pub latest_status: watch::Sender<VehicleStatus>,
    pub latest_grid: watch::Sender<domain_types::OccupancyGrid>,
    uplink_client: UplinkServiceClient<Channel>,
}

impl VehicleLink {
    pub async fn submit_mission(&self, mission: Vec<MissionItem>) -> Res<()> {
        let mut client = self.uplink_client.clone();
        let wire_mission = uplink::Mission {
            mission_item: mission
                .into_iter()
                .map(uplink::MissionItem::from)
                .collect::<Vec<_>>(),
        };
        client
            .execute_mission(wire_mission)
            .await
            .map_err(|err| UploadError(format!("Could not start mission {:?}", err)))?;
        Ok(())
    }

    pub async fn abort_mission(&self, signal: Abort) -> Res<()> {
        let mut client = self.uplink_client.clone();

        client
            .abort_mission(uplink::AbortMission::from(signal))
            .await
            .map_err(|err| UploadError(format!("Could not abort mission {:?}", err)))?;
        Ok(())
    }

    pub async fn upload_mission(
        &self,
        mission_item: MissionItem,
    ) -> Res<Option<(TrajectoryId, Duration)>> {
        let mut client = self.uplink_client.clone();

        let res = client
            .upload_trajectory(uplink::MissionItem::from(mission_item))
            .await
            .map_err(|err| UploadError(format!("Could not upload mission item {:?}", err)))?;

        Ok(res.into_inner().res.and_then(|t_res| {
            t_res.duration.map(|d| {
                (
                    TrajectoryId(t_res.trajectory_id as u8),
                    d.try_into().unwrap(),
                )
            })
        }))
    }
}
