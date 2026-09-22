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

/// Exact revision of the independently deployed worker wire contract.
pub const SANDBOX_WORKER_PROTOCOL_REVISION: u32 = 1;

/// Covers the admitted 64 MiB artifact payload plus bounded framing overhead.
/// The shared mTLS foundation also enforces its absolute 128 MiB ceiling.
pub const SANDBOX_WORKER_MAX_MESSAGE_SIZE: usize = 72 * 1024 * 1024;
