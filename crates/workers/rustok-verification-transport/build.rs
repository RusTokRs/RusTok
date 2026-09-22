fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    // Keep generated transport code reproducible on developer machines and in
    // CI without relying on a host-installed protobuf compiler.
    // Cargo runs build scripts before concurrent code generation begins, so
    // this process-local environment update cannot race application code.
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    tonic_prost_build::configure().compile_protos(
        &["proto/rustok/verification/v1/verification.proto"],
        &["proto"],
    )?;
    Ok(())
}
