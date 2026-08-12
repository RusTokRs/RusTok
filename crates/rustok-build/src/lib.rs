//! Platform-owned build and release persistence contracts.

pub mod build;
pub mod control;
pub mod events;
pub mod execution;
pub mod executor;
pub mod module_manifest_contribution;
pub mod plan;
pub mod report;
pub mod request;
pub mod service;
pub mod snapshot;

pub use build::{BuildStage, BuildStatus, DeploymentProfile};
pub use control::{BuildControl, SharedBuildControl};
pub use events::{
    BuildEvent, BuildEventPublisher, EventBusBuildEventPublisher, NoopBuildEventPublisher,
};
pub use execution::{BuildCommandSpec, build_manifest_snapshot_path, run_build_command};
pub use executor::BuildExecutionService;
pub use module_manifest_contribution::{
    ContributionRoleExport, NormalizedModuleContributionManifest,
    normalize_module_contribution_manifest,
};
pub use plan::{
    BuildExecutionPlan, BuildRuntimeMode, FrontendArtifactKind, FrontendBuildPlan,
    FrontendBuildTool, RoleBuildPlan, parse_execution_plan,
};
pub use report::BuildExecutionReport;
pub use request::{BuildRequest, ModuleSpec};
pub use service::BuildService;
pub use snapshot::build_snapshot;
