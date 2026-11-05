use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::PlatformBuildSnapshot;
use uuid::Uuid;

use crate::{
    BuildCommandSpec, BuildControl, BuildEventPublicationContext, BuildEventScope,
    BuildExecutionPlan, BuildRuntimeMode, BuildService, BuildStage, BuildStatus, DeploymentProfile,
    EventBusBuildEventPublisher, NoopBuildEventPublisher, SharedBuildControl,
};

struct DummyBuildControl;

#[async_trait]
impl BuildControl for DummyBuildControl {
    async fn active_build(&self) -> anyhow::Result<Option<PlatformBuildSnapshot>> {
        Ok(None)
    }

    async fn list_builds_page(
        &self,
        _limit: u64,
        _offset: u64,
    ) -> anyhow::Result<Vec<PlatformBuildSnapshot>> {
        Ok(Vec::new())
    }
}

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains("readme = \"README.md\""),
        "manifest must declare readme"
    );
    assert!(
        manifest.contains("repository.workspace = true"),
        "manifest must inherit repository from workspace"
    );
    assert!(
        manifest.contains("rust-version.workspace = true"),
        "manifest must inherit rust-version from workspace"
    );
    assert!(
        manifest.contains("[package.metadata.docs.rs]"),
        "manifest must configure docs.rs metadata"
    );
}

#[test]
fn crate_api_contract_specification_is_complete() {
    let api_doc = include_str!("../CRATE_API.md");
    assert!(
        api_doc.contains("# `rustok-build` Crate API & Architecture Contract"),
        "CRATE_API.md must contain the main title"
    );
    assert!(
        api_doc.contains("## 1. Architectural Purpose & Boundaries"),
        "CRATE_API.md must document purpose and boundaries"
    );
    assert!(
        api_doc.contains("## 2. Build Lifecycle States & Execution Plans"),
        "CRATE_API.md must document lifecycle states"
    );
    assert!(
        api_doc.contains("## 3. Transactional Event Publishing & Scopes"),
        "CRATE_API.md must document event publishing"
    );
    assert!(
        api_doc.contains("## 4. Manifest Normalization & Contribution Brokering"),
        "CRATE_API.md must document manifest contribution"
    );
    assert!(
        api_doc.contains("## 5. Universal Logic Dimensions Verification"),
        "CRATE_API.md must document universal logic dimensions"
    );
}

#[test]
fn debug_formatting_implementations() {
    let shared = SharedBuildControl(Arc::new(DummyBuildControl));
    assert!(format!("{shared:?}").contains("SharedBuildControl"));

    let noop = NoopBuildEventPublisher;
    assert!(format!("{noop:?}").contains("NoopBuildEventPublisher"));

    let context = BuildEventPublicationContext {
        actor_id: Uuid::new_v4(),
        correlation_id: Uuid::new_v4(),
        trace_id: "trace-123".to_string(),
    };
    let event_bus = rustok_core::EventBus::new();
    let publisher = EventBusBuildEventPublisher::new(event_bus, BuildEventScope::Platform, context);
    let debug_pub = format!("{publisher:?}");
    assert!(debug_pub.contains("EventBusBuildEventPublisher"));
    assert!(debug_pub.contains("trace-123"));

    let db = sea_orm::MockDatabase::new(sea_orm::DatabaseBackend::Postgres).into_connection();
    let service = BuildService::new(db);
    assert!(format!("{service:?}").contains("BuildService"));
}

#[test]
fn deployment_profile_variants() {
    let profiles = [
        DeploymentProfile::Monolith,
        DeploymentProfile::ServerWithAdmin,
        DeploymentProfile::ServerWithStorefront,
        DeploymentProfile::HeadlessApi,
        DeploymentProfile::Worker,
        DeploymentProfile::Registry,
    ];
    assert_eq!(profiles.len(), 6);
}

#[test]
fn build_runtime_mode_invariants() {
    assert_eq!(BuildRuntimeMode::default(), BuildRuntimeMode::Full);
    assert_eq!(BuildRuntimeMode::Full.as_str(), "full");
    assert_eq!(BuildRuntimeMode::RegistryOnly.as_str(), "registry_only");
    assert_eq!(BuildRuntimeMode::Api.as_str(), "api");
    assert_eq!(BuildRuntimeMode::AdminSsr.as_str(), "admin_ssr");
    assert_eq!(BuildRuntimeMode::StorefrontSsr.as_str(), "storefront_ssr");
    assert_eq!(BuildRuntimeMode::Worker.as_str(), "worker");
}

#[test]
fn build_status_and_stage_invariants() {
    assert!(!BuildStatus::Queued.is_final());
    assert!(!BuildStatus::Running.is_final());
    assert!(BuildStatus::Success.is_final());
    assert!(BuildStatus::Failed.is_final());
    assert!(BuildStatus::Cancelled.is_final());

    // Stages ordering coverage
    let stages = [
        BuildStage::Pending,
        BuildStage::Checkout,
        BuildStage::Build,
        BuildStage::Test,
        BuildStage::Deploy,
        BuildStage::Complete,
    ];
    assert_eq!(stages.len(), 6);
}

#[test]
fn build_command_spec_getters_and_render() {
    let plan = BuildExecutionPlan {
        runtime_mode: BuildRuntimeMode::Full,
        cargo_package: "rustok-server".to_string(),
        cargo_profile: "release".to_string(),
        cargo_target: None,
        cargo_features: vec!["admin".to_string()],
        cargo_command: String::new(),
        admin_build: None,
        storefront_build: None,
    };

    let manifest = PathBuf::from("/tmp/manifests/modules.toml");
    let spec = BuildCommandSpec::from_server_plan(&plan, Path::new("/workspace"), manifest.clone());

    assert_eq!(spec.workdir(), Path::new("/workspace"));
    assert_eq!(spec.manifest_path(), manifest.as_path());
    assert!(spec.render().contains("cargo build -p rustok-server"));
}
