use std::cmp;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use sea_orm::{
    sea_query::{LockBehavior, LockType},
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde_json::from_value;
use uuid::Uuid;

use rustok_core::events::EventTransport;
use rustok_core::{Error, Result};
use rustok_events::EventEnvelope;

use crate::entity;
use crate::entity::SysEventStatus;

#[derive(Clone, Debug)]
pub struct RelayConfig {
    pub batch_size: u64,
    pub max_attempts: i32,
    pub backoff_base: Duration,
    pub backoff_max: Duration,
    pub max_concurrency: usize,
    pub claim_ttl: Duration,
    pub worker_id: String,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            max_attempts: 5,
            backoff_base: Duration::from_secs(1),
            backoff_max: Duration::from_secs(60),
            max_concurrency: 8,
            claim_ttl: Duration::from_secs(60),
            worker_id: format!("relay-{}", Uuid::new_v4()),
        }
    }
}

#[derive(Debug, Default)]
struct RelayMetrics {
    success_total: AtomicU64,
    failure_total: AtomicU64,
    retry_total: AtomicU64,
    dlq_total: AtomicU64,
    latency_ms_total: AtomicU64,
    processed_total: AtomicU64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RelayMetricsSnapshot {
    pub success_total: u64,
    pub failure_total: u64,
    pub retry_total: u64,
    pub dlq_total: u64,
    pub latency_ms_total: u64,
    pub processed_total: u64,
}

impl RelayMetrics {
    fn snapshot(&self) -> RelayMetricsSnapshot {
        RelayMetricsSnapshot {
            success_total: self.success_total.load(Ordering::Relaxed),
            failure_total: self.failure_total.load(Ordering::Relaxed),
            retry_total: self.retry_total.load(Ordering::Relaxed),
            dlq_total: self.dlq_total.load(Ordering::Relaxed),
            latency_ms_total: self.latency_ms_total.load(Ordering::Relaxed),
            processed_total: self.processed_total.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone)]
pub struct OutboxRelay {
    db: DatabaseConnection,
    target: Arc<dyn EventTransport>,
    config: RelayConfig,
    metrics: Arc<RelayMetrics>,
}

impl std::fmt::Debug for OutboxRelay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutboxRelay")
            .field("config", &self.config)
            .field("metrics", &self.metrics.snapshot())
            .finish_non_exhaustive()
    }
}

impl OutboxRelay {
    pub fn new(db: DatabaseConnection, target: Arc<dyn EventTransport>) -> Self {
        Self {
            db,
            target,
            config: RelayConfig::default(),
            metrics: Arc::new(RelayMetrics::default()),
        }
    }

    pub fn with_config(mut self, config: RelayConfig) -> Self {
        self.config = config;
        self
    }

    pub fn metrics(&self) -> RelayMetricsSnapshot {
        self.metrics.snapshot()
    }

    pub async fn run(&self) -> Result<()> {
        loop {
            match self.process_pending_once().await {
                Ok(count) => {
                    if count == 0 {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
                Err(e) => {
                    tracing::error!("Relay processing error: {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    pub async fn process_pending_once(&self) -> Result<usize> {
        let claimed = self.claim_batch().await?;
        let claimed_count = claimed.len();
        let max_concurrency = self.config.max_concurrency.max(1);
        let mut tasks = tokio::task::JoinSet::new();
        let mut first_error = None;

        for model in claimed {
            while tasks.len() >= max_concurrency {
                Self::collect_dispatch_result(&mut tasks, &mut first_error).await;
            }

            let relay = self.clone();
            tasks.spawn(async move { relay.process_claimed_event(&model).await });
        }

        while !tasks.is_empty() {
            Self::collect_dispatch_result(&mut tasks, &mut first_error).await;
        }

        if let Some(error) = first_error {
            return Err(error);
        }

        Ok(claimed_count)
    }

    async fn claim_batch(&self) -> Result<Vec<entity::Model>> {
        let now = Utc::now();
        let stale_before = now
            - chrono::Duration::from_std(self.config.claim_ttl)
                .unwrap_or_else(|_| chrono::Duration::seconds(60));
        let worker_id = self.config.worker_id.clone();

        let txn = self.db.begin().await?;
        let candidates_query = entity::Entity::find()
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(
                Condition::any()
                    .add(entity::Column::NextAttemptAt.is_null())
                    .add(entity::Column::NextAttemptAt.lte(now)),
            )
            .filter(
                Condition::any()
                    .add(entity::Column::ClaimedAt.is_null())
                    .add(entity::Column::ClaimedAt.lte(stale_before)),
            )
            .order_by_asc(entity::Column::CreatedAt)
            .limit(self.config.batch_size);
        let candidates = if self.db.get_database_backend() == DatabaseBackend::Postgres {
            candidates_query
                .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
                .all(&txn)
                .await?
        } else {
            candidates_query.all(&txn).await?
        };

        let candidate_ids: Vec<Uuid> = candidates.iter().map(|m| m.id).collect();
        if candidate_ids.is_empty() {
            txn.commit().await?;
            return Ok(Vec::new());
        }

        entity::Entity::update_many()
            .filter(entity::Column::Id.is_in(candidate_ids.clone()))
            .filter(
                Condition::any()
                    .add(entity::Column::ClaimedAt.is_null())
                    .add(entity::Column::ClaimedAt.lte(stale_before)),
            )
            .set(entity::ActiveModel {
                claimed_by: Set(Some(worker_id.clone())),
                claimed_at: Set(Some(now)),
                ..Default::default()
            })
            .exec(&txn)
            .await?;

        let claimed = entity::Entity::find()
            .filter(entity::Column::Id.is_in(candidate_ids))
            .filter(entity::Column::ClaimedBy.eq(worker_id))
            .filter(entity::Column::ClaimedAt.is_not_null())
            .all(&txn)
            .await?;

        txn.commit().await?;
        Ok(claimed)
    }

    async fn collect_dispatch_result(
        tasks: &mut tokio::task::JoinSet<Result<()>>,
        first_error: &mut Option<Error>,
    ) {
        match tasks.join_next().await {
            Some(Ok(Ok(()))) | None => {}
            Some(Ok(Err(error))) => {
                if first_error.is_none() {
                    *first_error = Some(error);
                }
            }
            Some(Err(error)) => {
                tracing::error!(error = ?error, "Outbox relay dispatch task failed");
                if first_error.is_none() {
                    *first_error = Some(Error::External(format!(
                        "outbox relay dispatch task failed: {error}"
                    )));
                }
            }
        }
    }

    async fn process_claimed_event(&self, model: &entity::Model) -> Result<()> {
        let started = Instant::now();
        let event_id = model.id;
        let envelope: EventEnvelope = from_value(model.payload.clone())?;

        let publish_result = self.target.publish(envelope).await;
        let elapsed_ms = started.elapsed().as_millis() as u64;
        self.metrics
            .latency_ms_total
            .fetch_add(elapsed_ms, Ordering::Relaxed);
        self.metrics.processed_total.fetch_add(1, Ordering::Relaxed);

        match publish_result {
            Ok(()) => {
                tracing::info!(event_id = %event_id, latency_ms = elapsed_ms, "Outbox event dispatched");
                self.mark_dispatched(event_id).await?;
                self.metrics.success_total.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(err) => {
                tracing::warn!(event_id = %event_id, error = %err, "Outbox event dispatch failed");
                self.metrics.failure_total.fetch_add(1, Ordering::Relaxed);
                self.mark_failed_attempt(model, err).await
            }
        }
    }

    async fn mark_dispatched(&self, event_id: Uuid) -> Result<()> {
        let mut active: entity::ActiveModel = entity::Entity::find_by_id(event_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("sys_event {event_id}")))?
            .into();
        active.status = Set(SysEventStatus::Dispatched);
        active.dispatched_at = Set(Some(Utc::now()));
        active.claimed_by = Set(None);
        active.claimed_at = Set(None);
        active.last_error = Set(None);
        active.next_attempt_at = Set(None);
        active.update(&self.db).await?;
        Ok(())
    }

    async fn mark_failed_attempt(&self, model: &entity::Model, error: Error) -> Result<()> {
        let retry_count = model.retry_count + 1;
        let mut active: entity::ActiveModel = entity::Entity::find_by_id(model.id)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("sys_event {}", model.id)))?
            .into();

        active.retry_count = Set(retry_count);
        active.last_error = Set(Some(error.to_string()));
        active.claimed_by = Set(None);
        active.claimed_at = Set(None);

        if retry_count >= self.config.max_attempts {
            active.status = Set(SysEventStatus::Failed);
            active.next_attempt_at = Set(None);
            tracing::error!(event_id = %model.id, retry_count, "Outbox event moved to DLQ (failed)");
            self.metrics.dlq_total.fetch_add(1, Ordering::Relaxed);
        } else {
            let next_attempt_at = Utc::now() + self.backoff_duration(retry_count);
            active.status = Set(SysEventStatus::Pending);
            active.next_attempt_at = Set(Some(next_attempt_at));
            tracing::info!(
                event_id = %model.id,
                retry_count,
                next_attempt_at = %next_attempt_at,
                "Outbox event scheduled for retry"
            );
            self.metrics.retry_total.fetch_add(1, Ordering::Relaxed);
        }

        active.update(&self.db).await?;
        Ok(())
    }

    fn backoff_duration(&self, retry_count: i32) -> chrono::Duration {
        let attempt = retry_count.saturating_sub(1) as u32;
        let factor = 2u128.pow(cmp::min(attempt, 16));
        let millis = self.config.backoff_base.as_millis().saturating_mul(factor);
        let max_ms = self.config.backoff_max.as_millis();
        let bounded = cmp::min(millis, max_ms) as i64;
        chrono::Duration::milliseconds(bounded)
    }
}
