//! Isolated execution boundary for artifact trust verification.

pub mod policy;
pub mod service;

pub use policy::VerificationPolicy;
pub use service::{VerificationWorker, VerificationWorkerError};
