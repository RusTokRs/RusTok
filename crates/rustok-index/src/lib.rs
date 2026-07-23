//! RusToK Index - cross-module relational Index Engine.
//!
//! The database-independent engine core lives under [`domain`]. Source-specific
//! indexers and migrations remain temporarily while M0 removes the legacy storage
//! implementation in controlled steps.

use async_trait::async_trait;
use rustok_core::{
    MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry, ModuleKind,
    ModuleRuntimeExtensions, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

pub mod content;
pub mod domain;
pub mod error;
pub mod flex;
pub mod migrations;
pub mod product;
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

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        let runtime = ctx
            .extensions
            .get::<IndexerRuntimeConfig>()
            .cloned()
            .unwrap_or_else(IndexerRuntimeConfig::load);
        registry.register(content::ContentIndexer::with_runtime(
            ctx.db.clone(),
            runtime.clone(),
        ));
        registry.register(flex::FlexIndexer::with_runtime(
            ctx.db.clone(),
            runtime.clone(),
        ));
        registry.register(product::ProductIndexer::with_runtime(
            ctx.db.clone(),
            runtime,
        ));
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        extensions.get_or_insert_with(IndexerRuntimeConfig::load);
        Ok(())
    }
}

impl MigrationSource for IndexModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod contract_tests;
