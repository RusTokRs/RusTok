//! RusToK Index - cross-module relational Index Engine.
//!
//! The active implementation contains the database-independent generic engine
//! core under [`domain`] and [`application`], the canonical M3 PostgreSQL
//! storage-schema migrations, atomic mutation persistence, durable
//! schema-application leases, schema-derived secondary-index lifecycle, and a
//! fail-closed measured partition-admission contract.

use async_trait::async_trait;
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod migrations;

pub use application::*;
pub use domain::*;
pub use infrastructure::postgres::{
    evaluate_partition_admission, MutationApplyOutcome, MutationDelivery, MutationStorageError,
    PartitionAdmissionError, PartitionAdmissionOutcome, PartitionAdmissionPolicy,
    PartitionAdmissionReason, PartitionBaselineEvidence, PartitionEvidence,
    PartitionMeasurementCoverage, PartitionRelationPlan, PartitionShadowEvidence,
    PartitionShadowPlan, PartitionStrategy, PostgresMutationStore, PostgresSchemaLeaseStore,
    PostgresSecondaryIndexManager, SchemaApplicationLease, SchemaApplicationLeaseRequest,
    SchemaLeaseAcquireOutcome, SchemaLeaseError, SecondaryIndexClaimOutcome,
    SecondaryIndexError, SecondaryIndexExecutionOutcome, SecondaryIndexKind,
    SecondaryIndexLease, SecondaryIndexOperation, SecondaryIndexPlan, SecondaryIndexRequest,
    SecondaryIndexSpec,
};

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
