use std::io::stdout;
use tracing::Subscriber;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Layer, Registry};

pub fn trace_subscriber(env_filter: &str, print_json: bool) -> impl Subscriber {
    let env_filter = EnvFilter::new(env_filter);

    Registry::default().with(env_filter).with(if !print_json {
        Layer::boxed(
            tracing_subscriber::fmt::layer().with_span_events(FmtSpan::NEW | FmtSpan::CLOSE),
        )
    } else {
        Layer::boxed(
            JsonStorageLayer.and_then(BunyanFormattingLayer::new("mision-store".into(), stdout)),
        )
    })
}
