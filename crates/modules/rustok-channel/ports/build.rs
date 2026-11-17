/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

#[cfg(any(feature = "grpc-client", feature = "grpc-server"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "proto/rustok/channel/index_identity.proto";
    println!("cargo:rerun-if-changed={path}");
    let mut prost = tonic_prost_build::Config::new();
    prost.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);
    tonic_prost_build::configure()
        .build_client(cfg!(feature = "grpc-client"))
        .build_server(cfg!(feature = "grpc-server"))
        .compile_with_config(prost, &[path], &["proto"])?;
    Ok(())
}

#[cfg(not(any(feature = "grpc-client", feature = "grpc-server")))]
fn main() {}
