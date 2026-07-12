//! Module platform ownership: artifact identity, release lineage and lifecycle.

mod artifact;
mod contracts;
mod executor;
mod hooks;
mod installation;
mod lifecycle;
mod migrations;
#[cfg(feature = "oci-distribution")]
mod oci;
mod operation_store;
mod policy;
mod recovery;
mod runtime;
mod seed_writer;

use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use artifact::{
    ArtifactOrigin, ArtifactPayloadKind, ArtifactRelease, ArtifactReleaseDraft, ArtifactReleaseRef,
    ArtifactSourceLineage, ModuleArtifactDescriptor, ModuleArtifactError,
};
pub use contracts::{
    ControlPlaneRevision, ModuleCommandContext, ModuleControlPlaneError,
    ModuleControlPlaneSnapshot, ModuleErrorCode, ModuleSnapshotKind,
};
pub use executor::{
    ModuleLifecycleExecutionError, ModuleLifecycleToggleRequest, ModuleLifecycleToggleResult,
    execute_module_toggle,
};
pub use hooks::{ModuleLifecycleHookError, ModuleLifecycleHookPhase, run_module_lifecycle_hook};
pub use installation::{
    ArtifactInstallationStore, ArtifactRegistry, InstalledModuleArtifact, ModuleArtifactPackage,
    ModuleInstallationError, ModuleInstallationScope, ModuleInstaller, OciArtifactReference,
    SeaOrmArtifactInstallationStore,
};
pub use lifecycle::{ModuleOperationIssue, ModuleOperationRecoveryAction, ModuleOperationStatus};
#[cfg(feature = "oci-distribution")]
pub use oci::OciDistributionArtifactRegistry;
pub use operation_store::{
    ModuleOperationJournal, ModuleOperationRecord, ModuleOperationRequest, ModuleOperationSnapshot,
    ModuleOperationStoreError, TenantModuleSettingsRecord, TenantModuleSettingsRequest,
    TenantModuleStateRecord, TenantModuleStateRequest, TenantModuleStateStore,
};
pub use policy::{
    ModuleToggleValidationError, TenantModuleOverride, resolve_effective_modules,
    validate_module_toggle,
};
pub use recovery::{
    ModuleOperationRecoveryError, ModuleOperationRecoveryPlan, ModulePostHookRetryRequest,
    failed_module_operation_recovery_plans, module_operation_recovery_plan,
    retry_failed_post_hook_operation,
};
pub use runtime::{ArtifactRuntime, ArtifactRuntimeError};
pub use seed_writer::{ModuleLifecycleDbWriter, ModuleLifecycleDbWriterError};

/// Mandatory Core entry point for module and marketplace control-plane ownership.
pub struct ModulesModule;

impl MigrationSource for ModulesModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[async_trait]
impl RusToKModule for ModulesModule {
    fn slug(&self) -> &'static str {
        "modules"
    }

    fn name(&self) -> &'static str {
        "Module Platform"
    }

    fn description(&self) -> &'static str {
        "Mandatory module artifact, marketplace, and lifecycle control plane"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }
}
