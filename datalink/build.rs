fn main() {
    let file_descriptors = protox::compile(
        [
            "telemetry.proto",
            "uplink_service.proto",
            "downlink_service.proto",
            "mission_item.proto",
        ],
        ["proto/"],
    )
    .unwrap();

    tonic_prost_build::configure()
        .compile_fds(file_descriptors)
        .unwrap();
}
