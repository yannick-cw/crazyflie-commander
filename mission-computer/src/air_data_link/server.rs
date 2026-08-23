use crate::control::autopilot::VehicleDownlink;
use crate::control::vehicle_control::VehicleHandle;
use datalink::compression_adapter::compressed_grid_stream;
use datalink::domain_types;
use datalink::domain_types::Cell;
use datalink::downlink::message::Msg;
use datalink::downlink::stream_telemetry_server::StreamTelemetry;
use datalink::downlink::{Message, MissionStatus, OccupancyGrid, VehicleHealth, VehicleState};
use datalink::uplink::uplink_service_server::UplinkService;
use datalink::uplink::{AbortMission, Mission};
use std::pin::Pin;
use tokio_stream::wrappers::{BroadcastStream, WatchStream};
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};
use tracing::error;

pub struct MissionServer {
    pub vehicle_downlink: VehicleDownlink,
}

pub struct UplinkServer {
    pub vehicle_handle: VehicleHandle,
}

#[tonic::async_trait]
impl StreamTelemetry for MissionServer {
    type StreamTelemetryStream = Pin<Box<dyn Stream<Item = Result<Message, Status>> + Send>>;

    async fn stream_telemetry(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::StreamTelemetryStream>, Status> {
        let tele_stream = BroadcastStream::new(self.vehicle_downlink.subscribe_telemetry())
            .filter_map(Result::ok)
            .map(VehicleState::from)
            .map(|state| Message {
                msg: Some(Msg::State(state)),
            });

        let health_stream = BroadcastStream::new(self.vehicle_downlink.subscribe_health())
            .filter_map(Result::ok)
            .map(VehicleHealth::from)
            .map(|state| Message {
                msg: Some(Msg::Health(state)),
            });

        let status_stream = WatchStream::new(self.vehicle_downlink.subscribe_status())
            .map(MissionStatus::from)
            .map(|status| Message {
                msg: Some(Msg::Status(status)),
            });

        let all_update = tele_stream.merge(health_stream).merge(status_stream);
        Ok(Response::new(Box::pin(all_update.map(Ok))))
    }

    type StreamPayloadStream = Pin<Box<dyn Stream<Item = Result<OccupancyGrid, Status>> + Send>>;

    async fn stream_payload(
        &self,
        _: Request<()>,
    ) -> Result<Response<Self::StreamPayloadStream>, Status> {
        let grid_stream = BroadcastStream::new(self.vehicle_downlink.subscribe_grid())
            .filter_map(Result::ok)
            .map(|g| {
                let a: Vec<Vec<Cell>> = g
                    .inner()
                    .into_iter()
                    .map(|inner| inner.into_iter().map(|c| Cell::from(c)).collect())
                    .collect::<Vec<_>>();
                a
            });

        Ok(Response::new(Box::pin(
            compressed_grid_stream(grid_stream).map(Ok),
        )))
    }
}
#[tonic::async_trait]
impl UplinkService for UplinkServer {
    async fn execute_mission(&self, request: Request<Mission>) -> Result<Response<()>, Status> {
        let mission: Vec<_> = request
            .into_inner()
            .mission_item
            .into_iter()
            .map(domain_types::MissionItem::from)
            .collect();

        let vehicle_handle = self.vehicle_handle.clone();

        vehicle_handle
            .submit_mission(mission)
            .await
            .inspect_err(|err| error!("Failed mission execution {:?}", err))
            .map_err(|err| Status::failed_precondition(format!("{:?}", err)))
            .map(Response::new)
    }

    async fn abort_mission(&self, request: Request<AbortMission>) -> Result<Response<()>, Status> {
        let abort_signal = request.into_inner().into();
        let vehicle_handle = self.vehicle_handle.clone();

        vehicle_handle
            .abort_mission(abort_signal)
            .await
            .inspect_err(|err| error!("Failed mission abortion {:?}", err))
            .map_err(|err| Status::failed_precondition(format!("{:?}", err)))
            .map(Response::new)
    }
}
