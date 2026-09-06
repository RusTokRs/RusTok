//! RusToK Index - cross-module relational Index Engine.
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

use async_trait::async_trait;
use rustok_core::{
    MigrationDependencyDescriptor, MigrationSource, ModuleKind, ModuleRuntimeExtensions,
    RusToKModule,
};
use sea_orm_migration::MigrationTrait;

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod migrations;
pub mod replay_dry_run;

pub use application::*;
pub use domain::*;
pub use infrastructure::postgres::{
    INDEX_RECONCILIATION_WORKER, IndexDriftCandidateCompositionError,
    IndexDriftCandidateObserverCompositionError, IndexDriftConfirmedCandidateNotRecordedReason,
    IndexDriftConfirmedCandidateRecordError, IndexDriftConfirmedCandidateRecordOutcome,
    IndexDriftSnapshotCompositionError, IndexQueryRuntimeCompositionError,
    IndexReconciliationCancelOutcome, IndexReconciliationRetryDisposition,
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
    PartitionAdmissionPolicy, PartitionAdmissionReason, PartitionBaselineEvidence,
    PartitionEvidence, PartitionMeasurementCoverage, PartitionRelationPlan,
    PartitionShadowEvidence, PartitionShadowPlan, PartitionStrategy,
    PersistedSchemaRegistrationOutcome, PersistedSchemaSupersessionOutcome,
    PostgresIndexDriftCandidateMaterializedObserver, PostgresIndexDriftCandidateReader,
    PostgresIndexDriftConfirmedCandidateWriter, PostgresIndexDriftFindingLifecycleStore,
    PostgresIndexDriftMissingEntityEvidenceReader, PostgresIndexDriftMissingEntityRepairOwner,
    PostgresIndexDriftOrphanLinkEvidenceReader, PostgresIndexDriftOrphanLinkRepairOwner,
    PostgresIndexDriftRepairRecoveryStore, PostgresIndexDriftRepairStore,
    PostgresIndexDriftSnapshotReader, PostgresIndexQueryAdmissionCatalog,
    PostgresIndexQueryAdmissionDescriptor, PostgresIndexQueryAdmissionError,
    PostgresIndexQueryPort, PostgresIndexReconciliationRetryStore,
    PostgresIndexReconciliationRunner, PostgresIndexReconciliationWorkAdapter,
    PostgresIndexReplayCheckpointStore, PostgresIndexReplayJobStore, PostgresIndexReplayRunner,
    PostgresIndexSchemaReadinessStore, PostgresIndexSourceFactory,
    PostgresIndexSourceFactoryCatalog, PostgresIndexSourceFactoryDescriptor,
    PostgresIndexSourceFactoryError, PostgresMutationStore, PostgresSchemaLeaseStore,
    PostgresSchemaRegistrationStore, PostgresSecondaryIndexManager,
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
pub use replay_dry_run::*;

pub struct IndexModule;

#[async_trait]
impl RusToKModule for IndexModule {
    fn slug(&self) -> &'static str {
        "index"
    }

    fn name(&self) -> &'static str {
        "Index"
    }

    fn description(&self) -> &'static str {
        "Cross-module relational index and query engine."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        extensions.get_or_insert_with::<IndexSchemaSourceCatalog, _>(IndexSchemaSourceCatalog::new);
        extensions.get_or_insert_with::<IndexSourceCatalog, _>(IndexSourceCatalog::new);
        extensions.get_or_insert_with::<PostgresIndexSourceFactoryCatalog, _>(
            PostgresIndexSourceFactoryCatalog::new,
        );
        Ok(())
    }
}

impl MigrationSource for IndexModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod contract_tests;
