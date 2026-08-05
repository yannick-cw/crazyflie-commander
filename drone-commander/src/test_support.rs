use std::sync::Once;

static INIT: Once = Once::new();
pub fn init_tracing() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_test_writer()
            .try_init()
            .expect("Logs?!");
    });
}
