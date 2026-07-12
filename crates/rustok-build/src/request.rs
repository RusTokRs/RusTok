//! Typed input and output contracts for build and release operations.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{BuildExecutionPlan, DeploymentProfile};

#[derive(Debug, Clone)]
pub struct BuildRequest {
    pub manifest_ref: String,
    pub manifest_revision: i64,
    pub manifest_snapshot: serde_json::Value,
    /// Immutable selected artifact identity in addition to the mutable
    /// manifest snapshot. Distributed installer roles use the distribution
    /// composition hash here.
    pub artifact_identity: String,
    pub requested_by: String,
    pub reason: Option<String>,
    pub modules_delta: String,
    pub modules: HashMap<String, ModuleSpec>,
    pub profile: DeploymentProfile,
    pub execution_plan: BuildExecutionPlan,
}

#[derive(Debug, Clone, Default)]
pub struct ReleaseArtifactBundle {
    pub container_image: Option<String>,
    pub server_artifact_url: Option<String>,
    pub admin_artifact_url: Option<String>,
    pub storefront_artifact_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSpec {
    pub source: String,
    pub crate_name: String,
    pub version: Option<String>,
    pub git: Option<String>,
    pub rev: Option<String>,
    pub path: Option<String>,
}
