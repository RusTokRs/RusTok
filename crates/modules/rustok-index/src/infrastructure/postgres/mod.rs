mod drift_candidate_observer;
mod drift_candidate_reader;
mod drift_confirmed_candidate_writer;
mod drift_digest_recorder;
mod drift_finding_inspector;
mod drift_finding_lifecycle;
mod drift_finding_writer;
mod drift_missing_entity_repair;
mod drift_orphan_link_repair;
mod drift_repair;
mod drift_repair_recovery;
mod drift_snapshot_reader;
mod mutation_store;
mod partition_admission;
mod query_admission;
mod query_port;
mod query_runtime;
mod replay_runtime;
mod schema_lease;
mod schema_readiness;
mod schema_registration;
mod secondary_index;
mod source_factory;
mod source_reconciliation_dead_letter_inspector;
mod source_reconciliation_recovery;
mod source_reconciliation_retry;
mod source_reconciliation_runner;
mod source_reconciliation_scheduler;
mod source_replay;
mod source_replay_job;
mod source_replay_retry;
mod source_replay_timeout;
mod source_replay_runner {
    include!("source_replay_runner.rs");
    mod graceful_shutdown;
}

#[cfg(test)]
mod mutation_store_tests;
#[cfg(test)]
mod partition_admission_tests;
#[cfg(test)]
mod postgres_reference_equivalence_tests;
#[cfg(test)]
mod schema_lease_tests;
#[cfg(test)]
mod schema_readiness_tests;
#[cfg(test)]
mod schema_registration_tests;
#[cfg(test)]
mod secondary_index_tests;
#[cfg(test)]
mod source_reconciliation_runner_tests;
#[cfg(test)]
mod source_replay_graceful_shutdown_tests;
#[cfg(test)]
mod source_replay_job_tests;
#[cfg(test)]
mod source_replay_locale_job_tests;
#[cfg(test)]
mod source_replay_multihost_restart_tests;
#[cfg(test)]
mod source_replay_runner_tests;

pub use drift_candidate_observer::{
    IndexDriftCandidateObserverCompositionError, PostgresIndexDriftCandidateMaterializedObserver,
    materialize_postgres_index_drift_candidate_confirmer,
    materialize_postgres_index_drift_candidate_observer,
};
pub use drift_candidate_reader::{
    IndexDriftCandidateCompositionError, PostgresIndexDriftCandidateReader,
    materialize_postgres_index_drift_candidate_reader,
};
pub use drift_confirmed_candidate_writer::{
    IndexDriftConfirmedCandidateNotRecordedReason, IndexDriftConfirmedCandidateRecordError,
    IndexDriftConfirmedCandidateRecordOutcome, PostgresIndexDriftConfirmedCandidateWriter,
    materialize_postgres_index_drift_confirmed_candidate_writer,
};
pub use drift_finding_inspector::{
    IndexDriftFindingInspection, IndexDriftFindingInspectionError, IndexDriftFindingScope,
    IndexDriftFindingSeverity, PostgresIndexDriftFindingInspector,
};
pub use drift_finding_lifecycle::{
    PostgresIndexDriftFindingLifecycleStore,
    materialize_postgres_index_drift_finding_lifecycle_store,
};
pub use drift_finding_writer::{
    IndexDriftDigestFindingRequest, IndexDriftFindingWriteError, IndexDriftFindingWriteOutcome,
    PostgresIndexDriftFindingWriter,
};
pub use drift_missing_entity_repair::{
    PostgresIndexDriftMissingEntityEvidenceReader, PostgresIndexDriftMissingEntityRepairOwner,
    materialize_postgres_index_drift_missing_entity_repair_service,
};
pub use drift_orphan_link_repair::{
    PostgresIndexDriftOrphanLinkEvidenceReader, PostgresIndexDriftOrphanLinkRepairOwner,
    materialize_postgres_index_drift_orphan_link_repair_service,
};
pub use drift_repair::{
    PostgresIndexDriftRepairStore, materialize_postgres_index_drift_repair_store,
};
pub use drift_repair_recovery::{
    PostgresIndexDriftRepairRecoveryStore, RecoveryAwareIndexDriftRepairOwner,
    RecoveryAwareIndexDriftRepairStore, materialize_postgres_index_drift_repair_recovery_store,
};
pub use drift_snapshot_reader::{
    IndexDriftSnapshotCompositionError, PostgresIndexDriftSnapshotReader,
    materialize_postgres_index_drift_snapshot_reader,
};
pub use mutation_store::{
    MutationApplyOutcome, MutationDelivery, MutationStorageError, PostgresMutationStore,
};
pub use partition_admission::{
    PartitionAdmissionError, PartitionAdmissionOutcome, PartitionAdmissionPolicy,
    PartitionAdmissionReason, PartitionBaselineEvidence, PartitionEvidence,
    PartitionMeasurementCoverage, PartitionRelationPlan, PartitionShadowEvidence,
    PartitionShadowPlan, PartitionStrategy, evaluate_partition_admission,
};
pub use query_admission::{
    PostgresIndexQueryAdmissionCatalog, PostgresIndexQueryAdmissionDescriptor,
    PostgresIndexQueryAdmissionError, register_postgres_index_query_admission,
    register_postgres_index_query_link_target_availability,
};
pub use query_port::PostgresIndexQueryPort;
pub use query_runtime::{
    IndexQueryRuntimeCompositionError, materialize_postgres_index_query_runtime,
};
pub use replay_runtime::{
    IndexReplayRuntimeCompositionError, SharedIndexReplayRuntime,
    materialize_postgres_index_replay_runtime,
};
pub use schema_lease::{
    PostgresSchemaLeaseStore, SchemaApplicationLease, SchemaApplicationLeaseRequest,
    SchemaLeaseAcquireOutcome, SchemaLeaseError,
};
pub use schema_readiness::{
    IndexSchemaReadinessEntry, IndexSchemaReadinessError, IndexSchemaReadinessFailure,
    IndexSchemaReadinessReceipt, IndexSchemaReadinessRequest, MAX_INDEX_SCHEMA_READINESS_SCHEMAS,
    PostgresIndexSchemaReadinessStore,
};
pub use schema_registration::{
    PersistedSchemaRegistrationOutcome, PersistedSchemaSupersessionOutcome,
    PostgresSchemaRegistrationStore, SchemaRegistrationError,
};
pub use secondary_index::{
    PostgresSecondaryIndexManager, SecondaryIndexClaimOutcome, SecondaryIndexError,
    SecondaryIndexExecutionOutcome, SecondaryIndexKind, SecondaryIndexLease,
    SecondaryIndexOperation, SecondaryIndexPlan, SecondaryIndexRequest, SecondaryIndexSpec,
};
pub use source_factory::{
    PostgresIndexSourceFactory, PostgresIndexSourceFactoryCatalog,
    PostgresIndexSourceFactoryDescriptor, PostgresIndexSourceFactoryError,
    materialize_postgres_index_sources, register_postgres_index_source_factory,
};
pub use source_reconciliation_dead_letter_inspector::{
    IndexReconciliationDeadLetterInspection, IndexReconciliationDeadLetterInspectionError,
    PostgresIndexReconciliationDeadLetterInspector,
};
pub use source_reconciliation_recovery::{
    IndexReconciliationRecoveryError, IndexReconciliationRequeueOutcome,
    IndexReconciliationRequeueRequest, PostgresIndexReconciliationRecoveryStore,
};
pub use source_reconciliation_retry::{
    IndexReconciliationRetryDisposition, IndexReconciliationRetryError,
    IndexReconciliationRetryFailure, IndexReconciliationRetryFailureKind,
    IndexReconciliationRetryLease, IndexReconciliationRetryPolicy,
    PostgresIndexReconciliationRetryStore,
};
pub use source_reconciliation_runner::{
    IndexReconciliationCancelOutcome, IndexReconciliationRunError, IndexReconciliationRunOutcome,
    IndexReconciliationRunRequest, IndexReconciliationRunStatus, IndexReconciliationTerminalState,
    PostgresIndexReconciliationRunner,
};
pub use source_reconciliation_scheduler::{
    INDEX_RECONCILIATION_WORKER, IndexReconciliationSchedulerPolicy,
    PostgresIndexReconciliationWorkAdapter,
    IndexReconciliationSchedulerCompositionError,
    register_postgres_index_reconciliation_work,
};
#[cfg(test)]
pub(crate) use source_reconciliation_scheduler::IndexReconciliationWorkRegistration;
pub use source_replay::PostgresIndexReplayCheckpointStore;
pub use source_replay_job::{
    IndexReplayJobAcquireOutcome, IndexReplayJobError, IndexReplayJobLease,
    IndexReplayJobLeaseRequest, PostgresIndexReplayJobStore,
};
pub use source_replay_retry::{
    IndexReplayRetryDisposition, IndexReplayRetryError, IndexReplayRetryFailure,
    IndexReplayRetryFailureKind, IndexReplayRetryPolicy, PostgresIndexReplayRetryStore,
};
pub use source_replay_runner::{
    IndexReplayCancelOutcome, IndexReplayRunError, IndexReplayRunOutcome, IndexReplayRunRequest,
    IndexReplayRunStatus, IndexReplayTerminalState, PostgresIndexReplayRunner,
};

#[cfg(test)]
mod tests {
    use super::IndexReconciliationWorkRegistration;

    #[test]
    fn reconciliation_work_registration_is_exported() {
        let _ = IndexReconciliationWorkRegistration;
    }
}
