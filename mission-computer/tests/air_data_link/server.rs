use crate::setup::spawn_server;
use datalink::downlink::message::Msg;
use datalink::downlink::occupancy_grid;
use std::assert_matches;
use std::error::Error;
use tokio_stream::StreamExt;

#[tokio::test]
async fn consume_telemetry() -> Result<(), Box<dyn Error>> {
    let mut client = spawn_server().await?;

    let msgs = client.stream_telemetry(()).await?.into_inner();

    let downlink_res: Result<Vec<_>, _> = msgs.take(5).collect().await;
    let mut res = downlink_res.clone()?.into_iter().map(|m| m.msg.unwrap());

    let contains_health_data = res.any(|msg| matches!(msg, Msg::Health(_)));
    let contains_state_data = res.any(|msg| matches!(msg, Msg::State(_)));
    let contains_status_data = res.any(|msg| matches!(msg, Msg::Status(_)));

    assert!(contains_health_data);
    assert!(contains_state_data);
    assert!(contains_status_data);
    assert_eq!(downlink_res?.len(), 5);
    Ok(())
}

#[tokio::test]
async fn consume_payload() -> Result<(), Box<dyn Error>> {
    let mut client = spawn_server().await?;

    let grid = client.stream_payload(()).await?.into_inner();

    let grid_res: Result<Vec<_>, _> = grid.take(1).collect().await;
    assert_matches!(&grid_res?[0].msg, Some(occupancy_grid::Msg::Keyframe(k)) if k.lists.len() == 120);
    Ok(())
}
