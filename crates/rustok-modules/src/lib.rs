//! Module platform ownership: artifact identity, release lineage and lifecycle.

mod artifact;
mod artifact_cas;
mod contracts;
mod definition;
mod dependency;
mod dispatcher;
mod executor;
mod installation;
mod lifecycle;
mod lifecycle_writer;
mod migrations;
#[cfg(feature = "oci-distribution")]
mod oci;
mod operation_store;
mod policy;
mod recovery;
mod resolution;
mod runtime;
mod trust;

use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use artifact::{
    ArtifactModuleKind, ArtifactOrigin, ArtifactPayloadKind, ArtifactPermissionDescriptor,
    ArtifactPersistenceContract, ArtifactRelease, ArtifactReleaseDraft, ArtifactReleaseRef,
    ArtifactSourceLineage, ArtifactUiContribution, ModuleArtifactDescriptor, ModuleArtifactError,
    ModuleBindingIdempotency, ModuleDependencyConstraint, ModuleRuntimeBinding,
    ModuleRuntimeBindingKind,
};
pub use artifact_cas::StorageArtifactBlobStore;
pub use contracts::{
    ControlPlaneRevision, ModuleCommandContext, ModuleControlPlaneError,
    ModuleControlPlaneSnapshot, ModuleErrorCode, ModuleSnapshotKind, RevisionedModuleCommand,
};
pub use definition::{
    ModuleDefinition, ModuleDefinitionCatalog, ModuleDefinitionError, ModuleDefinitionKind,
    ModuleDefinitionSource,
};
pub use dependency::{
    ModuleDependencyLockError, ModuleDependencyLockGraph, ModuleDependencyLockNode,
};
pub use dispatcher::{
    ArtifactLifecycleDispatch, ArtifactLifecycleExecutor, ModuleDispatchError,
    ModuleExecutionDispatcher, ModuleLifecycleHookPhase,
};
pub use executor::{
    execute_module_toggle, ModuleLifecycleExecutionError, ModuleLifecycleToggleRequest,
    ModuleLifecycleToggleResult,
};
pub use installation::{
    ArtifactAdmissionLimits, ArtifactAdmissionReconciler, ArtifactAdmissionRecoveryRecord,
    ArtifactAdmissionService, ArtifactAdmissionStage, ArtifactAdmissionStatus,
    ArtifactAdmissionStore, ArtifactBlobRetentionPolicy, ArtifactBlobStore, ArtifactRegistry,
    DurableArtifactBlobStore, InMemoryArtifactBlobStore, InstalledModuleArtifact,
    ModuleArtifactPackage, ModuleInstallationError, ModuleInstallationScope, ModuleInstaller,
    OciArtifactReference, SeaOrmArtifactInstallationStore, StagedArtifactBlob,
};
pub use lifecycle::{ModuleOperationIssue, ModuleOperationRecoveryAction, ModuleOperationStatus};
pub use lifecycle_writer::{
    persist_module_settings, ModuleLifecycleDbWriter, ModuleLifecycleDbWriterError,
};
#[cfg(feature = "oci-distribution")]
pub use oci::OciDistributionArtifactRegistry;
pub use operation_store::{
    ModuleOperationJournal, ModuleOperationRecord, ModuleOperationRequest, ModuleOperationSnapshot,
    ModuleOperationStoreError, TenantModuleSettingsRecord, TenantModuleSettingsRequest,
    TenantModuleStateRecord, TenantModuleStateRequest, TenantModuleStateStore,
};
pub use policy::{
    resolve_effective_modules, validate_module_toggle, ModuleToggleValidationError,
    TenantModuleOverride,
};
pub use recovery::{
    failed_module_operation_recovery_plans, module_operation_recovery_plan,
    retry_failed_post_hook_operation, ModuleOperationRecoveryError, ModuleOperationRecoveryPlan,
    ModulePostHookRetryRequest,
};
pub use resolution::{
    resolve_module_dependencies, ModuleResolutionCandidate, ModuleResolutionConflict,
    ModuleResolutionError, ModuleResolutionProvider, ModuleResolutionRequest,
    ModuleResolutionResult,
};
pub use runtime::{
    ArtifactInstallationResolver, ArtifactRuntime, ArtifactRuntimeError,
    ArtifactRuntimeLifecycleExecutor, ArtifactSandboxPolicyResolver,
};
pub use trust::{
    TrustPolicyRevision, TrustVerificationDecision, TrustVerificationRequest, TrustVerifier,
};

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
