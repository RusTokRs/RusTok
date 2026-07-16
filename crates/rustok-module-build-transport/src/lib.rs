//! Typed gRPC adapter for the isolated `rustok-modules` build-worker port.

pub mod client;
pub mod server;

pub mod proto {
    tonic::include_proto!("rustok.module_build.v1");
}

pub use client::GrpcModuleBuildWorker;
pub use server::ModuleBuildGrpcService;
pub use tonic::transport::ClientTlsConfig;
