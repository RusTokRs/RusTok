//! # RusToK Index
//!
//! Cross-module relational Index Engine.
//!
//! The active implementation contains the database-independent generic engine
//! core under [`domain`] and [`application`], the canonical M3 PostgreSQL
//! storage-schema migrations, atomic mutation persistence, tenant-scoped source
//! schema registration, bounded source replay/load contracts, bounded side-effect-free
//! replay dry-run validation, one-page replay orchestration with durable fenced
//! checkpoint progression, bounded multi-page replay coordination with
//! heartbeat/yield/cancellation semantics,
//! bounded multi-pass source reconciliation with durable pass/cursor progression,
//! bounded reconciliation retry transitions,
//! host-owned due reconciliation scheduling through the generic module-work lifecycle,
//! host-published replay and query capabilities, host-database-aware source factory
//! composition, durable replay-job and schema-application leases, schema-derived
//! secondary-index lifecycle, fail-closed measured partition admission, and the
//! PostgreSQL execution adapter for structured Index queries.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod migrations;
pub mod module;
pub mod replay_dry_run;

pub use application::*;
pub use domain::*;

pub use application::{
    AggregateOrderValidationError, CompiledManyRelationColumn, CompiledPostgresCell,
    CompiledPostgresCount, CompiledPostgresLocalizedPageQuery, CompiledPostgresPageQuery,
    CompiledPostgresQuery, CompiledPostgresRow, CompiledQueryColumn, CursorCodec, CursorCodecError,
    CursorValidationError, ExecutableQueryPlan, IndexCursor, IndexDriftAuthorizedRepairCommand,
    IndexDriftAuthorizedRepairRecoveryCommand, IndexDriftCandidate,
    IndexDriftCandidateConfirmationError, IndexDriftCandidateConfirmationFailure,
    IndexDriftCandidateConfirmationFailureKind, IndexDriftCandidateConfirmationOutcome,
    IndexDriftCandidateConfirmer, IndexDriftCandidateCursor, IndexDriftCandidateError,
    IndexDriftCandidateFailure, IndexDriftCandidateFailureKind, IndexDriftCandidateFence,
    IndexDriftCandidateMaterializedObservation, IndexDriftCandidateMaterializedObserver,
    IndexDriftCandidateNotCandidateReason, IndexDriftCandidatePage, IndexDriftCandidateReader,
    IndexDriftCandidateRequest, IndexDriftCandidateScope, IndexDriftConfirmedCandidate,
    IndexDriftConfirmedMissingEntity, IndexDriftConfirmedOrphanLink, IndexDriftDependencyFailure,
    IndexDriftDependencyFailureKind, IndexDriftDigestError, IndexDriftDigestMismatch,
    IndexDriftDigestOutcome, IndexDriftDigestProducer, IndexDriftDigestRequest,
    IndexDriftEntityState, IndexDriftFindingAuthorizedLifecycleCommand,
    IndexDriftFindingLifecycleAction, IndexDriftFindingLifecycleActor,
    IndexDriftFindingLifecycleAuthorization, IndexDriftFindingLifecycleAuthorizer,
    IndexDriftFindingLifecycleCommand, IndexDriftFindingLifecycleFailure,
    IndexDriftFindingLifecycleFailureError, IndexDriftFindingLifecycleFailureKind,
    IndexDriftFindingLifecycleNotAppliedReason, IndexDriftFindingLifecycleOutcome,
    IndexDriftFindingLifecycleReceipt, IndexDriftFindingLifecycleService,
    IndexDriftFindingLifecycleStore, IndexDriftFindingLifecycleStoreOutcome,
    IndexDriftFindingLifecycleValidationError, IndexDriftFindingState, IndexDriftMismatchReceipt,
    IndexDriftMismatchRecordStatus, IndexDriftMismatchRecorder,
    IndexDriftMissingEntityCandidateOutcome, IndexDriftOrphanLinkCandidate,
    IndexDriftRepairAuthorization, IndexDriftRepairAuthorizer, IndexDriftRepairCommand,
    IndexDriftRepairCompletion, IndexDriftRepairEvidence, IndexDriftRepairEvidenceReader,
    IndexDriftRepairEvidenceState, IndexDriftRepairFailure, IndexDriftRepairFailureError,
    IndexDriftRepairFailureKind, IndexDriftRepairFinding, IndexDriftRepairNotStartedReason,
    IndexDriftRepairOutcome, IndexDriftRepairOwner, IndexDriftRepairOwnerOutcome,
    IndexDriftRepairOwnerRegistry, IndexDriftRepairReceipt, IndexDriftRepairReceiptOutcome,
    IndexDriftRepairRecoveryAction, IndexDriftRepairRecoveryAuthorization,
    IndexDriftRepairRecoveryAuthorizer, IndexDriftRepairRecoveryCommand,
    IndexDriftRepairRecoveryCommandParams, IndexDriftRepairRecoveryFailure,
    IndexDriftRepairRecoveryFailureError, IndexDriftRepairRecoveryFailureKind,
    IndexDriftRepairRecoveryOutcome, IndexDriftRepairRecoveryReceipt,
    IndexDriftRepairRecoveryService, IndexDriftRepairRecoveryState, IndexDriftRepairRecoveryStore,
    IndexDriftRepairRecoveryStoreOutcome, IndexDriftRepairRecoveryValidationError,
    IndexDriftRepairReservationOutcome, IndexDriftRepairService, IndexDriftRepairStore,
    IndexDriftRepairStoreCompletionOutcome, IndexDriftRepairTarget, IndexDriftRepairTargetKind,
    IndexDriftRepairTicket, IndexDriftRepairValidationError, IndexDriftSnapshotBoundary,
    IndexDriftSnapshotPair, IndexDriftSnapshotReader, IndexDriftSnapshotView,
    IndexDriftStaleEntityCandidate, IndexMutationAcknowledgeFailure,
    IndexMutationAcknowledgeFailureKind, IndexMutationEventAcknowledger, IndexMutationEventCatalog,
    IndexMutationEventDelivery, IndexMutationEventDescriptor, IndexMutationEventError,
    IndexMutationEventProcessError, IndexMutationEventProcessOutcome, IndexMutationEventWorker,
    IndexNestedRelationItem, IndexNestedRelationProjection, IndexProjectedValue,
    IndexQueryExecutionError, IndexQueryItem, IndexQueryPage, IndexQueryPort,
    IndexRelationIdentity, IndexReplayCheckpoint, IndexReplayCheckpointKey,
    IndexReplayCheckpointStore, IndexReplayError, IndexReplayExecutionSurface, IndexReplayFailure,
    IndexReplayFailureKind, IndexReplayMode, IndexReplayModeSelection, IndexReplayMutationOutcome,
    IndexReplayMutationSink, IndexReplayPageOutcome, IndexReplayPageRequest, IndexReplayPageStatus,
    IndexReplayTargetedError, IndexReplayTargetedExecutor, IndexReplayTargetedOutcome,
    IndexReplayWorker, IndexSchemaSourceCatalog, IndexSchemaSourceDescriptor,
    IndexSchemaSourceError, IndexSource, IndexSourceAbsenceCatalog, IndexSourceAbsenceDescriptor,
    IndexSourceAbsenceError, IndexSourceAbsenceProvider, IndexSourceAbsenceWatermark,
    IndexSourceCatalog, IndexSourceContinuationCodec, IndexSourceContinuationError,
    IndexSourceContinuationScope, IndexSourceContinuationToken, IndexSourceCursor,
    IndexSourceDescriptor, IndexSourceError, IndexSourceEventIdError, IndexSourceFailure,
    IndexSourceFailureKind, IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage,
    IndexSourceRefreshEventDelivery, IndexSourceRefreshEventError,
    IndexSourceRefreshEventProcessError, IndexSourceRefreshEventProcessOutcome,
    IndexSourceRefreshEventWorker, IndexSourceScanRequest, LinkPathStep, LocalizedCursorCodec,
    LocalizedCursorCodecError, LocalizedCursorValidationError, LocalizedEntityQueryValidationError,
    LocalizedIndexCursor, LocalizedQueryPlanFingerprint, PersistedSchemaReadinessFailure,
    PlannedField, PlannedJoin, PlannedManyProjection, PlannedOrder, PostgresBindValue,
    PostgresLocalizedQueryBuildError, PostgresLocalizedQueryDecodeError, PostgresQueryBuildError,
    PostgresQueryCompileError, PostgresQueryDecodeError, PostgresQueryEntityAdmission,
    PostgresQueryEntityAdmissionApplyError, PostgresQueryEntityAdmissionError,
    PostgresQueryPageBuildError, QueryPlanError, QueryPlanFingerprint, QueryValidationError,
    RecordValidationError, RegisteredSchema, RegistrationOutcome, SchemaRegistry,
    SchemaRegistryError, SharedIndexMutationEventRegistry, SharedIndexQueryRuntime,
    SharedIndexSchemaRegistry, SharedIndexSourceAbsenceRegistry, SharedIndexSourceRegistry,
    derive_index_schema_source_event_id, derive_index_source_event_id,
    materialize_index_mutation_event_registry, materialize_index_schema_registry,
    materialize_index_source_absence_registry, materialize_index_source_registry,
    register_index_mutation_event, register_index_schema_source, register_index_source,
    register_index_source_absence_provider,
};
pub use domain::{
    DomainError, EntityKey, EntityName, FieldCardinality, FieldName, FieldPath, FilterExpr,
    IndexField, IndexLink, IndexLinkValue, IndexMutation, IndexQuery, IndexQueryScope, IndexRecord,
    IndexSchema, IndexValue, IndexValueType, LinkCardinality, LinkName, LinkedEntityKey, LocaleKey,
    LocaleMode, LocalizedEntityQuery, ManyOrderAggregate, ModuleName, OrderDirection, OrderExpr,
    Pagination, SchemaFingerprint, SchemaIdentity, SchemaRef, SchemaVersion,
};
pub use infrastructure::postgres::{
    INDEX_RECONCILIATION_WORKER, IndexDriftCandidateCompositionError,
    IndexDriftCandidateObserverCompositionError, IndexDriftConfirmedCandidateNotRecordedReason,
    IndexDriftConfirmedCandidateRecordError, IndexDriftConfirmedCandidateRecordOutcome,
    IndexDriftSnapshotCompositionError, IndexQueryRuntimeCompositionError,
    IndexReconciliationBudget, IndexReconciliationCancelOutcome,
    IndexReconciliationDeadLetterInspection, IndexReconciliationDeadLetterInspectionError,
    IndexReconciliationRecoveryError, IndexReconciliationRequeueOutcome,
    IndexReconciliationRequeueRequest, IndexReconciliationRetryDisposition,
    IndexReconciliationRetryError, IndexReconciliationRetryFailure,
    IndexReconciliationRetryFailureKind, IndexReconciliationRetryLease,
    IndexReconciliationRetryPolicy, IndexReconciliationRunError, IndexReconciliationRunOutcome,
    IndexReconciliationRunRequest, IndexReconciliationRunStatus,
    IndexReconciliationSchedulerCompositionError, IndexReconciliationSchedulerPolicy,
    IndexReconciliationTerminalState, IndexReplayCancelOutcome, IndexReplayJobAcquireOutcome,
    IndexReplayJobError, IndexReplayJobLease, IndexReplayJobLeaseRequest, IndexReplayRunError,
    IndexReplayRunOutcome, IndexReplayRunRequest, IndexReplayRunStatus,
    IndexReplayRuntimeCompositionError, IndexReplayTerminalState, IndexSchemaReadinessEntry,
    IndexSchemaReadinessError, IndexSchemaReadinessFailure, IndexSchemaReadinessReceipt,
    IndexSchemaReadinessRequest, MAX_INDEX_SCHEMA_READINESS_SCHEMAS, MutationApplyOutcome,
    MutationDelivery, MutationStorageError, PartitionAdmissionError, PartitionAdmissionOutcome,
    PartitionAdmissionPolicy, PartitionAdmissionPolicyParams, PartitionAdmissionReason,
    PartitionBaselineEvidence, PartitionEvidence, PartitionMeasurementCoverage,
    PartitionRelationPlan, PartitionShadowEvidence, PartitionShadowEvidenceParams,
    PartitionShadowPlan, PartitionStrategy, PersistedSchemaRegistrationOutcome,
    PersistedSchemaSupersessionOutcome, PostgresIndexDriftCandidateMaterializedObserver,
    PostgresIndexDriftCandidateReader, PostgresIndexDriftConfirmedCandidateWriter,
    PostgresIndexDriftFindingLifecycleStore, PostgresIndexDriftMissingEntityEvidenceReader,
    PostgresIndexDriftMissingEntityRepairOwner, PostgresIndexDriftOrphanLinkEvidenceReader,
    PostgresIndexDriftOrphanLinkRepairOwner, PostgresIndexDriftRepairRecoveryStore,
    PostgresIndexDriftRepairStore, PostgresIndexDriftSnapshotReader,
    PostgresIndexQueryAdmissionCatalog, PostgresIndexQueryAdmissionDescriptor,
    PostgresIndexQueryAdmissionError, PostgresIndexQueryPort,
    PostgresIndexReconciliationDeadLetterInspector, PostgresIndexReconciliationRecoveryStore,
    PostgresIndexReconciliationRetryStore, PostgresIndexReconciliationRunner,
    PostgresIndexReconciliationWorkAdapter, PostgresIndexReplayCheckpointStore,
    PostgresIndexReplayJobStore, PostgresIndexReplayRunner, PostgresIndexSchemaReadinessStore,
    PostgresIndexSourceFactory, PostgresIndexSourceFactoryCatalog,
    PostgresIndexSourceFactoryDescriptor, PostgresIndexSourceFactoryError, PostgresMutationStore,
    PostgresSchemaLeaseStore, PostgresSchemaRegistrationStore, PostgresSecondaryIndexManager,
    RecoveryAwareIndexDriftRepairOwner, RecoveryAwareIndexDriftRepairStore, SchemaApplicationLease,
    SchemaApplicationLeaseRequest, SchemaLeaseAcquireOutcome, SchemaLeaseError,
    SchemaRegistrationError, SecondaryIndexClaimOutcome, SecondaryIndexError,
    SecondaryIndexExecutionOutcome, SecondaryIndexKind, SecondaryIndexLease,
    SecondaryIndexOperation, SecondaryIndexPlan, SecondaryIndexRequest, SecondaryIndexSpec,
    SharedIndexReplayRuntime, evaluate_partition_admission,
    materialize_postgres_index_drift_candidate_confirmer,
    materialize_postgres_index_drift_candidate_observer,
    materialize_postgres_index_drift_candidate_reader,
    materialize_postgres_index_drift_confirmed_candidate_writer,
    materialize_postgres_index_drift_finding_lifecycle_store,
    materialize_postgres_index_drift_missing_entity_repair_service,
    materialize_postgres_index_drift_orphan_link_repair_service,
    materialize_postgres_index_drift_repair_recovery_store,
    materialize_postgres_index_drift_repair_store,
    materialize_postgres_index_drift_snapshot_reader, materialize_postgres_index_query_runtime,
    materialize_postgres_index_replay_runtime, materialize_postgres_index_sources,
    register_postgres_index_query_admission,
    register_postgres_index_query_link_target_availability,
    register_postgres_index_reconciliation_work, register_postgres_index_source_factory,
};
pub use module::IndexModule;
// Canonical migrations provided via migrations::migrations()
pub use replay_dry_run::{
    IndexReplayDryRunError, IndexReplayDryRunOutcome, IndexReplayDryRunRequest,
    IndexReplayDryRunRuntimeCompositionError, IndexReplayDryRunStatus,
    SharedIndexReplayDryRunRuntime, materialize_index_replay_dry_run_runtime,
};

#[cfg(test)]
mod contract_tests;
