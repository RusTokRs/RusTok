use async_trait::async_trait;
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::sync::OnceLock;
use std::time::Instant;
use tokio::task::JoinSet;
use tracing::warn;
use uuid::Uuid;

use crate::error::IndexResult;

#[derive(Clone, Debug)]
pub struct IndexerRuntimeConfig {
    reindex_parallelism: usize,
    reindex_entity_budget: usize,
    reindex_yield_every: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReindexRunStats {
    pub discovered: u64,
    pub scheduled: u64,
    pub completed: u64,
    pub failed: u64,
    pub panicked: u64,
    pub truncated: u64,
}

impl Default for IndexerRuntimeConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl IndexerRuntimeConfig {
    pub fn new(
        reindex_parallelism: usize,
        reindex_entity_budget: usize,
        reindex_yield_every: u64,
    ) -> Self {
        Self {
            reindex_parallelism: reindex_parallelism.max(1),
            reindex_entity_budget: reindex_entity_budget.max(1),
            reindex_yield_every: reindex_yield_every.max(1),
        }
    }

    pub fn from_env() -> Self {
        Self::new(
            env_or_default("RUSTOK_INDEX_REINDEX_MAX_PARALLELISM", 4usize),
            env_or_default("RUSTOK_INDEX_REINDEX_ENTITY_BUDGET", 500usize),
            env_or_default("RUSTOK_INDEX_REINDEX_YIELD_EVERY", 50u64),
        )
    }

    pub fn load() -> Self {
        static CONFIG: OnceLock<IndexerRuntimeConfig> = OnceLock::new();
        CONFIG.get_or_init(Self::from_env).clone()
    }
}

/// Context for indexing operations
#[derive(Clone)]
pub struct IndexerContext {
    pub db: DatabaseConnection,
    pub tenant_id: Uuid,
    runtime: IndexerRuntimeConfig,
}

impl IndexerContext {
    pub fn new(db: DatabaseConnection, tenant_id: Uuid) -> Self {
        Self::new_with_runtime(db, tenant_id, IndexerRuntimeConfig::load())
    }

    pub fn new_with_runtime(
        db: DatabaseConnection,
        tenant_id: Uuid,
        runtime: IndexerRuntimeConfig,
    ) -> Self {
        Self {
            db,
            tenant_id,
            runtime,
        }
    }

    pub fn reindex_parallelism(&self) -> usize {
        self.runtime.reindex_parallelism
    }

    pub fn reindex_entity_budget(&self) -> usize {
        self.runtime.reindex_entity_budget
    }

    pub fn reindex_yield_every(&self) -> u64 {
        self.runtime.reindex_yield_every
    }
}

/// Trait for indexers that update denormalized tables
#[async_trait]
pub trait Indexer: Send + Sync {
    /// Indexer name for logging
    fn name(&self) -> &'static str;

    /// Index a single entity by ID (all locales)
    async fn index_one(&self, ctx: &IndexerContext, entity_id: Uuid) -> IndexResult<()>;

    /// Remove entity from index
    async fn remove_one(&self, ctx: &IndexerContext, entity_id: Uuid) -> IndexResult<()>;

    /// Reindex all entities for a tenant
    async fn reindex_all(&self, ctx: &IndexerContext) -> IndexResult<u64>;
}

/// Trait for locale-aware indexers
#[async_trait]
pub trait LocaleIndexer: Indexer {
    /// Index entity for specific locale
    async fn index_locale(
        &self,
        ctx: &IndexerContext,
        entity_id: Uuid,
        locale: &str,
    ) -> IndexResult<()>;

    /// Remove entity from index for specific locale
    async fn remove_locale(
        &self,
        ctx: &IndexerContext,
        entity_id: Uuid,
        locale: &str,
    ) -> IndexResult<()>;
}

pub async fn run_bounded_reindex<I>(
    indexer: I,
    ctx: &IndexerContext,
    mut ids: Vec<Uuid>,
    operation: &'static str,
) -> ReindexRunStats
where
    I: Indexer + Clone + Send + Sync + 'static,
{
    let indexer_name = indexer.name();
    metrics::record_index_reindex_runtime_config(
        indexer_name,
        ctx.reindex_parallelism(),
        ctx.reindex_entity_budget(),
        ctx.reindex_yield_every(),
    );
    metrics::record_index_reindex_run(indexer_name, operation, "started");

    let started_at = Instant::now();
    let discovered = ids.len();
    let budget = ctx.reindex_entity_budget().min(discovered);
    let mut stats = ReindexRunStats {
        discovered: discovered as u64,
        truncated: discovered.saturating_sub(budget) as u64,
        ..ReindexRunStats::default()
    };

    if discovered > budget {
        warn!(
            indexer = indexer_name,
            tenant_id = %ctx.tenant_id,
            operation,
            discovered,
            budget,
            "Reindex workload truncated by tenant budget"
        );
        ids.truncate(budget);
    }

    stats.scheduled = ids.len() as u64;
    let effective_parallelism = ctx.reindex_parallelism().min(ids.len().max(1));
    let yield_every = ctx.reindex_yield_every();
    let mut join_set = JoinSet::new();
    let mut drain_count = 0u64;

    for entity_id in ids {
        while join_set.len() >= effective_parallelism {
            drain_reindex_task(&mut join_set, indexer_name, ctx, operation, &mut stats).await;
            drain_count += 1;
            if drain_count.is_multiple_of(yield_every) {
                tokio::task::yield_now().await;
            }
        }

        let task_indexer = indexer.clone();
        let task_ctx = ctx.clone();
        join_set.spawn(async move {
            (
                entity_id,
                task_indexer.index_one(&task_ctx, entity_id).await,
            )
        });
    }

    while !join_set.is_empty() {
        drain_reindex_task(&mut join_set, indexer_name, ctx, operation, &mut stats).await;
        drain_count += 1;
        if drain_count.is_multiple_of(yield_every) {
            tokio::task::yield_now().await;
        }
    }

    metrics::record_index_reindex_entities(indexer_name, operation, "discovered", stats.discovered);
    metrics::record_index_reindex_entities(indexer_name, operation, "scheduled", stats.scheduled);
    metrics::record_index_reindex_entities(indexer_name, operation, "completed", stats.completed);
    metrics::record_index_reindex_entities(indexer_name, operation, "failed", stats.failed);
    metrics::record_index_reindex_entities(indexer_name, operation, "panicked", stats.panicked);
    metrics::record_index_reindex_entities(indexer_name, operation, "truncated", stats.truncated);
    metrics::record_index_reindex_duration(
        indexer_name,
        operation,
        started_at.elapsed().as_secs_f64(),
    );
    metrics::record_index_reindex_run(indexer_name, operation, "completed");

    stats
}

async fn drain_reindex_task(
    join_set: &mut JoinSet<(Uuid, IndexResult<()>)>,
    indexer_name: &'static str,
    ctx: &IndexerContext,
    operation: &'static str,
    stats: &mut ReindexRunStats,
) {
    match join_set.join_next().await {
        Some(Ok((entity_id, Ok(())))) => {
            let _ = entity_id;
            stats.completed += 1;
        }
        Some(Ok((entity_id, Err(err)))) => {
            stats.failed += 1;
            warn!(
                indexer = indexer_name,
                tenant_id = %ctx.tenant_id,
                operation,
                entity_id = %entity_id,
                error = %err,
                "Failed to reindex entity"
            );
        }
        Some(Err(err)) => {
            stats.panicked += 1;
            warn!(
                indexer = indexer_name,
                tenant_id = %ctx.tenant_id,
                operation,
                error = ?err,
                "Reindex task aborted before completion"
            );
        }
        None => {}
    }
}

fn env_or_default<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr + Copy,
{
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<T>().ok())
        .unwrap_or(default)
}
