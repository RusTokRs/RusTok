fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(any(feature = "grpc-client", feature = "grpc-server"))]
    tonic_prost_build::configure()
        .build_client(cfg!(feature = "grpc-client"))
        .build_server(cfg!(feature = "grpc-server"))
        .compile_with_config(
            {
                let mut config = tonic_prost_build::Config::new();
                config.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);
                config
            },
            &["proto/rustok/flex/attached_schema.proto"],
            &["proto"],
        )?;
    Ok(())
}
