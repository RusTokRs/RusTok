//! Module platform ownership: artifact identity, release lineage and lifecycle.

mod artifact;
mod artifact_cas;
mod build;
mod composition;
mod contracts;
mod data;
mod definition;
mod dependency;
mod dispatcher;
mod execution_audit;
mod executor;
mod governance;
mod installation;
mod lifecycle;
mod lifecycle_writer;
mod mcp;
mod migrations;
#[cfg(feature = "oci-distribution")]
mod oci;
mod operation_store;
mod policy;
mod recovery;
mod resolution;
mod runtime;
mod secrets;
mod trust;

use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use artifact::{
    canonical_schema_digest, ArtifactModuleKind, ArtifactOrigin, ArtifactPayloadKind,
    ArtifactPermissionDescriptor, ArtifactPersistenceContract, ArtifactRelease,
    ArtifactReleaseDraft, ArtifactReleaseRef, ArtifactSchemaDocument, ArtifactSourceLineage,
    ArtifactUiContribution, ModuleArtifactDescriptor, ModuleArtifactError,
    ModuleBindingIdempotency, ModuleDependencyConstraint, ModuleHttpBinding, ModuleHttpMethod,
    ModuleHttpStreamingPolicy, ModuleRuntimeBinding, ModuleRuntimeBindingKind,
    ModuleScheduleBinding, ModuleScheduleDeduplication, ModuleScheduleMisfirePolicy,
    ModuleScheduleOverlapPolicy, MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
    MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE, MODULE_ARTIFACT_SIDECAR_MEDIA_TYPE,
    MODULE_ARTIFACT_STATIC_PROMOTION_MEDIA_TYPE, MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE,
};
pub use artifact_cas::StorageArtifactBlobStore;
pub use build::{
    ModuleBuildAuthoring, ModuleBuildComponentInterface, ModuleBuildDependencyPolicy,
    ModuleBuildDiagnostic, ModuleBuildDiagnosticStage, ModuleBuildEvidence, ModuleBuildFailureCode,
    ModuleBuildLimits, ModuleBuildMetrics, ModuleBuildNetworkPolicy, ModuleBuildNextAction,
    ModuleBuildOutcome, ModuleBuildProtocolError, ModuleBuildPublicationReceipt,
    ModuleBuildRequest, ModuleBuildResult, ModuleBuildResultRecord, ModuleBuildSignatureAuthority,
    ModuleBuildSource, ModuleBuildSubmission, ModuleBuildToolchain, ModuleBuildValidationProfile,
    ModuleBuildWitContract, ModuleBuildWorker, SeaOrmModuleBuildService,
    MODULE_BUILD_PROTOCOL_VERSION,
};
pub use composition::{
    ModuleCompositionBuildEnqueuer, ModuleCompositionError, ModuleCompositionSnapshot,
    ModuleCompositionUpdate, SeaOrmModuleCompositionService, ACTIVE_MODULE_COMPOSITION_ID,
};
pub use contracts::{
    ControlPlaneRevision, ModuleCommandContext, ModuleControlPlaneError,
    ModuleControlPlaneSnapshot, ModuleErrorCode, ModuleSnapshotKind, RevisionedModuleCommand,
};
pub use data::{
    validate_artifact_data_key, validate_artifact_data_prefix, ArtifactBindingDataUpgradeHook,
    ArtifactDataAccess, ArtifactDataAuthorizer, ArtifactDataBroker, ArtifactDataError,
    ArtifactDataMigrationCheckpointStore, ArtifactDataPage, ArtifactDataPageRequest,
    ArtifactDataPurgeAuthorizer, ArtifactDataPurgeRequest, ArtifactDataPurgeResult,
    ArtifactDataRecord, ArtifactDataSchemaValidator, ArtifactDataScope, ArtifactDataUpgradeApplier,
    ArtifactDataUpgradeApplyRequest, ArtifactDataUpgradeApplyResult, ArtifactDataUpgradeHook,
    ArtifactDataUpgradeInput, ArtifactDataUpgradePlan, ArtifactDataUpgradePlanner,
    ArtifactDataUpgradeRecord, ArtifactDataUpgradeRequest, ArtifactDataWrite,
    SeaOrmArtifactDataBroker, SeaOrmArtifactDataCapabilityBroker, SeaOrmArtifactDataPurgeService,
    SeaOrmArtifactDataSchemaValidator,
};
pub use definition::{
    ModuleDefinition, ModuleDefinitionCatalog, ModuleDefinitionError, ModuleDefinitionKind,
    ModuleDefinitionSource,
};
pub use dependency::{
    ModuleDependencyLockError, ModuleDependencyLockGraph, ModuleDependencyLockNode,
};
pub use dispatcher::{
    ArtifactBindingDispatch, ArtifactBindingExecutor, ArtifactLifecycleExecutor,
    ModuleDispatchError, ModuleExecutionDispatcher, ModuleLifecycleHookPhase,
};
pub use execution_audit::SeaOrmArtifactExecutionObserver;
pub use executor::{
    execute_module_toggle, ModuleLifecycleExecutionError, ModuleLifecycleToggleRequest,
    ModuleLifecycleToggleResult,
};
pub use governance::{
    ModuleBuildServiceAttestationCommand, ModuleGovernanceError, ModuleOwnerBindCommand,
    ModuleOwnerTransferCommand, ModulePlatformAdmissionCommand, ModulePublicationEvidenceAuthority,
    ModulePublicationEvidenceCommand, ModulePublicationEvidenceResult,
    ModulePublishApprovalOverride, ModulePublishArtifactAttachCommand,
    ModulePublishArtifactAttachResult, ModulePublishRequestChangesCommand,
    ModulePublishRequestCreateCommand, ModulePublishRequestHoldCommand,
    ModulePublishRequestPublicationCommand, ModulePublishRequestRejectCommand,
    ModulePublishRequestResumeCommand, ModuleReleaseYankCommand, ModuleRemoteValidationClaim,
    ModuleRemoteValidationClaimCommand, ModuleRemoteValidationHeartbeatCommand,
    ModuleRemoteValidationTerminalCommand, ModuleRemoteValidationTerminalOutcome,
    ModuleValidationJobClaimCommand, ModuleValidationJobClaimResult,
    ModuleValidationJobEnqueueCommand, ModuleValidationJobEnqueueResult,
    ModuleValidationJobResultCommand, ModuleValidationJobResultOutcome,
    ModuleValidationJobRetryCommand, ModuleValidationStageReportCommand,
    SeaOrmModuleGovernanceService, REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
    REGISTRY_HOLD_REASON_CODES, REGISTRY_OWNER_TRANSFER_REASON_CODES, REGISTRY_REJECT_REASON_CODES,
    REGISTRY_REQUEST_CHANGES_REASON_CODES, REGISTRY_RESUME_REASON_CODES,
    REGISTRY_VALIDATION_STAGE_REASON_CODES, REGISTRY_YANK_REASON_CODES,
};
pub use installation::{
    ArtifactAdmissionCommand, ArtifactAdmissionLimits, ArtifactAdmissionReconciler,
    ArtifactAdmissionRecoveryRecord, ArtifactAdmissionResult, ArtifactAdmissionReverification,
    ArtifactAdmissionService, ArtifactAdmissionStage, ArtifactAdmissionStatus,
    ArtifactAdmissionStore, ArtifactBlobRetentionPolicy, ArtifactBlobRetentionRule,
    ArtifactBlobStore, ArtifactDeactivationRequest, ArtifactDeactivationResult,
    ArtifactMigrationCheckpointRequest, ArtifactMigrationRollbackMode, ArtifactPayloadSource,
    ArtifactRegistry, ArtifactRollbackRequest, ArtifactRollbackResult,
    ArtifactTenantDisableRequest, ArtifactTenantDisableResult, ArtifactUninstallRequest,
    ArtifactUninstallResult, ArtifactVerificationEvidence, DurableArtifactBlobStore,
    InMemoryArtifactBlobStore, InstalledModuleArtifact, ModuleArtifactPackage,
    ModuleInstallationError, ModuleInstallationScope, ModuleInstaller, OciArtifactReference,
    SeaOrmArtifactInstallationStore, SnapshotArtifactBlobRetentionPolicy, StagedArtifactBlob,
};
pub use lifecycle::{ModuleOperationIssue, ModuleOperationRecoveryAction, ModuleOperationStatus};
pub use lifecycle_writer::{
    persist_module_settings, ModuleLifecycleDbWriter, ModuleLifecycleDbWriterError,
};
pub use mcp::{
    ArtifactMcpCallRequest, ArtifactMcpCapabilityBroker, ArtifactMcpError, ArtifactMcpInvoker,
};
#[cfg(feature = "oci-distribution")]
pub use oci::{
    strict_oci_distribution_client, OciArtifactEvidence, OciArtifactEvidenceKind,
    OciArtifactPublicationBundle, OciArtifactPublicationError, OciArtifactPublicationReceipt,
    OciArtifactPublicationTarget, OciArtifactPublisher, OciDistributionArtifactPublisher,
    OciDistributionArtifactRegistry, MODULE_ARTIFACT_DESCRIPTOR_MEDIA_TYPE,
    MODULE_ARTIFACT_PROVENANCE_MEDIA_TYPE, MODULE_ARTIFACT_RELEASE_LINEAGE_MEDIA_TYPE,
    MODULE_ARTIFACT_SBOM_MEDIA_TYPE, MODULE_ARTIFACT_TEST_EVIDENCE_MEDIA_TYPE,
    OCI_EMPTY_CONFIG_MEDIA_TYPE,
};
pub use operation_store::{
    ModuleOperationJournal, ModuleOperationRecord, ModuleOperationRequest, ModuleOperationSnapshot,
    ModuleOperationStoreError, TenantModuleSettingsRecord, TenantModuleSettingsRequest,
    TenantModuleStateRecord, TenantModuleStateRequest, TenantModuleStateStore,
};
pub use policy::{
    validate_module_toggle, ModuleEffectivePolicy, ModuleEffectivePolicyQuery,
    ModuleToggleValidationError, TenantModuleOverride,
};
pub use recovery::{
    failed_module_operation_recovery_plans, module_operation_recovery_plan,
    retry_failed_post_hook_operation, ModuleOperationRecoveryError, ModuleOperationRecoveryPlan,
    ModulePostHookRetryRequest,
};
pub use resolution::{
    resolve_module_dependencies, ModuleResolutionCandidate, ModuleResolutionConflict,
    ModuleResolutionError, ModuleResolutionProvider, ModuleResolutionProviderKind,
    ModuleResolutionRequest, ModuleResolutionResult, ModuleResolutionScope,
};
pub use runtime::{
    ArtifactInstallationResolver, ArtifactRuntime, ArtifactRuntimeError,
    ArtifactRuntimeLifecycleExecutor, ArtifactSandboxPolicyResolver, VerifiedArtifactNodeCache,
};
pub use secrets::{
    ArtifactSecretAuthorizer, ArtifactSecretBindingRequest, ArtifactSecretError,
    ArtifactSecretHandle, ArtifactSecretHandleAuthorizer, ArtifactSecretHandleRequest,
    ArtifactSecretPolicy, RegistryArtifactSecretAuthorizer, SeaOrmArtifactSecretCapabilityBroker,
    SeaOrmArtifactSecretHandleService, SeaOrmArtifactSecretService,
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
