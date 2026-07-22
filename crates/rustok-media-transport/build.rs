fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    // Build generated code reproducibly without requiring a host Protobuf install.
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    tonic_prost_build::configure()
        .compile_protos(&["proto/rustok/media/media.proto"], &["proto"])?;
    Ok(())
}
