//! Module platform ownership: artifact identity, release lineage and lifecycle.

mod artifact;
mod artifact_capability_router;
mod artifact_cas;
mod artifact_node_reconciliation;
mod artifact_schema;
mod artifact_settings;
mod artifact_settings_recovery;
mod authoring;
mod binding_idempotency;
mod build;
mod build_surface;
mod capability_events;
mod capability_http;
mod composition;
mod conflict_fences;
mod contracts;
mod control_plane;
mod data;
pub mod data_copier;
pub mod data_object_migration;
pub mod data_post_purge_recovery;
pub mod data_snapshot_intents;
pub mod data_snapshot_readiness;
pub mod data_upgrade;
mod data_snapshot;
mod definition;
mod dependency;
mod dispatcher;
mod distribution;
mod distribution_bootstrap;
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
mod migration_preflight;
pub mod migrations;
#[cfg(feature = "oci-distribution")]
mod oci;
#[cfg(feature = "oci-distribution")]
mod oci_transport;
mod operation_store;
mod policy;
mod policy_cache;
mod policy_revision_consumer;
mod policy_transition_event;
mod promotion;
mod publication_evidence;
mod publish_validation;
pub mod queue_drain;
mod reconciliation;
mod recovery;
mod release_admission_journal;
mod release_preparation;
pub mod rhai_authoring;
mod resolution;
mod retention;
mod runtime;
mod runtime_handles;
mod schedule_delivery;
mod schedule_materializer;
mod secrets;
mod security_epoch;
mod security_state;
mod settings;
mod settings_guard;
mod static_package;
mod static_settings_localization;
mod static_settings_source_locale;
pub mod static_settings_translation_read;
mod transition_coordinator;
mod transition_receipts;
mod transition_store;
mod trust;

pub use conflict_fences::{ConflictFenceSet, ConflictKey, ConflictKeyKind};
pub use data_copier::{
    ArtifactDataCopyError, ArtifactDataCrossRevisionCopier, CrossRevisionDataCopyReceipt,
    CrossRevisionDataCopyRequest,
};
pub use data_object_migration::{
    ArtifactDataObjectMigrationError, ArtifactDataObjectMigrationReceipt,
    ArtifactDataObjectMigrationRequest, ArtifactDataObjectMigrationService,
};
pub use data_post_purge_recovery::{
    ArtifactDataPostPurgeRecoveryService, PostPurgeRecoveryCutoverReceipt, PostPurgeRecoveryError,
    PrepareRecoveryRequest, StagedRecoveryReceipt,
};
pub use data_snapshot_intents::{
    ArtifactDataSnapshotIntentService, ReconciledSnapshotIntentsReceipt, SnapshotCopyIntent,
    SnapshotCopyKind, SnapshotIntentError,
};
pub use data_snapshot_readiness::{
    ArtifactDataRecoveryReadinessAttestation, ArtifactDataRecoveryReadinessService,
    ArtifactDataSnapshotReadiness, PlatformPostgresRecoveryEvidence, PostgresRecoveryEvidenceError,
    RecoveryReadinessError, SnapshotReadinessError,
};
pub use data_upgrade::{
    DataUpgradeDecision, DataUpgradeEvidence, DataUpgradePhase, evaluate_data_upgrade_decision,
};
pub use migration_preflight::{
    MigrationPreflightInput, MigrationPreflightReceipt, UpdateMode, evaluate_migration_preflight,
};
pub use queue_drain::{
    ArtifactQueueDrainError, ArtifactQueueDrainReceipt, ArtifactQueueDrainRequest,
    ArtifactQueueDrainService,
};
pub use release_admission_journal::{
    ReleaseAdmissionIntentJournal, ReleaseAdmissionIntentRecord, ReleaseAdmissionJournalError,
};
pub use rhai_authoring::{
    RhaiAuthoringError, RhaiAuthoringPackageCommand, RhaiAuthoringPublishableRelease,
    RhaiAuthoringService, RhaiOciPayload, RhaiSourceCasReceipt,
};
pub use retention::{
    RetentionError, RetentionHoldKind, RetentionHoldLedger, RetentionHoldRecord, RetentionTarget,
};
pub use security_epoch::{
    GlobalSecurityEpoch, SecurityEpochConflictError, SecurityEpochRecord, SecurityEpochRegistry,
};
pub use settings_guard::{
    SettingsCompatibilityGuard, SettingsGuardError, SettingsGuardState,
    validate_settings_intersection,
};
pub use transition_coordinator::{
    ModuleTransitionCheckpoint, ModuleTransitionCoordinator, ModuleTransitionFinalizeCommand,
    ModuleTransitionRecoveryCommand, ModuleTransitionState, StartTransitionInput,
    TransitionCoordinatorError, evaluate_transition_watchdog,
};
pub use transition_receipts::{
    TransitionApplyReceipt, TransitionCancellationReceipt, TransitionConfirmationReceipt,
    TransitionPreviewReceipt, TransitionReceiptError, TransitionRollbackReceipt,
};
pub use transition_store::{RetentionHoldStore, TransitionCheckpointStore, TransitionStoreError};

use async_trait::async_trait;
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use artifact::{
    ArtifactDataIndexField, ArtifactDataIndexValueType, ArtifactLocalizationCatalog,
    ArtifactModuleKind, ArtifactOrigin, ArtifactPayloadKind, ArtifactPermissionDescriptor,
    ArtifactPersistenceContract, ArtifactRelease, ArtifactReleaseDraft, ArtifactReleaseRef,
    ArtifactSchemaDocument, ArtifactSourceLineage, ArtifactUiAuditPolicy, ArtifactUiContribution,
    ArtifactUiContributionContent, ArtifactUiProjectionError,
    MAX_MODULE_ARTIFACT_SOURCE_MANIFEST_BYTES, MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
    MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE, MODULE_ARTIFACT_SIDECAR_MEDIA_TYPE,
    MODULE_ARTIFACT_SOURCE_MANIFEST_FILE, MODULE_ARTIFACT_STATIC_PROMOTION_MEDIA_TYPE,
    MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE, ModuleArtifactDescriptor, ModuleArtifactError,
    ModuleArtifactSourceManifest, ModuleArtifactSourceManifestError, ModuleBindingIdempotency,
    ModuleDependencyConstraint, ModuleHttpBinding, ModuleHttpMethod, ModuleHttpStreamingPolicy,
    ModuleRuntimeBinding, ModuleRuntimeBindingKind, ModuleScheduleBinding,
    ModuleScheduleDeduplication, ModuleScheduleMisfirePolicy, ModuleScheduleOverlapPolicy,
    canonical_artifact_descriptor_digest, canonical_schema_digest, schedule_binding_digest,
};
pub use artifact_capability_router::{
    ArtifactCapabilityBrokerResolver, ArtifactCapabilityBrokerResolverRouter,
    ArtifactCapabilityExecution, ResolvingArtifactCapabilityBroker,
    resolve_granted_artifact_capability,
};
pub use artifact_cas::StorageArtifactBlobStore;
pub use artifact_node_reconciliation::{
    MODULE_ARTIFACT_NODE_ASSIGNMENT_LEASE_SECONDS, ModuleArtifactNodeAgentPort,
    ModuleArtifactNodeAssignment, ModuleArtifactNodeAssignmentClaimCommand,
    ModuleArtifactNodeAssignmentHeartbeatCommand, ModuleArtifactNodeAssignmentHeartbeatReceipt,
    ModuleArtifactNodeAssignmentReport, ModuleArtifactNodeAssignmentReportReceipt,
    ModuleArtifactNodeAssignmentTarget, ModuleArtifactNodeAssignmentWorkItem,
    ModuleArtifactNodeInstallationScope, ModuleArtifactNodeReconciliation,
    ModuleArtifactNodeReconciliationAuthorizer, ModuleArtifactNodeReconciliationError,
    ModuleArtifactNodeReconciliationReceipt, ModuleArtifactNodeReconciliationRequest,
    ModuleArtifactNodeReconciliationStatus, ModuleArtifactNodeReconciliationWorkIdentity,
    ModuleArtifactNodeTopologyResolver, ModuleArtifactNodeTopologySnapshot,
    SeaOrmArtifactNodeReadiness, SeaOrmModuleArtifactNodeAgentService,
    SeaOrmModuleArtifactNodeReconciliationService, module_artifact_node_topology_digest,
};
pub use artifact_settings_recovery::{
    ArtifactSettingsPurgeRequest, ArtifactSettingsPurgeResult,
    ArtifactSettingsRecoveryAuthorizationContext, ArtifactSettingsRecoveryAuthorizer,
    ArtifactSettingsRecoveryBindRequest, ArtifactSettingsRecoveryBindResult,
    ArtifactSettingsRecoveryCipher, ArtifactSettingsRecoveryCipherContext,
    ArtifactSettingsRecoveryCiphertext, ArtifactSettingsRecoveryCollectionCandidate,
    ArtifactSettingsRecoveryCollectionPolicy, ArtifactSettingsRecoveryCollectionRequest,
    ArtifactSettingsRecoveryCollectionResult, ArtifactSettingsRecoveryError,
    ArtifactSettingsRecoveryPoint, ArtifactSettingsRecoveryPointCreateRequest,
    ArtifactSettingsRecoveryRetention, ArtifactSettingsRecoveryRetentionUpdate,
    ArtifactSettingsRecoveryRetentionUpdateRequest, ArtifactSettingsRecoveryRetentionUpdateResult,
    ArtifactSettingsRecoveryRewrapRequest, ArtifactSettingsRecoveryRewrapResult,
    ArtifactSettingsRestoreRequest, ArtifactSettingsRestoreResult,
    SeaOrmArtifactSettingsRecoveryService,
};
pub use authoring::{
    MODULE_AUTHORING_BUILD_MAX_ARCHIVE_BYTES, MODULE_AUTHORING_BUILD_MAX_SOURCE_BYTES,
    MODULE_AUTHORING_BUILD_MAX_SOURCE_ENTRIES, ModuleAuthoringBuildCommand,
    ModuleAuthoringBuildControl, ModuleAuthoringBuildError, ModuleAuthoringBuildSubmission,
    ModuleAuthoringPublishCommand, ModuleAuthoringPublishControl, ModuleAuthoringPublishError,
    ModuleAuthoringPublishSubmission, ModuleAuthoringSourceArchiveBuilder,
    PreparedModuleSourceArchive, SeaOrmModuleAuthoringBuildService,
    SeaOrmModuleAuthoringPublishService, SharedModuleAuthoringBuildControl,
    SharedModuleAuthoringPublishControl,
};
pub use binding_idempotency::{
    ArtifactBindingIdempotencyClaim, ArtifactBindingIdempotencyError,
    ArtifactBindingIdempotencyRequest, SeaOrmArtifactBindingIdempotencyStore,
    artifact_binding_request_digest,
};
pub use build::{
    MODULE_BUILD_COMPONENT_TARGET, MODULE_BUILD_PROTOCOL_VERSION, MODULE_BUILD_RUNTIME_ABI,
    MODULE_BUILD_WIT_VERSION, MODULE_BUILD_WIT_WORLD, ModuleBuildAuthoring,
    ModuleBuildClaimedRequest, ModuleBuildCompletedResult, ModuleBuildComponentInterface,
    ModuleBuildDependencyPolicy, ModuleBuildDiagnostic, ModuleBuildDiagnosticStage,
    ModuleBuildEvidence, ModuleBuildExecutionClaim, ModuleBuildFailureCode, ModuleBuildLimits,
    ModuleBuildMetrics, ModuleBuildNetworkPolicy, ModuleBuildNextAction, ModuleBuildOutcome,
    ModuleBuildProtocolError, ModuleBuildPublicationReceipt, ModuleBuildRequest, ModuleBuildResult,
    ModuleBuildResultRecord, ModuleBuildScenario, ModuleBuildSignatureAuthority, ModuleBuildSource,
    ModuleBuildSubmission, ModuleBuildToolchain, ModuleBuildValidationOutcome,
    ModuleBuildValidationProfile, ModuleBuildValidationResult, ModuleBuildWitContract,
    ModuleBuildWorker, ModuleBuildWorkerReadiness, SeaOrmModuleBuildService,
};
pub use build_surface::{
    PlatformAdminBuildSurfaceContract, PlatformBuildSurfaceContract,
    PlatformBuildSurfaceValidationError, PlatformStorefrontBuildSurfaceContract,
    validate_platform_build_surface_contract,
};
pub use capability_events::{
    ArtifactEventCapabilityBroker, SeaOrmArtifactEventCapabilityBrokerResolver,
};
pub use capability_http::{
    ArtifactHttpCapabilityBroker, SeaOrmArtifactHttpCapabilityBrokerResolver,
};
pub use composition::{
    ACTIVE_MODULE_COMPOSITION_ID, ModuleCompositionBuildAdmission,
    ModuleCompositionBuildEnqueueResult, ModuleCompositionBuildEnqueuer,
    ModuleCompositionBuildLease, ModuleCompositionBuildReceipt, ModuleCompositionError,
    ModuleCompositionOperation, ModuleCompositionSnapshot, ModuleCompositionUpdate,
    SeaOrmModuleCompositionService,
};
pub use contracts::{
    ControlPlaneRevision, ModuleCommandContext, ModuleControlPlaneError,
    ModuleControlPlaneSnapshot, ModuleErrorCode, ModuleSnapshotKind, RevisionedModuleCommand,
};
pub use control_plane::{EffectivePolicyService, ModuleControlPlane};
pub use data::{
    ArtifactBindingDataUpgradeHook, ArtifactDataAccess, ArtifactDataAuthorizer,
    ArtifactDataBatchWrite, ArtifactDataBroker, ArtifactDataDeleteRequest,
    ArtifactDataDeleteResult, ArtifactDataError, ArtifactDataExportAuthorizer,
    ArtifactDataExportRequest, ArtifactDataExportResult, ArtifactDataIndexQuery,
    ArtifactDataMigrationCheckpointStore, ArtifactDataObject, ArtifactDataObjectBroker,
    ArtifactDataObjectContent, ArtifactDataObjectDeleteRequest, ArtifactDataObjectDeleteResult,
    ArtifactDataObjectGcResult, ArtifactDataObjectPage, ArtifactDataObjectRetentionPolicy,
    ArtifactDataObjectRetentionRule, ArtifactDataObjectUpload, ArtifactDataObjectUploadChunk,
    ArtifactDataObjectUploadCompleteRequest, ArtifactDataObjectUploadReapResult,
    ArtifactDataObjectUploadSession, ArtifactDataObjectUploadSessionRequest, ArtifactDataPage,
    ArtifactDataPageRequest, ArtifactDataPurgeAuthorizer, ArtifactDataPurgeRequest,
    ArtifactDataPurgeResult, ArtifactDataQuota, ArtifactDataQuotaPolicy, ArtifactDataRecord,
    ArtifactDataSchemaValidator, ArtifactDataScope, ArtifactDataUpgradeApplier,
    ArtifactDataUpgradeApplyRequest, ArtifactDataUpgradeApplyResult, ArtifactDataUpgradeHook,
    ArtifactDataUpgradeInput, ArtifactDataUpgradePlan, ArtifactDataUpgradePlanner,
    ArtifactDataUpgradeRecord, ArtifactDataUpgradeRequest, ArtifactDataWrite,
    FixedArtifactDataQuotaPolicy, SeaOrmArtifactDataBroker, SeaOrmArtifactDataCapabilityBroker,
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
    ArtifactBindingExecutionContext, ArtifactBindingExecutor, ArtifactCommandBindingRequest,
    ArtifactHttpBindingRequest, ArtifactInstallationTarget, ArtifactLifecycleExecutor,
    ModuleDispatchError, ModuleExecutionDispatcher, ModuleLifecycleHookPhase,
    dispatch_artifact_command_binding, dispatch_artifact_http_binding,
    find_artifact_command_binding, find_artifact_http_binding, find_artifact_ui_action_binding,
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
    ModuleStaticDistributionItem, ModuleStaticDistributionPreparationSource,
    ModuleStaticDistributionRole, ModuleStaticDistributionRoleArtifact,
    ModuleStaticDistributionSelection, ModuleStaticDistributionState,
    ModuleStaticDistributionWorkItem, ModuleStaticDistributionWorkerAuthorizer,
    SeaOrmModuleStaticDistributionService, SeaOrmModuleStaticDistributionWorkerService,
    module_static_distribution_composition_digest,
};
pub use distribution_bootstrap::{
    MODULE_STATIC_DISTRIBUTION_BOOTSTRAP_RECEIPT_CONTRACT,
    ModuleStaticDistributionBootstrapImportCommand, ModuleStaticDistributionBootstrapImportReceipt,
    ModuleStaticDistributionBootstrapPreparation, ModuleStaticDistributionBootstrapReceipt,
    ModuleStaticDistributionBootstrapReceiptError, ModuleStaticDistributionBootstrapReceiptPayload,
    SeaOrmModuleStaticDistributionBootstrapService,
    VerifiedModuleStaticDistributionBootstrapReceipt,
};
pub use distribution_release::{
    ModuleStaticDistributionAdmissionCommand, ModuleStaticDistributionAdmissionReceipt,
    ModuleStaticDistributionInstallBinding, ModuleStaticDistributionRelease,
    ModuleStaticDistributionReleaseAdmission, ModuleStaticDistributionReleaseAuthorizer,
    ModuleStaticDistributionReleaseError, ModuleStaticDistributionReleaseState,
    ModuleStaticDistributionReleaseStatus, ModuleStaticDistributionReleaseVerificationRequest,
    ModuleStaticDistributionReleaseVerifier, ModuleStaticDistributionRevocationCommand,
    ModuleStaticDistributionRevocationReceipt, SeaOrmModuleStaticDistributionReleaseService,
    resolve_static_distribution_install_binding,
};
pub use distribution_rollout::{
    ModuleStaticDistributionAssignment, ModuleStaticDistributionAssignmentClaimCommand,
    ModuleStaticDistributionAssignmentHeartbeatCommand,
    ModuleStaticDistributionAssignmentHeartbeatReceipt, ModuleStaticDistributionAssignmentReport,
    ModuleStaticDistributionAssignmentReportReceipt, ModuleStaticDistributionAssignmentWorkItem,
    ModuleStaticDistributionRecoveryRequest, ModuleStaticDistributionRollout,
    ModuleStaticDistributionRolloutAssignment, ModuleStaticDistributionRolloutAuthorizer,
    ModuleStaticDistributionRolloutError, ModuleStaticDistributionRolloutReceipt,
    ModuleStaticDistributionRolloutRequest, ModuleStaticDistributionRolloutStatus,
    ModuleStaticDistributionRolloutWorkIdentity, ModuleStaticDistributionTopologyResolver,
    ModuleStaticDistributionTopologySnapshot, ModuleStaticDistributionTransitionKind,
    SeaOrmModuleStaticDistributionRolloutService, module_static_distribution_topology_digest,
};
pub use event_delivery::{
    ARTIFACT_EVENT_DELIVERY_WORKER, ArtifactEventDeliveryCompletion, ArtifactEventDeliveryConfig,
    ArtifactEventDeliveryError, ArtifactEventDeliveryOutcome, ArtifactEventDeliveryReceipt,
    ArtifactEventDeliveryRequest, ArtifactEventDeliverySource, ArtifactEventDeliveryWorkAdapter,
    ArtifactEventDeliveryWorkItem, ArtifactEventDeliveryWorkRegistration,
    ArtifactEventProjectionTransport, SeaOrmArtifactEventDeliveryQueue,
    SeaOrmArtifactEventSubscriptionProjector,
};
pub use execution_audit::{
    ArtifactBindingExecutionAuditError, SeaOrmArtifactBindingExecutionAuditReader,
    SeaOrmArtifactExecutionObserver,
};
pub use executor::{
    ModuleLifecycleExecutionError, ModuleLifecycleToggleRequest, ModuleLifecycleToggleResult,
    execute_module_toggle,
};
pub use governance::{
    ALLOY_PUBLICATION_SMOKE_TEST_PATH, ModuleAlloyAuthoredStageCommand,
    ModuleAlloyAuthoredStageResult, ModuleAuthorSignatureEvidenceCommand,
    ModuleBuildServiceAttestationCommand, ModuleExternalPrebuiltStageCommand,
    ModuleExternalPrebuiltStageResult, ModuleExternalSourceEvidence, ModuleGovernanceAction,
    ModuleGovernanceActorContext, ModuleGovernanceError, ModuleGovernanceErrorCategory,
    ModuleGovernanceEventPayload, ModuleGovernanceEventSnapshot, ModuleGovernanceGateSnapshot,
    ModuleGovernanceLifecycleSnapshot, ModuleGovernanceModerationPolicy,
    ModuleGovernanceOwnerSnapshot, ModuleGovernanceOwnerTransition,
    ModuleGovernancePublishArtifactDownloadSnapshot, ModuleGovernancePublishArtifactUploadSlot,
    ModuleGovernancePublishRequestNextAction, ModuleGovernancePublishRequestStatusSnapshot,
    ModuleGovernanceReleaseSnapshot, ModuleGovernanceRequestAuthorizationSnapshot,
    ModuleGovernanceRequestSnapshot, ModuleGovernanceValidationStageSnapshot,
    ModuleOwnerTransferCommand, ModulePlatformAdmissionCommand, ModulePlatformPublicationSource,
    ModulePublicationArtifactOrigin, ModulePublicationEvidenceResult,
    ModulePublishApprovalOverride, ModulePublishArtifactAttachCommand,
    ModulePublishArtifactAttachResult, ModulePublishPlatformBuildStageCommand,
    ModulePublishPlatformBuildStageResult, ModulePublishRequestChangesCommand,
    ModulePublishRequestCreateCommand, ModulePublishRequestHoldCommand,
    ModulePublishRequestPublicationCommand, ModulePublishRequestRejectCommand,
    ModulePublishRequestResumeCommand, ModulePublishValidationContract,
    ModulePublishedArtifactContract, ModulePublishedRhaiWorkspace, ModuleReleaseYankCommand,
    ModuleReleaseYankResult, ModuleRemoteValidationClaim, ModuleRemoteValidationClaimCommand,
    ModuleRemoteValidationHeartbeatCommand, ModuleRemoteValidationRunnerSnapshot,
    ModuleRemoteValidationStageTransition, ModuleRemoteValidationTerminalCommand,
    ModuleRemoteValidationTerminalOutcome, ModuleValidationJobClaimCommand,
    ModuleValidationJobClaimResult, ModuleValidationJobEnqueueCommand,
    ModuleValidationJobEnqueueResult, ModuleValidationJobResultCommand,
    ModuleValidationJobResultOutcome, ModuleValidationJobRetryCommand, ModuleValidationJobWorkItem,
    ModuleValidationStageReportCommand, REGISTRY_APPROVE_OVERRIDE_REASON_CODES,
    REGISTRY_EXTERNAL_SOURCE_ABSENCE_REASON_CODES, REGISTRY_HOLD_REASON_CODES,
    REGISTRY_OWNER_TRANSFER_REASON_CODES, REGISTRY_REJECT_REASON_CODES,
    REGISTRY_REQUEST_CHANGES_REASON_CODES, REGISTRY_RESUME_REASON_CODES,
    REGISTRY_VALIDATION_STAGE_REASON_CODES, REGISTRY_YANK_REASON_CODES,
    SeaOrmModuleGovernanceService, alloy_publication_smoke_scenario_digest,
};
pub use infrastructure::{ControlPlaneClock, ControlPlaneIdGenerator, ControlPlaneInfrastructure};
pub use installation::{
    ArtifactActivationRequest, ArtifactActivationResult, ArtifactAdmissionCommand,
    ArtifactAdmissionLimits, ArtifactAdmissionReconciler, ArtifactAdmissionRecoveryRecord,
    ArtifactAdmissionResult, ArtifactAdmissionReverification, ArtifactAdmissionService,
    ArtifactAdmissionStage, ArtifactAdmissionStatus, ArtifactAdmissionStore,
    ArtifactBlobRetentionPolicy, ArtifactBlobRetentionRule, ArtifactBlobStore,
    ArtifactDeactivationRequest, ArtifactDeactivationResult, ArtifactMigrationCheckpointRequest,
    ArtifactMigrationRollbackMode, ArtifactPayloadSource, ArtifactRegistry,
    ArtifactRollbackRequest, ArtifactRollbackResult, ArtifactTenantDisableRequest,
    ArtifactTenantDisableResult, ArtifactTenantEnableRequest, ArtifactTenantEnableResult,
    ArtifactTenantLifecycleSnapshot, ArtifactUninstallRequest, ArtifactUninstallResult,
    ArtifactVerificationEvidence, DurableArtifactBlobStore, InMemoryArtifactBlobStore,
    InstalledModuleArtifact, ModuleArtifactPackage, ModuleInstallationError,
    ModuleInstallationScope, ModuleInstaller, OciArtifactReference,
    SeaOrmArtifactInstallationStore, SeaOrmArtifactSandboxPolicyResolver,
    SnapshotArtifactBlobRetentionPolicy, StagedArtifactBlob,
};
pub use lifecycle::{ModuleOperationIssue, ModuleOperationRecoveryAction, ModuleOperationStatus};
pub use lifecycle_writer::{
    ModuleLifecycleDbWriter, ModuleLifecycleDbWriterError, ModuleLifecycleRecoveryCommand,
    ModuleLifecycleSettingsCommand, ModuleLifecycleSettingsResult, ModuleLifecycleToggleCommand,
    TenantModuleOverrideSnapshot,
};
pub use marketplace::{
    MODULE_MARKETPLACE_DEFAULT_LIMIT, MODULE_MARKETPLACE_MAX_LIMIT,
    ModuleMarketplaceArtifactOrigin, ModuleMarketplaceArtifactRelease,
    ModuleMarketplaceArtifactReleaseError, ModuleMarketplaceCatalog, ModuleMarketplaceEntry,
    ModuleMarketplaceError, ModuleMarketplaceEvidenceKind, ModuleMarketplaceEvidenceReference,
    ModuleMarketplaceQuery, ModuleMarketplaceRuntimeKind, ModuleMarketplaceVersion,
    SharedModuleMarketplaceCatalog, normalize_module_marketplace_slug,
    normalize_module_registry_id,
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
};
pub use operation_store::{
    ModuleOperationJournal, ModuleOperationRecord, ModuleOperationRecordOutcome,
    ModuleOperationRequest, ModuleOperationSnapshot, ModuleOperationStoreError,
    StaticTenantLifecycleClaim, StaticTenantLifecycleSnapshot, StaticTenantLifecycleStore,
    StaticTenantLifecycleStoreError, TenantModuleSettingsRecord, TenantModuleStateRecord,
};
pub(crate) use operation_store::{
    TenantModuleSettingsRequest, TenantModuleStateRequest, TenantModuleStateStore,
};
pub use policy::{
    EffectivePolicyCacheIdentity, ModuleEffectivePolicy, ModuleEffectivePolicyChannelBinding,
    ModuleEffectivePolicyChannelInput, ModuleEffectivePolicyDecision,
    ModuleEffectivePolicyDenialReason, ModuleEffectivePolicyError, ModuleEffectivePolicyFact,
    ModuleEffectivePolicyMaintenanceInput, ModulePolicyRevisionApplyOutcome,
    ModulePolicyRevisionGate, ModulePolicyRevisionGateError, ModulePolicyRevisionTransition,
    ModuleToggleValidationError, TenantModuleOverride, validate_module_toggle,
};
pub use policy_cache::ModuleEffectivePolicyCache;
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
pub use publication_evidence::{
    ModulePlatformPublicationEvidenceCommand, ModulePlatformPublicationEvidenceError,
    ModulePlatformPublicationEvidenceOwner, ModulePlatformPublicationEvidenceProducer,
    ModulePlatformPublicationEvidenceResult, ModulePublicationArtifactRegistryProvider,
};
pub use publish_validation::{
    MODULE_PUBLISH_ALLOY_WORKSPACE_MAX_BYTES, MODULE_PUBLISH_ARTIFACT_MANIFEST_MAX_BYTES,
    MODULE_PUBLISH_ARTIFACT_MAX_BYTES, MODULE_PUBLISH_BUNDLE_CONTENT_TYPE,
    MODULE_PUBLISH_BUNDLE_TYPE, ModulePublishBundleFiles, ModulePublishBundleValidation,
    build_module_publish_bundle, validate_module_publish_artifact, validate_module_publish_bundle,
};
pub use reconciliation::{
    ModuleDesiredObservedState, ModuleReconciliationEvidence, ModuleReconciliationFailure,
    ModuleReconciliationPhase,
};
pub use recovery::{ModuleOperationRecoveryError, ModuleOperationRecoveryPlan};
pub use release_preparation::{
    ReleasePreparation, ReleasePreparationError, ReleasePreparationState,
    SanitizedPreparationEvidence,
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
pub use rustok_build_source::SourceTreeFile;
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
    SeaOrmArtifactSecretHandlePolicy, SeaOrmArtifactSecretHandleService,
    SeaOrmArtifactSecretService, SeaOrmArtifactSecretUseService,
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
pub use static_settings_localization::{
    StaticLocalizedSettingApplyCommand, StaticLocalizedSettingRecord,
    StaticSettingsLocalizedSourceSnapshot, StaticSettingsLocalizationError,
    StaticSettingsLocalizationRegistry, StaticSettingsLocalizationService,
};
pub use static_settings_source_locale::{
    StaticSettingsAuthoritativeSourceSnapshot, StaticSettingsSourceLocaleAssignCommand,
    StaticSettingsSourceLocaleError, StaticSettingsSourceLocaleRecord,
    StaticSettingsSourceLocaleService,
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
    TrustEvidenceKind, TrustEvidenceReference, TrustPolicyRevision, TrustVerificationDecision,
    TrustVerificationRequest, TrustVerifier,
};

/// Mandatory Core entry point for module and marketplace control-plane ownership.
pub struct ModulesModule;

impl MigrationSource for ModulesModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        vec![
            MigrationDependencyDescriptor::new(
                "m20260717_000002_create_registry_publish_build_staging",
                vec!["m20260403_000002_create_registry_publish_tables"],
            ),
            MigrationDependencyDescriptor::new(
                "m20260722_000034_static_promotions",
                vec!["m20260403_000002_create_registry_publish_tables"],
            ),
            MigrationDependencyDescriptor::new(
                "m20260727_000040_registry_platform_admission_contracts",
                vec!["m20260403_000002_create_registry_publish_tables"],
            ),
            MigrationDependencyDescriptor::new(
                "m20260727_000041_registry_release_artifact_contracts",
                vec!["m20260717_000002_create_registry_publish_build_staging"],
            ),
        ]
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
