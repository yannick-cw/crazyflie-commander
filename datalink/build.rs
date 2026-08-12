fn main() {
    let file_descriptors =
        protox::compile(["telemetry.proto", "service.proto"], ["proto/"]).unwrap();

    tonic_prost_build::configure()
        .compile_fds(file_descriptors)
        .unwrap();
}
