fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    // Keep generated code reproducible without relying on a host-installed
    // protobuf compiler.
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    tonic_prost_build::configure().compile_protos(
        &[
            "proto/rustok/module_build/module_build.proto",
            "proto/rustok/static_distribution/static_distribution.proto",
        ],
        &["proto"],
    )?;
    Ok(())
}
