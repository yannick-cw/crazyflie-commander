use crate::Autopilot;
use datalink::compression_adapter::compressed_grid_stream;
use datalink::domain_types::Cell;
use datalink::downlink::message::Msg;
use datalink::downlink::{
    Message, MissionStatus, OccupancyGrid, VehicleHealth, VehicleState, stream_telemetry_server,
};
use std::pin::Pin;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};

pub struct MissionServer<A: Autopilot> {
    pub autopilot: A, // probably will need arc at some point
}

#[tonic::async_trait]
impl<A: Autopilot + Send + Sync + 'static> stream_telemetry_server::StreamTelemetry
    for MissionServer<A>
{
    type StreamTelemetryStream = Pin<Box<dyn Stream<Item = Result<Message, Status>> + Send>>;

    async fn stream_telemetry(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::StreamTelemetryStream>, Status> {
        let tele_stream = BroadcastStream::new(self.autopilot.telemetry())
            .filter_map(Result::ok)
            .map(VehicleState::from)
            .map(|state| Message {
                msg: Some(Msg::State(state)),
            });

        let health_stream = BroadcastStream::new(self.autopilot.health())
            .filter_map(Result::ok)
            .map(VehicleHealth::from)
            .map(|state| Message {
                msg: Some(Msg::Health(state)),
            });

        let status_stream = BroadcastStream::new(self.autopilot.status())
            .filter_map(Result::ok)
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
        let grid_stream = BroadcastStream::new(self.autopilot.grid())
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
