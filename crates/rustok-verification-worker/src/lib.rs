//! Isolated execution boundary for artifact trust verification.

pub mod cosign;
pub mod policy;
pub mod service;

pub use cosign::CosignTrustVerifier;
pub use policy::{VerificationPolicy, VerificationTrustRoot};
pub use rustok_verification_transport::VerificationGrpcService;
pub use rustok_worker_transport::MutualTlsListenerConfig;
pub use service::{VerificationWorker, VerificationWorkerError};
