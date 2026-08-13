use crate::Autopilot;
use datalink::downlink::message::Msg;
use datalink::downlink::{Message, VehicleHealth, VehicleState, stream_telemetry_server};
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

        let all_update = tele_stream.merge(health_stream);
        Ok(Response::new(Box::pin(all_update.map(Ok))))
    }
}
