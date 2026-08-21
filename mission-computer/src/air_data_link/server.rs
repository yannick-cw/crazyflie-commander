use crate::Autopilot;
use datalink::compression_adapter::compressed_grid_stream;
use datalink::domain_types;
use datalink::domain_types::Cell;
use datalink::downlink::message::Msg;
use datalink::downlink::stream_telemetry_server::StreamTelemetry;
use datalink::downlink::{Message, MissionStatus, OccupancyGrid, VehicleHealth, VehicleState};
use datalink::uplink::Mission;
use datalink::uplink::uplink_service_server::UplinkService;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};
use tracing::error;

pub struct MissionServer<A: Autopilot> {
    pub autopilot: Arc<A>,
}
impl<A: Autopilot> Clone for MissionServer<A> {
    fn clone(&self) -> Self {
        Self {
            autopilot: self.autopilot.clone(),
        }
    }
}

#[tonic::async_trait]
impl<A: Autopilot + Send + Sync + 'static> StreamTelemetry for MissionServer<A> {
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
#[tonic::async_trait]
impl<A: Autopilot + Send + Sync + 'static> UplinkService for MissionServer<A> {
    async fn execute_mission(&self, request: Request<Mission>) -> Result<Response<()>, Status> {
        let mission: Vec<_> = request
            .into_inner()
            .mission_item
            .into_iter()
            .map(domain_types::MissionItem::from)
            .collect();

        let pilot = self.autopilot.clone();

        // todo should probably not run from here directly - e.g. double calling clashes - maybe some
        // actors with messages or state machine internally
        // -------
        // move to handler model, this server here just gets a handler, the handler itself runs on a
        // task and has idle, mission_exe, free_flight or whatever state, and the handler req communicate via
        // channels with it from here - oneshot reply if accepted or rejected..
        tokio::spawn(async move {
            let _ = pilot
                .run_mission(mission, async {
                    sleep(Duration::from_hours(10)).await;
                    None
                })
                .await
                .inspect_err(|err| error!("Failed mission execution {:?}", err));
        });
        Ok(Response::new(()))
    }
}
