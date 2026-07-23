//! RusToK Index - cross-module relational Index Engine.
//!
//! The active implementation is the database-independent generic engine core
//! under [`domain`]. Storage, ingestion, rebuild, and query infrastructure are
//! introduced only through the milestones in the live implementation plan.

use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleKind, ModuleRuntimeExtensions, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod domain;
pub mod error;
pub mod traits;

pub use domain::*;
pub use error::{IndexError, IndexResult};
pub use traits::{Indexer, IndexerContext, IndexerRuntimeConfig, LocaleIndexer};

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
        // Temporary host compatibility only. M0 removes this legacy runtime
        // configuration after server composition stops inserting it.
        extensions.get_or_insert_with(IndexerRuntimeConfig::load);
        Ok(())
    }
}

impl MigrationSource for IndexModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[cfg(test)]
mod contract_tests;
