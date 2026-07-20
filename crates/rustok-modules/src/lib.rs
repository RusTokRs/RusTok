//! Module platform ownership: artifact identity, release lineage and lifecycle.

mod artifact;
mod artifact_capability_router;
mod artifact_cas;
mod binding_idempotency;
mod build;
mod build_surface;
mod composition;
mod contracts;
mod control_plane;
mod data;
mod definition;
mod dependency;
mod dispatcher;
mod event_delivery;
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
mod publish_validation;
mod recovery;
mod resolution;
mod runtime;
mod runtime_handles;
mod schedule_delivery;
mod schedule_materializer;
mod secrets;
mod settings;
mod static_package;
mod trust;

use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use artifact::{
    canonical_schema_digest, schedule_binding_digest, ArtifactDataIndexField,
    ArtifactDataIndexValueType, ArtifactModuleKind, ArtifactOrigin, ArtifactPayloadKind,
    ArtifactPermissionDescriptor, ArtifactPersistenceContract, ArtifactRelease,
    ArtifactReleaseDraft, ArtifactReleaseRef, ArtifactSchemaDocument, ArtifactSourceLineage,
    ArtifactUiContribution, ModuleArtifactDescriptor, ModuleArtifactError,
    ModuleBindingIdempotency, ModuleDependencyConstraint, ModuleHttpBinding, ModuleHttpMethod,
    ModuleHttpStreamingPolicy, ModuleRuntimeBinding, ModuleRuntimeBindingKind,
    ModuleScheduleBinding, ModuleScheduleDeduplication, ModuleScheduleMisfirePolicy,
    ModuleScheduleOverlapPolicy, MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
    MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE, MODULE_ARTIFACT_RHAI_WORKSPACE_MEDIA_TYPE,
    MODULE_ARTIFACT_SIDECAR_MEDIA_TYPE, MODULE_ARTIFACT_STATIC_PROMOTION_MEDIA_TYPE,
    MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE,
};
pub use artifact_capability_router::{
    resolve_granted_artifact_capability, ArtifactCapabilityBrokerResolver,
    ArtifactCapabilityBrokerResolverRouter, ArtifactCapabilityExecution,
    ResolvingArtifactCapabilityBroker,
};
pub use artifact_cas::StorageArtifactBlobStore;
pub use binding_idempotency::{
    artifact_binding_request_digest, ArtifactBindingIdempotencyClaim,
    ArtifactBindingIdempotencyError, ArtifactBindingIdempotencyRequest,
    SeaOrmArtifactBindingIdempotencyStore,
};
pub use build::{
    ModuleBuildAuthoring, ModuleBuildCompletedResult, ModuleBuildComponentInterface,
    ModuleBuildDependencyPolicy, ModuleBuildDiagnostic, ModuleBuildDiagnosticStage,
    ModuleBuildEvidence, ModuleBuildFailureCode, ModuleBuildLimits, ModuleBuildMetrics,
    ModuleBuildNetworkPolicy, ModuleBuildNextAction, ModuleBuildOutcome, ModuleBuildProtocolError,
    ModuleBuildPublicationReceipt, ModuleBuildRequest, ModuleBuildResult, ModuleBuildResultRecord,
    ModuleBuildSignatureAuthority, ModuleBuildSource, ModuleBuildSubmission, ModuleBuildToolchain,
    ModuleBuildValidationOutcome, ModuleBuildValidationProfile, ModuleBuildValidationResult,
    ModuleBuildWitContract, ModuleBuildWorker, ModuleBuildWorkerReadiness,
    SeaOrmModuleBuildService, MODULE_BUILD_PROTOCOL_VERSION,
};
pub use build_surface::{
    validate_platform_build_surface_contract, PlatformAdminBuildSurfaceContract,
    PlatformBuildSurfaceContract, PlatformBuildSurfaceValidationError,
    PlatformStorefrontBuildSurfaceContract,
};
pub use composition::{
    ModuleCompositionBuildEnqueuer, ModuleCompositionError, ModuleCompositionSnapshot,
    ModuleCompositionUpdate, SeaOrmModuleCompositionService, ACTIVE_MODULE_COMPOSITION_ID,
};
pub use contracts::{
    ControlPlaneRevision, ModuleCommandContext, ModuleControlPlaneError,
    ModuleControlPlaneSnapshot, ModuleErrorCode, ModuleSnapshotKind, RevisionedModuleCommand,
};
pub use control_plane::{EffectivePolicyService, ModuleControlPlane};
pub use data::{
    validate_artifact_data_key, validate_artifact_data_prefix, ArtifactBindingDataUpgradeHook,
    ArtifactDataAccess, ArtifactDataAuthorizer, ArtifactDataBatchWrite, ArtifactDataBroker,
    ArtifactDataError, ArtifactDataExportAuthorizer, ArtifactDataExportRequest,
    ArtifactDataExportResult, ArtifactDataIndexQuery, ArtifactDataMigrationCheckpointStore,
    ArtifactDataObject, ArtifactDataObjectBroker, ArtifactDataObjectContent,
    ArtifactDataObjectGcResult, ArtifactDataObjectPage, ArtifactDataObjectRetentionPolicy,
    ArtifactDataObjectRetentionRule, ArtifactDataObjectUpload, ArtifactDataObjectUploadChunk,
    ArtifactDataObjectUploadCompleteRequest, ArtifactDataObjectUploadReapResult,
    ArtifactDataObjectUploadSession, ArtifactDataObjectUploadSessionRequest, ArtifactDataPage,
    ArtifactDataPageRequest, ArtifactDataPurgeAuthorizer, ArtifactDataPurgeRequest,
    ArtifactDataPurgeResult, ArtifactDataRecord, ArtifactDataSchemaValidator, ArtifactDataScope,
    ArtifactDataUpgradeApplier, ArtifactDataUpgradeApplyRequest, ArtifactDataUpgradeApplyResult,
    ArtifactDataUpgradeHook, ArtifactDataUpgradeInput, ArtifactDataUpgradePlan,
    ArtifactDataUpgradePlanner, ArtifactDataUpgradeRecord, ArtifactDataUpgradeRequest,
    ArtifactDataWrite, SeaOrmArtifactDataBroker, SeaOrmArtifactDataCapabilityBroker,
    SeaOrmArtifactDataCapabilityBrokerResolver, SeaOrmArtifactDataExportService,
    SeaOrmArtifactDataObjectBroker, SeaOrmArtifactDataObjectCapabilityBroker,
    SeaOrmArtifactDataObjectCapabilityBrokerResolver, SeaOrmArtifactDataObjectGcService,
    SeaOrmArtifactDataObjectUploadService, SeaOrmArtifactDataPurgeService,
    SeaOrmArtifactDataSchemaValidator, SnapshotArtifactDataObjectRetentionPolicy,
};
pub use definition::{
    ModuleDefinition, ModuleDefinitionCatalog, ModuleDefinitionError, ModuleDefinitionKind,
    ModuleDefinitionSource,
};
pub use dependency::{
    ModuleDependencyLockError, ModuleDependencyLockGraph, ModuleDependencyLockNode,
};
pub use dispatcher::{
    dispatch_artifact_command_binding, dispatch_artifact_http_binding,
    find_artifact_command_binding, find_artifact_http_binding, ArtifactBindingDispatch,
    ArtifactBindingDispatchEnvelope, ArtifactBindingDispatchEnvelopeError,
    ArtifactBindingExecutionContext, ArtifactBindingExecutor, ArtifactInstallationTarget,
    ArtifactLifecycleExecutor, ModuleDispatchError, ModuleExecutionDispatcher,
    ModuleLifecycleHookPhase, ARTIFACT_BINDING_DISPATCH_ENVELOPE_VERSION,
};
pub use event_delivery::{
    ArtifactEventDeliveryCompletion, ArtifactEventDeliveryConfig, ArtifactEventDeliveryError,
    ArtifactEventDeliveryOutcome, ArtifactEventDeliveryReceipt, ArtifactEventDeliveryRequest,
    ArtifactEventDeliverySource, ArtifactEventDeliveryWorkAdapter, ArtifactEventDeliveryWorkItem,
    ArtifactEventDeliveryWorkRegistration, ArtifactEventProjectionTransport,
    SeaOrmArtifactEventDeliveryQueue, SeaOrmArtifactEventSubscriptionProjector,
    ARTIFACT_EVENT_DELIVERY_WORKER,
};
pub use execution_audit::SeaOrmArtifactExecutionObserver;
pub use executor::{
    execute_module_toggle, ModuleLifecycleExecutionError, ModuleLifecycleToggleRequest,
    ModuleLifecycleToggleResult,
};
pub use governance::{
    ModuleAlloyAuthoredStageCommand, ModuleAlloyAuthoredStageResult,
    ModuleBuildServiceAttestationCommand, ModuleExternalPrebuiltStageCommand,
    ModuleExternalPrebuiltStageResult, ModuleExternalSourceEvidence, ModuleGovernanceError,
    ModuleOwnerBindCommand, ModuleOwnerTransferCommand, ModulePlatformAdmissionCommand,
    ModulePublicationArtifactOrigin, ModulePublicationEvidenceAuthority,
    ModulePublicationEvidenceCommand, ModulePublicationEvidenceResult,
    ModulePublishApprovalOverride, ModulePublishArtifactAttachCommand,
    ModulePublishArtifactAttachResult, ModulePublishPlatformBuildStageCommand,
    ModulePublishPlatformBuildStageResult, ModulePublishRequestChangesCommand,
    ModulePublishRequestCreateCommand, ModulePublishRequestHoldCommand,
    ModulePublishRequestPublicationCommand, ModulePublishRequestRejectCommand,
    ModulePublishRequestResumeCommand, ModulePublishValidationContract, ModuleReleaseYankCommand,
    ModuleRemoteValidationClaim, ModuleRemoteValidationClaimCommand,
    ModuleRemoteValidationHeartbeatCommand, ModuleRemoteValidationTerminalCommand,
    ModuleRemoteValidationTerminalOutcome, ModuleValidationJobClaimCommand,
    ModuleValidationJobClaimResult, ModuleValidationJobEnqueueCommand,
    ModuleValidationJobEnqueueResult, ModuleValidationJobResultCommand,
    ModuleValidationJobResultOutcome, ModuleValidationJobRetryCommand, ModuleValidationJobWorkItem,
    ModuleValidationStageReportCommand, SeaOrmModuleGovernanceService,
    REGISTRY_APPROVE_OVERRIDE_REASON_CODES, REGISTRY_EXTERNAL_SOURCE_ABSENCE_REASON_CODES,
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
    ArtifactTenantDisableRequest, ArtifactTenantDisableResult, ArtifactTenantEnableRequest,
    ArtifactTenantEnableResult, ArtifactUninstallRequest, ArtifactUninstallResult,
    ArtifactVerificationEvidence, DurableArtifactBlobStore, InMemoryArtifactBlobStore,
    InstalledModuleArtifact, ModuleArtifactPackage, ModuleInstallationError,
    ModuleInstallationScope, ModuleInstaller, OciArtifactReference,
    SeaOrmArtifactInstallationStore, SeaOrmArtifactSandboxPolicyResolver,
    SnapshotArtifactBlobRetentionPolicy, StagedArtifactBlob,
};
pub use lifecycle::{ModuleOperationIssue, ModuleOperationRecoveryAction, ModuleOperationStatus};
pub use lifecycle_writer::{
    persist_module_settings, ModuleLifecycleDbWriter, ModuleLifecycleDbWriterError,
};
pub use mcp::{
    ArtifactMcpCallRequest, ArtifactMcpCapabilityBroker, ArtifactMcpCapabilityBrokerResolver,
    ArtifactMcpError, ArtifactMcpInvoker,
};
#[cfg(feature = "oci-distribution")]
pub use oci::{
    strict_oci_distribution_client, strict_oci_distribution_client_with_policy,
    OciArtifactEvidence, OciArtifactEvidenceKind,
    OciArtifactPublicationBundle, OciArtifactPublicationError, OciArtifactPublicationReceipt,
    OciArtifactPublicationTarget, OciArtifactPublisher, OciDistributionArtifactPublisher,
    OciDistributionArtifactRegistry, OciRegistryProxyMode, OciRegistryTransportPolicy,
    MODULE_ARTIFACT_DESCRIPTOR_MEDIA_TYPE,
    MODULE_ARTIFACT_PROVENANCE_MEDIA_TYPE, MODULE_ARTIFACT_RELEASE_LINEAGE_MEDIA_TYPE,
    MODULE_ARTIFACT_SBOM_MEDIA_TYPE, MODULE_ARTIFACT_TEST_EVIDENCE_MEDIA_TYPE,
    OCI_EMPTY_CONFIG_MEDIA_TYPE,
};
pub use operation_store::{
    ModuleOperationJournal, ModuleOperationRecord, ModuleOperationRecordOutcome,
    ModuleOperationRequest, ModuleOperationSnapshot, ModuleOperationStoreError,
    TenantModuleSettingsRecord, TenantModuleSettingsRequest, TenantModuleStateRecord,
    TenantModuleStateRequest, TenantModuleStateStore,
};
pub use policy::{
    validate_module_toggle, ModuleEffectivePolicy, ModuleEffectivePolicyQuery,
    ModuleToggleValidationError, TenantModuleOverride,
};
pub use publish_validation::{
    validate_module_publish_artifact, validate_module_publish_bundle,
    ModulePublishBundleValidation, MODULE_PUBLISH_ALLOY_WORKSPACE_MAX_BYTES,
    MODULE_PUBLISH_ARTIFACT_MANIFEST_MAX_BYTES, MODULE_PUBLISH_ARTIFACT_MAX_BYTES,
    MODULE_PUBLISH_BUNDLE_TYPE,
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
pub use runtime_handles::{
    ArtifactDeliveryTenantSource, SharedArtifactBindingExecutor, SharedArtifactDeliveryTenantSource,
};
pub use schedule_delivery::{
    ArtifactScheduleDeliveryConfig, ArtifactScheduleDeliveryError, ArtifactScheduleDeliveryOutcome,
    ArtifactScheduleDeliveryReceipt, ArtifactScheduleDeliveryRequest,
    ArtifactScheduleDeliveryWorkAdapter, ArtifactScheduleDeliveryWorkItem,
    ArtifactScheduleDeliveryWorkRegistration, SeaOrmArtifactScheduleDeliveryQueue,
    ARTIFACT_SCHEDULE_DELIVERY_WORKER,
};
pub use schedule_materializer::{
    ArtifactScheduleMaterializationConfig, ArtifactScheduleMaterializationError,
    ArtifactScheduleMaterializationReport, ArtifactScheduleMaterializer,
};
pub use secrets::{
    ArtifactSecretAuthorizer, ArtifactSecretBindingRequest, ArtifactSecretError,
    ArtifactSecretHandle, ArtifactSecretHandleAuthorizer, ArtifactSecretHandleRequest,
    ArtifactSecretPolicy, RegistryArtifactSecretAuthorizer, SeaOrmArtifactSecretCapabilityBroker,
    SeaOrmArtifactSecretCapabilityBrokerResolver, SeaOrmArtifactSecretHandleService,
    SeaOrmArtifactSecretService,
};
pub use settings::{
    normalize_module_settings, validate_module_settings_schema, ModuleSettingSpec,
    ModuleSettingsValidationError,
};
pub use static_package::{
    is_valid_static_module_slug, resolve_static_module_entrypoints,
    resolve_static_module_ui_classification, static_module_platform_version_is_compatible,
    validate_static_module_catalog_contract, validate_static_module_http_provides_contract,
    validate_static_module_package_contract, validate_static_module_registry_contracts,
    validate_static_module_topology_contract, validate_static_module_ui_i18n_contract,
    StaticModuleCatalogContract, StaticModuleCatalogValidationError,
    StaticModuleEntrypointContract, StaticModuleEntrypointValidationError, StaticModuleEntrypoints,
    StaticModuleHttpProvidesContract, StaticModuleHttpProvidesValidationError,
    StaticModulePackageContract, StaticModulePackageValidationError,
    StaticModulePlatformVersionError, StaticModuleTopologyContract, StaticModuleTopologyModule,
    StaticModuleTopologyValidationError, StaticModuleUiClassificationError,
    StaticModuleUiI18nContract, StaticModuleUiI18nResolved, StaticModuleUiI18nValidationError,
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

    fn register_runtime_extensions(&self, extensions: &mut rustok_core::ModuleRuntimeExtensions) {
        let registrations = extensions
            .get_or_insert_with::<rustok_runtime::ModuleWorkRegistrations, _>(Default::default);
        registrations.register(std::sync::Arc::new(
            ArtifactEventDeliveryWorkRegistration::default(),
        ));
        registrations.register(std::sync::Arc::new(
            ArtifactScheduleDeliveryWorkRegistration::default(),
        ));
    }
}
