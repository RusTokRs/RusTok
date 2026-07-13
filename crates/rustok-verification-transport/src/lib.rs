//! Typed gRPC adapter for the `rustok-modules` trust-verification port.

pub mod client;
pub mod server;

pub mod proto {
    tonic::include_proto!("rustok.verification.v1");
}

pub use client::GrpcTrustVerifier;
pub use server::VerificationGrpcService;
