mod aggregate_ordering;
mod cursor;
mod drift_candidate_confirmation;
mod drift_candidates;
mod drift_digest;
mod drift_finding_lifecycle;
mod drift_repair;
mod drift_repair_recovery;
mod localized_cursor;
mod localized_validation;
mod mutation_event;
mod planner;
mod postgres_compiler;
mod postgres_localized_query;
mod postgres_localized_query_result;
mod postgres_query_admission;
mod postgres_query_result;
mod postgres_query_sql;
mod query_port;
mod query_runtime;
mod registry;
mod replay_mode;
mod source_absence;
mod source_continuation;
mod source_event_id;
mod source_refresh_event;
mod source_registry;
mod source_replay;
mod source_schema_registry;
mod source_timeout;
mod validation;

#[cfg(test)]
mod aggregate_ordering_tests;
#[cfg(test)]
mod drift_repair_tests;
#[cfg(test)]
mod planner_tests;
#[cfg(test)]
mod postgres_compiler_tests;
#[cfg(test)]
mod postgres_many_projection_tests;
#[cfg(test)]
mod postgres_query_result_tests;
#[cfg(test)]
mod query_snapshot_tests;
#[cfg(test)]
mod reference;
#[cfg(test)]
mod source_refresh_event_tests;
#[cfg(test)]
mod source_replay_tests;

pub use aggregate_ordering::AggregateOrderValidationError;
pub use cursor::{CursorCodec, CursorCodecError, CursorValidationError, IndexCursor};
pub use drift_candidate_confirmation::{
    IndexDriftCandidateConfirmationError, IndexDriftCandidateConfirmationFailure,
    IndexDriftCandidateConfirmationFailureKind, IndexDriftCandidateConfirmationOutcome,
    IndexDriftCandidateConfirmer, IndexDriftCandidateMaterializedObservation,
    IndexDriftCandidateMaterializedObserver, IndexDriftCandidateNotCandidateReason,
    IndexDriftConfirmedCandidate, IndexDriftConfirmedMissingEntity,
    IndexDriftConfirmedOrphanLink,
};
pub use drift_candidates::{
    IndexDriftCandidate, IndexDriftCandidateCursor, IndexDriftCandidateError,
    IndexDriftCandidateFailure, IndexDriftCandidateFailureKind, IndexDriftCandidateFence,
    IndexDriftCandidatePage, IndexDriftCandidateReader, IndexDriftCandidateRequest,
    IndexDriftCandidateScope, IndexDriftOrphanLinkCandidate, IndexDriftStaleEntityCandidate,
};
pub use drift_digest::{
    IndexDriftDependencyFailure, IndexDriftDependencyFailureKind, IndexDriftDigestError,
    IndexDriftDigestMismatch, IndexDriftDigestOutcome, IndexDriftDigestProducer,
    IndexDriftDigestRequest, IndexDriftEntityState, IndexDriftMismatchReceipt,
    IndexDriftMismatchRecordStatus, IndexDriftMismatchRecorder,
    IndexDriftMissingEntityCandidateOutcome, IndexDriftSnapshotBoundary, IndexDriftSnapshotPair,
    IndexDriftSnapshotReader, IndexDriftSnapshotView,
};
pub use drift_finding_lifecycle::{
    IndexDriftFindingAuthorizedLifecycleCommand, IndexDriftFindingLifecycleAction,
    IndexDriftFindingLifecycleActor, IndexDriftFindingLifecycleAuthorization,
    IndexDriftFindingLifecycleAuthorizer, IndexDriftFindingLifecycleCommand,
    IndexDriftFindingLifecycleFailure, IndexDriftFindingLifecycleFailureError,
    IndexDriftFindingLifecycleFailureKind, IndexDriftFindingLifecycleNotAppliedReason,
    IndexDriftFindingLifecycleOutcome, IndexDriftFindingLifecycleReceipt,
    IndexDriftFindingLifecycleService, IndexDriftFindingLifecycleStore,
    IndexDriftFindingLifecycleStoreOutcome, IndexDriftFindingLifecycleValidationError,
    IndexDriftFindingState,
};
pub use drift_repair::{
    IndexDriftAuthorizedRepairCommand, IndexDriftRepairAuthorization,
    IndexDriftRepairAuthorizer, IndexDriftRepairCommand, IndexDriftRepairCompletion,
    IndexDriftRepairEvidence, IndexDriftRepairEvidenceReader, IndexDriftRepairEvidenceState,
    IndexDriftRepairFailure, IndexDriftRepairFailureError, IndexDriftRepairFailureKind,
    IndexDriftRepairFinding, IndexDriftRepairNotStartedReason, IndexDriftRepairOutcome,
    IndexDriftRepairOwner, IndexDriftRepairOwnerOutcome, IndexDriftRepairOwnerRegistry,
    IndexDriftRepairReceipt, IndexDriftRepairReceiptOutcome, IndexDriftRepairReservationOutcome,
    IndexDriftRepairService, IndexDriftRepairStore, IndexDriftRepairStoreCompletionOutcome,
    IndexDriftRepairTarget, IndexDriftRepairTargetKind, IndexDriftRepairTicket,
    IndexDriftRepairValidationError,
};
pub use drift_repair_recovery::{
    IndexDriftAuthorizedRepairRecoveryCommand, IndexDriftRepairRecoveryAction,
    IndexDriftRepairRecoveryAuthorization, IndexDriftRepairRecoveryAuthorizer,
    IndexDriftRepairRecoveryCommand, IndexDriftRepairRecoveryFailure,
    IndexDriftRepairRecoveryFailureError, IndexDriftRepairRecoveryFailureKind,
    IndexDriftRepairRecoveryOutcome, IndexDriftRepairRecoveryReceipt,
    IndexDriftRepairRecoveryService, IndexDriftRepairRecoveryState,
    IndexDriftRepairRecoveryStore, IndexDriftRepairRecoveryStoreOutcome,
    IndexDriftRepairRecoveryValidationError,
};
pub use localized_cursor::{
    LocalizedCursorCodec, LocalizedCursorCodecError, LocalizedCursorValidationError,
    LocalizedIndexCursor,
};
pub use localized_validation::LocalizedEntityQueryValidationError;
pub use mutation_event::{
    IndexMutationAcknowledgeFailure, IndexMutationAcknowledgeFailureKind,
    IndexMutationEventAcknowledger, IndexMutationEventCatalog, IndexMutationEventDelivery,
    IndexMutationEventDescriptor, IndexMutationEventError, IndexMutationEventProcessError,
    IndexMutationEventProcessOutcome, IndexMutationEventWorker, SharedIndexMutationEventRegistry,
    materialize_index_mutation_event_registry, register_index_mutation_event,
};
pub use planner::{
    ExecutableQueryPlan, PlannedField, PlannedJoin, PlannedManyProjection, PlannedOrder,
    QueryPlanError, QueryPlanFingerprint,
};
pub use postgres_compiler::{
    CompiledManyRelationColumn, CompiledPostgresCount, CompiledPostgresQuery,
    CompiledQueryColumn, PostgresBindValue, PostgresQueryBuildError, PostgresQueryCompileError,
};
pub use postgres_localized_query::{
    CompiledPostgresLocalizedPageQuery, LocalizedQueryPlanFingerprint,
    PostgresLocalizedQueryBuildError,
};
pub use postgres_localized_query_result::PostgresLocalizedQueryDecodeError;
pub use postgres_query_admission::{
    PostgresQueryEntityAdmission, PostgresQueryEntityAdmissionApplyError,
    PostgresQueryEntityAdmissionError,
};
pub use postgres_query_result::{
    CompiledPostgresCell, CompiledPostgresPageQuery, CompiledPostgresRow,
    IndexNestedRelationItem, IndexNestedRelationProjection, IndexProjectedValue, IndexQueryItem,
    IndexQueryPage, IndexRelationIdentity, PostgresQueryDecodeError, PostgresQueryPageBuildError,
};
pub use query_port::{
    IndexQueryExecutionError, IndexQueryPort, PersistedSchemaReadinessFailure,
};
pub use query_runtime::SharedIndexQueryRuntime;
pub use registry::{
    LinkPathStep, RegisteredSchema, RegistrationOutcome, SchemaRegistry, SchemaRegistryError,
};
pub use replay_mode::{
    IndexReplayExecutionSurface, IndexReplayMode, IndexReplayModeSelection,
};
pub use source_absence::{
    IndexSourceAbsenceCatalog, IndexSourceAbsenceDescriptor, IndexSourceAbsenceError,
    IndexSourceAbsenceProvider, IndexSourceAbsenceWatermark, SharedIndexSourceAbsenceRegistry,
    materialize_index_source_absence_registry, register_index_source_absence_provider,
};
pub use source_continuation::{
    IndexSourceContinuationCodec, IndexSourceContinuationError, IndexSourceContinuationScope,
    IndexSourceContinuationToken,
};
pub use source_event_id::{
    IndexSourceEventIdError, derive_index_schema_source_event_id, derive_index_source_event_id,
};
pub use source_refresh_event::{
    IndexSourceRefreshEventDelivery, IndexSourceRefreshEventError,
    IndexSourceRefreshEventProcessError, IndexSourceRefreshEventProcessOutcome,
    IndexSourceRefreshEventWorker,
};
pub use source_registry::{
    IndexSource, IndexSourceCatalog, IndexSourceCursor, IndexSourceDescriptor, IndexSourceError,
    IndexSourceFailure, IndexSourceFailureKind, IndexSourceLoadBatch, IndexSourceLoadRequest,
    IndexSourcePage, IndexSourceScanRequest, SharedIndexSourceRegistry,
    materialize_index_source_registry,
};
pub use source_replay::{
    IndexReplayCheckpoint, IndexReplayCheckpointKey, IndexReplayCheckpointStore, IndexReplayError,
    IndexReplayFailure, IndexReplayFailureKind, IndexReplayMutationOutcome, IndexReplayMutationSink,
    IndexReplayPageOutcome, IndexReplayPageRequest, IndexReplayPageStatus, IndexReplayWorker,
};
pub use source_schema_registry::{
    IndexSchemaSourceCatalog, IndexSchemaSourceDescriptor, IndexSchemaSourceError,
    SharedIndexSchemaRegistry, materialize_index_schema_registry, register_index_schema_source,
};
pub use source_timeout::register_index_source;
pub use validation::{QueryValidationError, RecordValidationError};
