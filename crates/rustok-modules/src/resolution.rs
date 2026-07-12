//! Deterministic provider boundary for exact module dependency resolution.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{ModuleDependencyConstraint, ModuleDependencyLockGraph, ModuleDependencyLockNode};

/// Candidate released by the owner catalog and eligible for solver evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleResolutionCandidate {
    pub node: ModuleDependencyLockNode,
    pub runtime_abi: String,
    pub trusted: bool,
    pub active: bool,
    pub yanked: bool,
    pub revoked: bool,
}

/// Owner provider. Its implementation may use `pubgrub`, but its dependency
/// solving types and derivation internals never cross this boundary.
#[async_trait]
pub trait ModuleResolutionProvider: Send + Sync {
    async fn candidates(
        &self,
        dependency: &ModuleDependencyConstraint,
    ) -> Result<Vec<ModuleResolutionCandidate>, ModuleResolutionError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleResolutionConflict {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub involved_slugs: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModuleResolutionError {
    #[error("module resolution provider failed: {0}")]
    Provider(String),
    #[error("module dependency resolution conflict: {0}")]
    Conflict(String),
}

/// Immutable result returned by the solver adapter after selecting every
/// direct and transitive dependency for one scope/revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleResolutionResult {
    pub lock_graph: ModuleDependencyLockGraph,
    #[serde(default)]
    pub conflicts: Vec<ModuleResolutionConflict>,
}
