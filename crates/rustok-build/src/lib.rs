//! Platform-owned build and release persistence contracts.

pub mod build;
pub mod control;
pub mod events;
pub mod execution;
pub mod executor;
pub mod plan;
pub mod release;
pub mod report;
pub mod request;
pub mod runtime;
pub mod service;

pub use build::{BuildStage, BuildStatus, DeploymentProfile};
pub use control::{BuildControl, BuildRollbackCommand, SharedBuildControl};
pub use events::{
    BuildEvent, BuildEventPublisher, EventBusBuildEventPublisher, NoopBuildEventPublisher,
};
pub use execution::{BuildCommandSpec, build_manifest_snapshot_path, run_build_command};
pub use executor::BuildExecutionService;
pub use plan::{
    BuildExecutionPlan, BuildRuntimeMode, FrontendArtifactKind, FrontendBuildPlan,
    FrontendBuildTool, RoleBuildPlan, parse_execution_plan,
};
pub use release::ReleaseStatus;
pub use report::BuildExecutionReport;
pub use request::{BuildRequest, ModuleSpec, ReleaseArtifactBundle};
pub use runtime::{
    DeploymentBackend, DeploymentSettings, DeploymentWorkspace, NoopReleaseActivationHook,
    ReleaseActivationHook, ReleasePublishRequest, ReleasePublisherPort,
};
pub use service::BuildService;
