use crate::Autopilot;
use datalink::wire;
use datalink::wire::WireTelemetry;
use std::pin::Pin;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};

pub struct MissionServer<A: Autopilot> {
    pub autopilot: A, // probably will need arc at some point
}

#[tonic::async_trait]
impl<A: Autopilot + Send + Sync + 'static> wire::stream_telemetry_server::StreamTelemetry
    for MissionServer<A>
{
    type StreamTelemetryStream = Pin<Box<dyn Stream<Item = Result<WireTelemetry, Status>> + Send>>;

    async fn stream_telemetry(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::StreamTelemetryStream>, Status> {
        Ok(Response::new(Box::pin(
            BroadcastStream::new(self.autopilot.telemetry())
                .filter_map(Result::ok)
                .map(WireTelemetry::from)
                .map(Ok),
        )))
    }
}
