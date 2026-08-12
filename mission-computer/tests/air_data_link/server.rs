use crate::setup::spawn_server;
use std::error::Error;
use tokio_stream::StreamExt;

#[tokio::test]
async fn consume_telemetry() -> Result<(), Box<dyn Error>> {
    let mut client = spawn_server().await?;

    let x = client.stream_telemetry(()).await?.into_inner();

    let some_tele: Result<Vec<_>, _> = x.take(5).collect().await;

    assert_eq!(some_tele?.len(), 5);
    Ok(())
}
