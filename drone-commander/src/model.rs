use drone_control::Telemetry;

#[derive(Debug, Default, Copy, Clone)]
pub struct Model {
    pub telemetry: Telemetry,
    pub exit: bool,
    pub counter: i64,
}
