//! Independent node-local materialization and readiness agent for dynamic
//! RusToK module artifacts.
//!
//! This crate receives only owner-issued assignments through the mTLS node
//! controller. It reads durable artifact CAS directly, uses the canonical
//! instance layout for local cache files, and checks the exact sandbox runtime
//! required by the assigned payload. It has no database, topology, policy,
//! product, AI, Alloy, or application-server dependency.

mod agent;
mod config;
mod materializer;

pub use agent::{
    ArtifactNodeAgent, ArtifactNodeAgentError, ArtifactNodeAssignmentController,
    ArtifactNodeMaterializationError, ArtifactNodeMaterializer, NodeArtifactPreparation,
};
pub use config::ArtifactNodeAgentConfig;
pub use materializer::StorageArtifactNodeMaterializer;
