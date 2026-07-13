//! Isolated execution boundary for artifact trust verification.

pub mod cosign;
pub mod policy;
pub mod service;

pub use policy::VerificationPolicy;
pub use rustok_verification_transport::VerificationGrpcService;
pub use cosign::CosignTrustVerifier;
pub use service::{VerificationWorker, VerificationWorkerError};
