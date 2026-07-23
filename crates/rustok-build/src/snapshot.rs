//! Mapping from build-owner persistence models to neutral API snapshots.

use rustok_api::{PlatformBuildSnapshot, PlatformReleaseSnapshot};

use crate::build::{BuildStage, BuildStatus, DeploymentProfile, Model as Build};
use crate::plan::BuildExecutionPlan;
use crate::release::{Model as Release, ReleaseStatus};

pub fn build_snapshot(model: &Build) -> PlatformBuildSnapshot {
    let execution_plan = execution_plan(model.modules_delta.as_ref());

    PlatformBuildSnapshot {
        id: model.id.to_string(),
        status: build_status(model.status.clone()).to_string(),
        stage: build_stage(model.stage.clone()).to_string(),
        progress: model.progress,
        profile: deployment_profile(model.profile.clone()).to_string(),
        manifest_ref: model.manifest_ref.clone(),
        manifest_hash: model.manifest_hash.clone(),
        manifest_revision: model.manifest_revision,
        modules_delta: modules_delta_summary(model.modules_delta.as_ref()),
        build_command: execution_plan
            .as_ref()
            .map(|plan| plan.cargo_command.clone()),
        build_features: execution_plan
            .as_ref()
            .map(|plan| plan.cargo_features.clone())
            .unwrap_or_default(),
        build_target: execution_plan
            .as_ref()
            .and_then(|plan| plan.cargo_target.clone()),
        build_profile: execution_plan.map(|plan| plan.cargo_profile),
        requested_by: model.requested_by.clone(),
        reason: model.reason.clone(),
        release_id: model.release_id.clone(),
        logs_url: model.logs_url.clone(),
        error_message: model.error_message.clone(),
        started_at: model.started_at.map(|value| value.to_rfc3339()),
        finished_at: model.finished_at.map(|value| value.to_rfc3339()),
        created_at: model.created_at.to_rfc3339(),
        updated_at: model.updated_at.to_rfc3339(),
    }
}

pub fn release_snapshot(model: &Release) -> PlatformReleaseSnapshot {
    PlatformReleaseSnapshot {
        id: model.id.clone(),
        build_id: model.build_id.to_string(),
        status: release_status(model.status.clone()).to_string(),
        environment: model.environment.clone(),
        container_image: model.container_image.clone(),
        server_artifact_url: model.server_artifact_url.clone(),
        admin_artifact_url: model.admin_artifact_url.clone(),
        storefront_artifact_url: model.storefront_artifact_url.clone(),
        manifest_hash: model.manifest_hash.clone(),
        manifest_revision: model.manifest_revision,
        modules: serde_json::from_value(model.modules.clone()).unwrap_or_default(),
        previous_release_id: model.previous_release_id.clone(),
        deployed_at: model.deployed_at.map(|value| value.to_rfc3339()),
        rolled_back_at: model.rolled_back_at.map(|value| value.to_rfc3339()),
        created_at: model.created_at.to_rfc3339(),
        updated_at: model.updated_at.to_rfc3339(),
    }
}

fn execution_plan(value: Option<&serde_json::Value>) -> Option<BuildExecutionPlan> {
    value
        .and_then(|value| value.get("execution_plan"))
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

fn modules_delta_summary(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };

    if let Some(summary) = value.as_str() {
        return summary.to_string();
    }

    if let Some(summary) = value.get("summary").and_then(serde_json::Value::as_str) {
        return summary.to_string();
    }

    if let Some(object) = value.as_object() {
        let mut slugs = object.keys().cloned().collect::<Vec<_>>();
        slugs.sort();
        return slugs.join(",");
    }

    value.to_string()
}

fn build_status(status: BuildStatus) -> &'static str {
    match status {
        BuildStatus::Queued => "QUEUED",
        BuildStatus::Running => "RUNNING",
        BuildStatus::Success => "SUCCESS",
        BuildStatus::Failed => "FAILED",
        BuildStatus::Cancelled => "CANCELLED",
    }
}

fn build_stage(stage: BuildStage) -> &'static str {
    match stage {
        BuildStage::Pending => "PENDING",
        BuildStage::Checkout => "CHECKOUT",
        BuildStage::Build => "BUILD",
        BuildStage::Test => "TEST",
        BuildStage::Deploy => "DEPLOY",
        BuildStage::Complete => "COMPLETE",
    }
}

fn deployment_profile(profile: DeploymentProfile) -> &'static str {
    match profile {
        DeploymentProfile::Monolith => "MONOLITH",
        DeploymentProfile::ServerWithAdmin => "SERVER_WITH_ADMIN",
        DeploymentProfile::ServerWithStorefront => "SERVER_WITH_STOREFRONT",
        DeploymentProfile::HeadlessApi => "HEADLESS_API",
        DeploymentProfile::Worker => "WORKER",
        DeploymentProfile::Registry => "REGISTRY",
    }
}

fn release_status(status: ReleaseStatus) -> &'static str {
    match status {
        ReleaseStatus::Pending => "PENDING",
        ReleaseStatus::Deploying => "DEPLOYING",
        ReleaseStatus::Active => "ACTIVE",
        ReleaseStatus::RolledBack => "ROLLED_BACK",
        ReleaseStatus::Failed => "FAILED",
    }
}
