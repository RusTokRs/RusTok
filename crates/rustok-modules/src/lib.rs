//! Module platform ownership: artifact identity, release lineage and lifecycle.

mod artifact;
mod artifact_capability_router;
mod artifact_cas;
mod artifact_schema;
mod binding_idempotency;
mod build;
mod build_surface;
mod composition;
mod contracts;
mod control_plane;
mod data;
mod data_snapshot;
mod definition;
mod dependency;
mod dispatcher;
mod distribution;
mod distribution_release;
mod distribution_rollout;
mod event_delivery;
mod execution_audit;
mod executor;
mod governance;
mod infrastructure;
mod installation;
mod lifecycle;
mod lifecycle_writer;
mod marketplace;
mod marketplace_content;
mod mcp;
mod migrations;
#[cfg(feature = "oci-distribution")]
mod oci;
mod operation_store;
mod policy;
mod policy_revision_consumer;
mod policy_transition_event;
mod promotion;
mod publish_validation;
mod recovery;
mod resolution;
mod runtime;
mod runtime_handles;
mod schedule_delivery;
mod schedule_materializer;
mod secrets;
mod security_state;
mod settings;
mod static_package;
mod trust;

use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use artifact::{
    ArtifactDataIndexField, ArtifactDataIndexValueType, ArtifactModuleKind, ArtifactOrigin,
    ArtifactPayloadKind, ArtifactPermissionDescriptor, ArtifactPersistenceContract,
    ArtifactRelease, ArtifactReleaseDraft, ArtifactReleaseRef, ArtifactSchemaDocument,
    ArtifactSourceLineage, ArtifactUiContribution, MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
    MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE, MODULE_ARTIFACT_RHAI_WORKSPACE_MEDIA_TYPE,
    MODULE_ARTIFACT_SIDECAR_MEDIA_TYPE, MODULE_ARTIFACT_STATIC_PROMOTION_MEDIA_TYPE,
    MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE, ModuleArtifactDescriptor, ModuleArtifactError,
    ModuleBindingIdempotency, ModuleDependencyConstraint, ModuleHttpBinding, ModuleHttpMethod,
    ModuleHttpStreamingPolicy, ModuleRuntimeBinding, ModuleRuntimeBindingKind,
    ModuleScheduleBinding, ModuleScheduleDeduplication, ModuleScheduleMisfirePolicy,
    ModuleScheduleOverlapPolicy, canonical_schema_digest, schedule_binding_digest,
};
pub use artifact_capability_router::{
    ArtifactCapabilityBrokerResolver, ArtifactCapabilityBrokerResolverRouter,
    ArtifactCapabilityExecution, ResolvingArtifactCapabilityBroker,
    resolve_granted_artifact_capability,
};
pub use artifact_cas::StorageArtifactBlobStore;
pub use binding_idempotency::{
    ArtifactBindingIdempotencyClaim, ArtifactBindingIdempotencyError,
    ArtifactBindingIdempotencyRequest, SeaOrmArtifactBindingIdempotencyStore,
    artifact_binding_request_digest,
};
pub use build::{
    MODULE_BUILD_PROTOCOL_VERSION, ModuleBuildAuthoring, ModuleBuildCompletedResult,
    ModuleBuildComponentInterface, ModuleBuildDependencyPolicy, ModuleBuildDiagnostic,
    ModuleBuildDiagnosticStage, ModuleBuildEvidence, ModuleBuildFailureCode, ModuleBuildLimits,
    ModuleBuildMetrics, ModuleBuildNetworkPolicy, ModuleBuildNextAction, ModuleBuildOutcome,
    ModuleBuildProtocolError, ModuleBuildPublicationReceipt, ModuleBuildRequest, ModuleBuildResult,
    ModuleBuildResultRecord, ModuleBuildSignatureAuthority, ModuleBuildSource,
    ModuleBuildSubmission, ModuleBuildToolchain, ModuleBuildValidationOutcome,
    ModuleBuildValidationProfile, ModuleBuildValidationResult, ModuleBuildWitContract,
    ModuleBuildWorker, ModuleBuildWorkerReadiness, SeaOrmModuleBuildService,
};
pub use build_surface::{
    PlatformAdminBuildSurfaceContract, PlatformBuildSurfaceContract,
    PlatformBuildSurfaceValidationError, PlatformStorefrontBuildSurfaceContract,
    validate_platform_build_surface_contract,
};
pub use composition::{
    ACTIVE_MODULE_COMPOSITION_ID, ModuleCompositionBuildEnqueuer, ModuleCompositionError,
    ModuleCompositionSnapshot, ModuleCompositionUpdate, SeaOrmModuleCompositionService,
};
pub use contracts::{
    ControlPlaneRevision, ModuleCommandContext, ModuleControlPlaneError,
    ModuleControlPlaneSnapshot, ModuleErrorCode, ModuleSnapshotKind, RevisionedModuleCommand,
};
pub use control_plane::{EffectivePolicyService, ModuleControlPlane};
pub use data::{
    ArtifactBindingDataUpgradeHook, ArtifactDataAccess, ArtifactDataAuthorizer,
    ArtifactDataBatchWrite, ArtifactDataBroker, ArtifactDataError, ArtifactDataExportAuthorizer,
    ArtifactDataExportRequest, ArtifactDataExportResult, ArtifactDataIndexQuery,
    ArtifactDataMigrationCheckpointStore, ArtifactDataObject, ArtifactDataObjectBroker,
    ArtifactDataObjectContent, ArtifactDataObjectGcResult, ArtifactDataObjectPage,
    ArtifactDataObjectRetentionPolicy, ArtifactDataObjectRetentionRule, ArtifactDataObjectUpload,
    ArtifactDataObjectUploadChunk, ArtifactDataObjectUploadCompleteRequest,
    ArtifactDataObjectUploadReapResult, ArtifactDataObjectUploadSession,
    ArtifactDataObjectUploadSessionRequest, ArtifactDataPage, ArtifactDataPageRequest,
    ArtifactDataPurgeAuthorizer, ArtifactDataPurgeRequest, ArtifactDataPurgeResult,
    ArtifactDataRecord, ArtifactDataSchemaValidator, ArtifactDataScope, ArtifactDataUpgradeApplier,
    ArtifactDataUpgradeApplyRequest, ArtifactDataUpgradeApplyResult, ArtifactDataUpgradeHook,
    ArtifactDataUpgradeInput, ArtifactDataUpgradePlan, ArtifactDataUpgradePlanner,
    ArtifactDataUpgradeRecord, ArtifactDataUpgradeRequest, ArtifactDataWrite,
    SeaOrmArtifactDataBroker, SeaOrmArtifactDataCapabilityBroker,
    SeaOrmArtifactDataCapabilityBrokerResolver, SeaOrmArtifactDataExportService,
    SeaOrmArtifactDataObjectBroker, SeaOrmArtifactDataObjectCapabilityBroker,
    SeaOrmArtifactDataObjectCapabilityBrokerResolver, SeaOrmArtifactDataObjectGcService,
    SeaOrmArtifactDataObjectUploadService, SeaOrmArtifactDataPurgeService,
    SeaOrmArtifactDataSchemaValidator, SnapshotArtifactDataObjectRetentionPolicy,
    validate_artifact_data_key, validate_artifact_data_prefix,
};
pub use data_snapshot::{
    ArtifactDataRestoreRequest, ArtifactDataRestoreResult, ArtifactDataSnapshot,
    ArtifactDataSnapshotAuthorizer, ArtifactDataSnapshotCollectionAuthorizer,
    ArtifactDataSnapshotCollectionCandidate, ArtifactDataSnapshotCollectionPolicy,
    ArtifactDataSnapshotCollectionRequest, ArtifactDataSnapshotCollectionResult,
    ArtifactDataSnapshotCollectionRule, ArtifactDataSnapshotCreateRequest,
    ArtifactDataSnapshotRetention, ArtifactDataSnapshotRetentionAuthorizer,
    ArtifactDataSnapshotRetentionUpdateRequest, SeaOrmArtifactDataSnapshotCollectionService,
    SeaOrmArtifactDataSnapshotRetentionService, SeaOrmArtifactDataSnapshotService,
    SnapshotArtifactDataSnapshotCollectionPolicy,
};
pub use definition::{
    ModuleDefinition, ModuleDefinitionCatalog, ModuleDefinitionError, ModuleDefinitionKind,
    ModuleDefinitionSource,
};
pub use dependency::{
    ModuleDependencyLockError, ModuleDependencyLockGraph, ModuleDependencyLockNode,
};
pub use dispatcher::{
    ARTIFACT_BINDING_DISPATCH_ENVELOPE_VERSION, ArtifactBindingDispatch,
    ArtifactBindingDispatchEnvelope, ArtifactBindingDispatchEnvelopeError,
    ArtifactBindingExecutionContext, ArtifactBindingExecutor, ArtifactInstallationTarget,
    ArtifactLifecycleExecutor, ModuleDispatchError, ModuleExecutionDispatcher,
    ModuleLifecycleHookPhase, dispatch_artifact_command_binding, dispatch_artifact_http_binding,
    find_artifact_command_binding, find_artifact_http_binding,
};
pub use distribution::{
    ModuleStaticDistributionAuthorizer, ModuleStaticDistributionBuild,
    ModuleStaticDistributionBuildCommand, ModuleStaticDistributionBuildEvidence,
    ModuleStaticDistributionBuildReceipt, ModuleStaticDistributionBuildStatus,
    ModuleStaticDistributionClaimCommand, ModuleStaticDistributionCompletionCommand,
    ModuleStaticDistributionCompletionOutcome, ModuleStaticDistributionCompletionReceipt,
    ModuleStaticDistributionError, ModuleStaticDistributionExecutor,
    ModuleStaticDistributionExecutorError, ModuleStaticDistributionExecutorMode,
    ModuleStaticDistributionExecutorReadiness, ModuleStaticDistributionFailure,
    ModuleStaticDistributionHeartbeatCommand, ModuleStaticDistributionHeartbeatReceipt,
    ModuleStaticDistributionItem, ModuleStaticDistributionSelection, ModuleStaticDistributionState,
    ModuleStaticDistributionWorkItem, ModuleStaticDistributionWorkerAuthorizer,
    SeaOrmModuleStaticDistributionService, SeaOrmModuleStaticDistributionWorkerService,
};
pub use distribution_release::{
    ModuleStaticDistributionActivationCommand, ModuleStaticDistributionActivationReceipt,
    ModuleStaticDistributionRelease, ModuleStaticDistributionReleaseAdmission,
    ModuleStaticDistributionReleaseAuthorizer, ModuleStaticDistributionReleaseError,
    ModuleStaticDistributionReleaseState, ModuleStaticDistributionReleaseStatus,
    ModuleStaticDistributionReleaseVerificationRequest, ModuleStaticDistributionReleaseVerifier,
    ModuleStaticDistributionRevocationCommand, ModuleStaticDistributionRevocationReceipt,
    ModuleStaticDistributionRollback, ModuleStaticDistributionRollbackCommand,
    ModuleStaticDistributionRollbackReceipt, ModuleStaticDistributionRollbackStatus,
    SeaOrmModuleStaticDistributionReleaseService,
};
pub use distribution_rollout::{
    ModuleStaticDistributionHealthEvidence, ModuleStaticDistributionNodeFailure,
    ModuleStaticDistributionNodePhase, ModuleStaticDistributionNodeReport,
    ModuleStaticDistributionNodeReportReceipt, ModuleStaticDistributionRollout,
    ModuleStaticDistributionRolloutAuthorizer, ModuleStaticDistributionRolloutError,
    ModuleStaticDistributionRolloutNode, ModuleStaticDistributionRolloutReceipt,
    ModuleStaticDistributionRolloutRequest, ModuleStaticDistributionRolloutState,
    ModuleStaticDistributionRolloutStatus, ModuleStaticDistributionTopologyResolver,
    ModuleStaticDistributionTopologySnapshot, SeaOrmModuleStaticDistributionRolloutService,
    module_static_distribution_topology_digest,
};
pub use event_delivery::{
    ARTIFACT_EVENT_DELIVERY_WORKER, ArtifactEventDeliveryCompletion, ArtifactEventDeliveryConfig,
    ArtifactEventDeliveryError, ArtifactEventDeliveryOutcome, ArtifactEventDeliveryReceipt,
    ArtifactEventDeliveryRequest, ArtifactEventDeliverySource, ArtifactEventDeliveryWorkAdapter,
    ArtifactEventDeliveryWorkItem, ArtifactEventDeliveryWorkRegistration,
    ArtifactEventProjectionTransport, SeaOrmArtifactEventDeliveryQueue,
    SeaOrmArtifactEventSubscriptionProjector,
};
pub use execution_audit::SeaOrmArtifactExecutionObserver;
pub use executor::{
    ModuleLifecycleExecutionError, ModuleLifecycleToggleRequest, ModuleLifecycleToggleResult,
    execute_module_toggle,
};
pub use governance::{
    ALLOY_PUBLICATION_SMOKE_TEST_PATH, ModuleAlloyAuthoredStageCommand,
    ModuleAlloyAuthoredStageResult, ModuleBuildServiceAttestationCommand,
    ModuleExternalPrebuiltStageCommand, ModuleExternalPrebuiltStageResult,
    ModuleExternalSourceEvidence, ModuleGovernanceAction, ModuleGovernanceError,
    ModuleGovernanceEventPayload, ModuleGovernanceEventSnapshot, ModuleGovernanceGateSnapshot,
    ModuleGovernanceLifecycleSnapshot, ModuleGovernanceModerationPolicy,
    ModuleGovernanceOwnerSnapshot, ModuleGovernanceOwnerTransition,
    ModuleGovernanceReleaseSnapshot, ModuleGovernanceRequestSnapshot,
    ModuleGovernanceValidationStageSnapshot, ModuleOwnerBindCommand, ModuleOwnerTransferCommand,
    ModulePlatformAdmissionCommand, ModulePublicationArtifactOrigin,
    ModulePublicationEvidenceAuthority, ModulePublicationEvidenceCommand,
    ModulePublicationEvidenceResult, ModulePublishApprovalOverride,
    ModulePublishArtifactAttachCommand, ModulePublishArtifactAttachResult,
    ModulePublishPlatformBuildStageCommand, ModulePublishPlatformBuildStageResult,
    ModulePublishRequestChangesCommand, ModulePublishRequestCreateCommand,
    ModulePublishRequestHoldCommand, ModulePublishRequestPublicationCommand,
    ModulePublishRequestRejectCommand, ModulePublishRequestResumeCommand,
    ModulePublishValidationContract, ModuleReleaseYankCommand, ModuleRemoteValidationClaim,
    ModuleRemoteValidationClaimCommand, ModuleRemoteValidationHeartbeatCommand,
    ModuleRemoteValidationTerminalCommand, ModuleRemoteValidationTerminalOutcome,
    ModuleValidationJobClaimCommand, ModuleValidationJobClaimResult,
    ModuleValidationJobEnqueueCommand, ModuleValidationJobEnqueueResult,
    ModuleValidationJobResultCommand, ModuleValidationJobResultOutcome,
    ModuleValidationJobRetryCommand, ModuleValidationJobWorkItem,
    ModuleValidationStageReportCommand, REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
    REGISTRY_EXTERNAL_SOURCE_ABSENCE_REASON_CODES, REGISTRY_HOLD_REASON_CODES,
    REGISTRY_OWNER_TRANSFER_REASON_CODES, REGISTRY_REJECT_REASON_CODES,
    REGISTRY_REQUEST_CHANGES_REASON_CODES, REGISTRY_RESUME_REASON_CODES,
    REGISTRY_VALIDATION_STAGE_REASON_CODES, REGISTRY_YANK_REASON_CODES,
    SeaOrmModuleGovernanceService,
};
pub use infrastructure::{ControlPlaneClock, ControlPlaneIdGenerator, ControlPlaneInfrastructure};
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
    ModuleLifecycleDbWriter, ModuleLifecycleDbWriterError, TenantModuleOverrideSnapshot,
};
pub use marketplace::{
    MODULE_MARKETPLACE_DEFAULT_LIMIT, MODULE_MARKETPLACE_MAX_LIMIT, ModuleMarketplaceCatalog,
    ModuleMarketplaceEntry, ModuleMarketplaceError, ModuleMarketplaceQuery,
    ModuleMarketplaceVersion, SharedModuleMarketplaceCatalog, normalize_module_marketplace_slug,
};
pub use marketplace_content::{
    MODULE_MARKETPLACE_CONTENT_FORMAT, MODULE_MARKETPLACE_CONTENT_TRUST,
    MODULE_MARKETPLACE_DESCRIPTION_MAX_CHARS, MODULE_MARKETPLACE_NAME_MAX_CHARS,
    ModuleMarketplaceContentError, ModuleMarketplaceContentProjection,
};
pub use mcp::{
    ArtifactMcpCallRequest, ArtifactMcpCapabilityBroker, ArtifactMcpCapabilityBrokerResolver,
    ArtifactMcpError, ArtifactMcpInvoker,
};
#[cfg(feature = "oci-distribution")]
pub use oci::{
    MODULE_ARTIFACT_DESCRIPTOR_MEDIA_TYPE, MODULE_ARTIFACT_PROVENANCE_MEDIA_TYPE,
    MODULE_ARTIFACT_RELEASE_LINEAGE_MEDIA_TYPE, MODULE_ARTIFACT_SBOM_MEDIA_TYPE,
    MODULE_ARTIFACT_TEST_EVIDENCE_MEDIA_TYPE, OCI_EMPTY_CONFIG_MEDIA_TYPE, OciArtifactEvidence,
    OciArtifactEvidenceKind, OciArtifactPublicationBundle, OciArtifactPublicationError,
    OciArtifactPublicationReceipt, OciArtifactPublicationTarget, OciArtifactPublisher,
    OciBuildPublicationArtifact, OciBuildPublicationBlob, OciDistributionArtifactPublisher,
    OciDistributionArtifactRegistry, OciRegistryProxyMode, OciRegistryTransportPolicy,
    strict_oci_distribution_client, strict_oci_distribution_client_with_policy,
};
pub use operation_store::{
    ModuleOperationJournal, ModuleOperationRecord, ModuleOperationRecordOutcome,
    ModuleOperationRequest, ModuleOperationSnapshot, ModuleOperationStoreError,
    TenantModuleSettingsRecord, TenantModuleStateRecord,
};
pub(crate) use operation_store::{
    TenantModuleSettingsRequest, TenantModuleStateRequest, TenantModuleStateStore,
};
pub use policy::{
    ModuleEffectivePolicy, ModuleEffectivePolicyChannelBinding, ModuleEffectivePolicyChannelInput,
    ModuleEffectivePolicyDecision, ModuleEffectivePolicyDenialReason, ModuleEffectivePolicyError,
    ModuleEffectivePolicyFact, ModuleEffectivePolicyMaintenanceInput,
    ModuleEffectivePolicyNodeReadinessInput, ModulePolicyRevisionApplyOutcome,
    ModulePolicyRevisionGate, ModulePolicyRevisionGateError, ModulePolicyRevisionTransition,
    ModuleToggleValidationError, TenantModuleOverride, validate_module_toggle,
};
pub use policy_revision_consumer::{
    ModulePolicyRevisionConsumerError, SeaOrmModulePolicyRevisionConsumer,
};
pub use policy_transition_event::{
    ModuleEffectivePolicyTransitionCoordinator, ModuleEffectivePolicyTransitionCoordinatorError,
    ModuleEffectivePolicyTransitionPublisher, ModuleEffectivePolicyTransitionPublisherError,
};
pub use promotion::{
    ModuleStaticPromotion, ModuleStaticPromotionApprovalCommand,
    ModuleStaticPromotionApprovalEvidence, ModuleStaticPromotionAuthorizer,
    ModuleStaticPromotionError, ModuleStaticPromotionEvidence, ModuleStaticPromotionReceipt,
    ModuleStaticPromotionRequestCommand, ModuleStaticPromotionStatus, SeaOrmModulePromotionService,
};
pub use publish_validation::{
    MODULE_PUBLISH_ALLOY_WORKSPACE_MAX_BYTES, MODULE_PUBLISH_ARTIFACT_MANIFEST_MAX_BYTES,
    MODULE_PUBLISH_ARTIFACT_MAX_BYTES, MODULE_PUBLISH_BUNDLE_TYPE, ModulePublishBundleValidation,
    validate_module_publish_artifact, validate_module_publish_bundle,
};
pub use recovery::{
    ModuleOperationRecoveryError, ModuleOperationRecoveryPlan, ModulePostHookRetryRequest,
    failed_module_operation_recovery_plans, module_operation_recovery_plan,
    retry_failed_post_hook_operation,
};
pub use resolution::{
    ModuleResolutionCandidate, ModuleResolutionConflict, ModuleResolutionError,
    ModuleResolutionProvider, ModuleResolutionProviderKind, ModuleResolutionRequest,
    ModuleResolutionResult, ModuleResolutionScope, resolve_module_dependencies,
};
pub use runtime::{
    ArtifactEffectivePolicyResolver, ArtifactInstallationResolver, ArtifactRuntime,
    ArtifactRuntimeError, ArtifactRuntimeLifecycleExecutor, ArtifactSandboxPolicyResolver,
    VerifiedArtifactNodeCache,
};
pub use runtime_handles::{
    ArtifactDeliveryTenantSource, SharedArtifactBindingExecutor, SharedArtifactDeliveryTenantSource,
};
pub use schedule_delivery::{
    ARTIFACT_SCHEDULE_DELIVERY_WORKER, ArtifactScheduleDeliveryConfig,
    ArtifactScheduleDeliveryError, ArtifactScheduleDeliveryOutcome,
    ArtifactScheduleDeliveryReceipt, ArtifactScheduleDeliveryRequest,
    ArtifactScheduleDeliveryWorkAdapter, ArtifactScheduleDeliveryWorkItem,
    ArtifactScheduleDeliveryWorkRegistration, SeaOrmArtifactScheduleDeliveryQueue,
};
pub use schedule_materializer::{
    ArtifactScheduleMaterializationConfig, ArtifactScheduleMaterializationError,
    ArtifactScheduleMaterializationReport, ArtifactScheduleMaterializer,
};
pub use secrets::{
    ArtifactSecretAuthorizer, ArtifactSecretBindingRequest, ArtifactSecretConsumerError,
    ArtifactSecretError, ArtifactSecretHandle, ArtifactSecretHandleAuthorizer,
    ArtifactSecretHandleRequest, ArtifactSecretPolicy, ArtifactSecretUseAuthorizer,
    ArtifactSecretUseContext, ArtifactSecretUseReceipt, ArtifactSecretUseRequest,
    ArtifactSecretValueConsumer, RegistryArtifactSecretAuthorizer,
    SeaOrmArtifactSecretCapabilityBroker, SeaOrmArtifactSecretCapabilityBrokerResolver,
    SeaOrmArtifactSecretHandleService, SeaOrmArtifactSecretService, SeaOrmArtifactSecretUseService,
};
pub use security_state::{
    ModuleArtifactRegistryReleaseStatus, ModuleArtifactSecurityAuthorizer,
    ModuleArtifactSecurityCommand, ModuleArtifactSecurityError, ModuleArtifactSecurityReceipt,
    ModuleArtifactSecuritySnapshot, ModuleArtifactSecurityStatus,
    SeaOrmModuleArtifactSecurityResolver, SeaOrmModuleArtifactSecurityService,
};
pub use settings::{
    ModuleSettingSpec, ModuleSettingsValidationError, normalize_module_settings,
    validate_module_settings_schema,
};
pub use static_package::{
    StaticModuleCatalogContract, StaticModuleCatalogValidationError,
    StaticModuleEntrypointContract, StaticModuleEntrypointValidationError, StaticModuleEntrypoints,
    StaticModuleHttpProvidesContract, StaticModuleHttpProvidesValidationError,
    StaticModulePackageContract, StaticModulePackageValidationError,
    StaticModulePlatformVersionError, StaticModuleTopologyContract, StaticModuleTopologyModule,
    StaticModuleTopologyValidationError, StaticModuleUiClassificationError,
    StaticModuleUiI18nContract, StaticModuleUiI18nResolved, StaticModuleUiI18nValidationError,
    is_valid_static_module_slug, resolve_static_module_entrypoints,
    resolve_static_module_ui_classification, static_module_platform_version_is_compatible,
    validate_static_module_catalog_contract, validate_static_module_http_provides_contract,
    validate_static_module_package_contract, validate_static_module_registry_contracts,
    validate_static_module_topology_contract, validate_static_module_ui_i18n_contract,
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

    fn register_runtime_extensions(
        &self,
        extensions: &mut rustok_core::ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        let registrations = extensions
            .get_or_insert_with::<rustok_runtime::ModuleWorkRegistrations, _>(Default::default);
        registrations.register(std::sync::Arc::new(
            ArtifactEventDeliveryWorkRegistration::default(),
        ));
        registrations.register(std::sync::Arc::new(
            ArtifactScheduleDeliveryWorkRegistration::default(),
        ));
        Ok(())
    }
}
