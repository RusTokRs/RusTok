//! Tonic gRPC framing for the Media-owned read and write ports.
//!
//! The domain DTOs and policy remain in `rustok-media`. This crate is a
//! replaceable transport adapter and never carries media binary bodies.

pub mod client;
pub mod server;

pub mod proto {
    tonic::include_proto!("rustok.media");
}

pub use client::GrpcMediaProvider;
pub use server::{MediaGrpcOperation, MediaGrpcService, TrustedMediaAuthority};
pub use tonic::transport::ClientTlsConfig;
