use crate::setup::spawn_server;
use datalink::downlink::Message;
use datalink::downlink::message::Msg;
use std::error::Error;
use tokio_stream::StreamExt;

#[tokio::test]
async fn consume_telemetry() -> Result<(), Box<dyn Error>> {
    let mut client = spawn_server().await?;

    let x = client.stream_telemetry(()).await?.into_inner();

    let downlink_res: Result<Vec<_>, _> = x.take(5).collect().await;
    let res = downlink_res?;

    let contains_health_data = res.iter().any(|msg| {
        matches!(
            msg,
            Message {
                msg: Some(Msg::Health(_))
            }
        )
    });

    let contains_state_data = res.iter().any(|msg| {
        matches!(
            msg,
            Message {
                msg: Some(Msg::State(_))
            }
        )
    });

    let contains_status_data = res.iter().any(|msg| {
        matches!(
            msg,
            Message {
                msg: Some(Msg::Status(_))
            }
        )
    });

    assert!(contains_health_data);
    assert!(contains_state_data);
    assert!(contains_status_data);
    assert_eq!(res.len(), 5);
    Ok(())
}
