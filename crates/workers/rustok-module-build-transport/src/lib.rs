//! Typed gRPC adapter for the isolated `rustok-modules` build-worker port.

pub mod client;
pub mod distribution_client;
pub mod distribution_server;
pub mod server;

pub mod module_build_proto {
    tonic::include_proto!("rustok.module_build");
}

pub mod static_distribution_proto {
    tonic::include_proto!("rustok.static_distribution");
}

pub use client::GrpcModuleBuildWorker;
pub use distribution_client::GrpcStaticDistributionExecutor;
pub use distribution_server::StaticDistributionGrpcService;
pub use server::ModuleBuildGrpcService;
pub use tonic::transport::ClientTlsConfig;
