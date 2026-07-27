//! RusToK Index - cross-module relational Index Engine.
//!
//! The active implementation contains the database-independent generic engine
//! core under [`domain`] and [`application`], the canonical M3 PostgreSQL
//! storage-schema migrations, atomic mutation persistence, and durable
//! schema-application leases.

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
    MutationApplyOutcome, MutationDelivery, MutationStorageError, PostgresMutationStore,
    PostgresSchemaLeaseStore, SchemaApplicationLease, SchemaApplicationLeaseRequest,
    SchemaLeaseAcquireOutcome, SchemaLeaseError,
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
