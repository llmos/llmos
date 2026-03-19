fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var(
        "PROTOC",
        protoc_bin_vendored::protoc_bin_path().expect("vendored protoc"),
    );
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/llmos/v1/harness.proto"], &["proto"])?;
    Ok(())
}
