//! Isolated execution boundary for artifact trust verification.

pub mod cosign;
pub mod listener;
pub mod policy;
pub mod service;

pub use cosign::CosignTrustVerifier;
pub use listener::ListenerConfig;
pub use policy::{VerificationPolicy, VerificationTrustRoot};
pub use rustok_verification_transport::VerificationGrpcService;
pub use service::{VerificationWorker, VerificationWorkerError};
