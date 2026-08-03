mod drift_finding_inspector;
mod drift_finding_writer;
mod mutation_store;
mod partition_admission;
mod query_port;
mod query_runtime;
mod replay_runtime;
mod schema_lease;
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
mod source_replay_runner;

#[cfg(test)]
mod mutation_store_tests;
#[cfg(test)]
mod partition_admission_tests;
#[cfg(test)]
mod postgres_reference_equivalence_tests;
#[cfg(test)]
mod schema_lease_tests;
#[cfg(test)]
mod schema_registration_tests;
#[cfg(test)]
mod secondary_index_tests;
#[cfg(test)]
mod source_reconciliation_runner_tests;
#[cfg(test)]
mod source_replay_job_tests;
#[cfg(test)]
mod source_replay_runner_tests;

pub use drift_finding_inspector::{
    IndexDriftFindingInspection, IndexDriftFindingInspectionError, IndexDriftFindingScope,
    IndexDriftFindingSeverity, PostgresIndexDriftFindingInspector,
};
pub use drift_finding_writer::{
    IndexDriftDigestFindingRequest, IndexDriftFindingWriteError, IndexDriftFindingWriteOutcome,
    PostgresIndexDriftFindingWriter,
};
pub use mutation_store::{
    MutationApplyOutcome, MutationDelivery, MutationStorageError, PostgresMutationStore,
};
pub use partition_admission::{
    evaluate_partition_admission, PartitionAdmissionError, PartitionAdmissionOutcome,
    PartitionAdmissionPolicy, PartitionAdmissionReason, PartitionBaselineEvidence,
    PartitionEvidence, PartitionMeasurementCoverage, PartitionRelationPlan,
    PartitionShadowEvidence, PartitionShadowPlan, PartitionStrategy,
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
pub use schema_registration::{
    PersistedSchemaRegistrationOutcome, PostgresSchemaRegistrationStore, SchemaRegistrationError,
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
    IndexReconciliationCancelOutcome, IndexReconciliationRunError,
    IndexReconciliationRunOutcome, IndexReconciliationRunRequest, IndexReconciliationRunStatus,
    IndexReconciliationTerminalState, PostgresIndexReconciliationRunner,
};
pub use source_reconciliation_scheduler::{
    INDEX_RECONCILIATION_WORKER, IndexReconciliationSchedulerCompositionError,
    IndexReconciliationSchedulerPolicy, PostgresIndexReconciliationWorkAdapter,
    register_postgres_index_reconciliation_work,
};
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
