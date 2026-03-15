//! RusToK Index - CQRS Read Model
//!
//! Denormalized indexes for fast reads and full-text search.

use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod content;
pub mod error;
pub mod product;
pub mod search;
pub mod traits;

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
        "CQRS Read Model (Fast Search)"
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
        Vec::new()
    }
}

#[cfg(test)]
mod contract_tests;
