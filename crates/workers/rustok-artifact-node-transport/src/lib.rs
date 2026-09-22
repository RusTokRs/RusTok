//! Typed mTLS transport for durable artifact-node agent and topology operations.

mod client;
mod reconciliation_server;
mod server;
mod status;

pub mod proto {
    tonic::include_proto!("rustok.artifact_node");
}

pub use client::GrpcArtifactNodeAgent;
pub use reconciliation_server::{
    ArtifactNodeReconciliationGrpcService, ModuleArtifactNodeReconciliationAuthenticator,
    ModuleArtifactNodeReconciliationIdentity, StaticModuleArtifactNodeReconciliationAuthenticator,
};
pub use server::{
    ArtifactNodeGrpcService, ModuleArtifactNodeAgentAuthenticator, ModuleArtifactNodeAgentIdentity,
    StaticModuleArtifactNodeAgentAuthenticator,
};
pub(crate) use status::owner_status;
pub use tonic::transport::ClientTlsConfig;

/// Exact external framing revision for the separately deployed node agent.
pub const ARTIFACT_NODE_AGENT_PROTOCOL_REVISION: u32 = 1;

/// Node reconciliation bodies contain an immutable assignment and bounded
/// evidence references, never a CAS payload.
pub const ARTIFACT_NODE_AGENT_MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// The reconciliation service accepts exactly one bounded topology submission
/// from an authenticated deployment operator. It has the same ceiling as the
/// agent service but a distinct mTLS principal class and generated RPC service.
pub const ARTIFACT_NODE_RECONCILIATION_MAX_MESSAGE_SIZE: usize = 1024 * 1024;
