//! Typed streaming transport for the separately deployed sandbox worker.

mod client;
mod server;
#[cfg(test)]
mod tests;

pub mod proto {
    tonic::include_proto!("rustok.sandbox");
}

pub use client::GrpcRhaiExecutor;
pub use server::{SandboxWorkerGrpcService, SandboxWorkerReadiness};
pub use tonic::transport::ClientTlsConfig;

/// Deployment identity evidence one worker reports about itself.
///
/// Every digest is a canonical `sha256:<hex>` string computed or validated
/// by the worker process from its own deployment facts (its binary, its
/// admitted image digest, its isolation envelope). Hosts use this as the
/// isolated-pool input to the governed runtime fingerprint and must fail
/// closed when the worker cannot supply it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SandboxWorkerIdentity {
    pub executor_binary_digest: String,
    pub engine_build_digest: String,
    pub worker_image_digest: String,
    pub engine_config_digest: String,
    pub target_cpu_contract: String,
    pub runtime_abi: String,
    pub isolation_runtime: String,
}

/// Exact revision of the independently deployed worker wire contract.
pub const SANDBOX_WORKER_PROTOCOL_REVISION: u32 = 1;

/// Covers the admitted 64 MiB artifact payload plus bounded framing overhead.
/// The shared mTLS foundation also enforces its absolute 128 MiB ceiling.
pub const SANDBOX_WORKER_MAX_MESSAGE_SIZE: usize = 72 * 1024 * 1024;
