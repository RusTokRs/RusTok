//! Immutable build execution-plan contracts shared by workers and CLI adapters.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildExecutionPlan {
    pub cargo_package: String,
    pub cargo_profile: String,
    pub cargo_target: Option<String>,
    pub cargo_features: Vec<String>,
    pub cargo_command: String,
    #[serde(default)]
    pub admin_build: Option<FrontendBuildPlan>,
    #[serde(default)]
    pub storefront_build: Option<FrontendBuildPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendBuildTool {
    Cargo,
    Trunk,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendArtifactKind {
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontendBuildPlan {
    pub surface: String,
    pub tool: FrontendBuildTool,
    pub package: String,
    pub workspace_path: String,
    pub profile: String,
    pub target: Option<String>,
    pub artifact_path: String,
    pub artifact_kind: FrontendArtifactKind,
    pub command: String,
}

pub fn parse_execution_plan(
    build_id: Uuid,
    modules_delta: Option<&serde_json::Value>,
) -> anyhow::Result<BuildExecutionPlan> {
    let value = modules_delta
        .ok_or_else(|| anyhow::anyhow!("build {build_id} does not contain execution metadata"))?;
    let plan = value
        .get("execution_plan")
        .ok_or_else(|| anyhow::anyhow!("build {build_id} is missing execution_plan metadata"))?;
    serde_json::from_value(plan.clone()).map_err(|error| {
        anyhow::anyhow!("build {build_id} has invalid execution_plan metadata: {error}")
    })
}
