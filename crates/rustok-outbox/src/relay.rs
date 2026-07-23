use std::cmp;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use chrono::Utc;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
    sea_query::{LockBehavior, LockType},
};
use serde_json::from_value;
use uuid::Uuid;

use rustok_core::events::EventTransport;
use rustok_core::{Error, Result};
use rustok_events::{ContractEventEnvelope, EventEnvelope};

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

enum RelayEnvelope {
    Root(EventEnvelope),
    Contract(ContractEventEnvelope),
}

impl RelayEnvelope {
    fn event_type(&self) -> &str {
        match self {
            Self::Root(envelope) => envelope.event_type.as_str(),
            Self::Contract(envelope) => envelope.event_type(),
        }
    }

    fn schema_version(&self) -> u16 {
        match self {
            Self::Root(envelope) => envelope.schema_version,
            Self::Contract(envelope) => envelope.schema_version(),
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
            match self.process_pending_once(None).await {
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

    pub async fn process_pending_once(&self, max_batch_hint: Option<u64>) -> Result<usize> {
        self.validate_config()?;
        if max_batch_hint == Some(0) {
            return Err(Error::Validation(
                "outbox max_batch_hint must be greater than zero".to_string(),
            ));
        }
        let batch_size = max_batch_hint
            .unwrap_or(self.config.batch_size)
            .min(self.config.batch_size);
        let claimed = self.claim_batch(batch_size).await?;
        let claimed_count = claimed.len();
        let mut tasks = tokio::task::JoinSet::new();
        let mut first_error = None;

        for model in claimed {
            while tasks.len() >= self.config.max_concurrency {
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

    fn validate_config(&self) -> Result<()> {
        if self.config.batch_size == 0 {
            return Err(Error::Validation(
                "outbox relay batch_size must be greater than zero".to_string(),
            ));
        }
        if self.config.max_attempts <= 0 {
            return Err(Error::Validation(
                "outbox relay max_attempts must be greater than zero".to_string(),
            ));
        }
        if self.config.max_concurrency == 0 {
            return Err(Error::Validation(
                "outbox relay max_concurrency must be greater than zero".to_string(),
            ));
        }
        if self.config.claim_ttl.is_zero() {
            return Err(Error::Validation(
                "outbox relay claim_ttl must be greater than zero".to_string(),
            ));
        }
        if self.config.worker_id.trim().is_empty() {
            return Err(Error::Validation(
                "outbox relay worker_id must not be empty".to_string(),
            ));
        }
        Ok(())
    }

    async fn claim_batch(&self, batch_size: u64) -> Result<Vec<entity::Model>> {
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
            .limit(batch_size);
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
            .set(entity::ActiveModel {
                claimed_by: Set(Some(worker_id.clone())),
                claimed_at: Set(Some(now)),
                ..Default::default()
            })
            .exec(&txn)
            .await?;

        let claimed = entity::Entity::find()
            .filter(entity::Column::Id.is_in(candidate_ids))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.eq(worker_id))
            .filter(entity::Column::ClaimedAt.eq(now))
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
        let envelope = match Self::decode_envelope(model) {
            Ok(envelope) => envelope,
            Err(error) => {
                let elapsed_ms = started.elapsed().as_millis() as u64;
                self.record_processed(elapsed_ms, false);
                tracing::warn!(
                    event_id = %event_id,
                    error = %error,
                    "Outbox event payload is invalid"
                );
                return self.mark_failed_attempt(model, error).await;
            }
        };

        let publish_result = match envelope {
            RelayEnvelope::Root(envelope) => self.target.publish(envelope).await,
            RelayEnvelope::Contract(envelope) => self.target.publish_contract(envelope).await,
        };
        let elapsed_ms = started.elapsed().as_millis() as u64;

        match publish_result {
            Ok(()) => {
                tracing::info!(event_id = %event_id, latency_ms = elapsed_ms, "Outbox event dispatched");
                self.mark_dispatched(model).await?;
                self.record_processed(elapsed_ms, true);
                Ok(())
            }
            Err(err) => {
                tracing::warn!(event_id = %event_id, error = %err, "Outbox event dispatch failed");
                self.record_processed(elapsed_ms, false);
                self.mark_failed_attempt(model, err).await
            }
        }
    }

    fn decode_envelope(model: &entity::Model) -> Result<RelayEnvelope> {
        let envelope = match from_value::<ContractEventEnvelope>(model.payload.clone()) {
            Ok(envelope) => {
                envelope
                    .validate_registered_schema()
                    .map_err(|error| Error::Validation(error.to_string()))?;
                RelayEnvelope::Contract(envelope)
            }
            Err(contract_error) => match from_value::<EventEnvelope>(model.payload.clone()) {
                Ok(envelope) => {
                    envelope
                        .validate_registered_schema()
                        .map_err(|error| Error::Validation(error.to_string()))?;
                    RelayEnvelope::Root(envelope)
                }
                Err(root_error) => {
                    tracing::debug!(
                        contract_error = %contract_error,
                        root_error = %root_error,
                        "Outbox payload matched neither contract nor root envelope"
                    );
                    return Err(Error::Serialization(root_error));
                }
            },
        };

        let envelope_schema_version = i16::try_from(envelope.schema_version()).map_err(|_| {
            Error::Validation(format!(
                "outbox envelope schema version {} exceeds database SMALLINT range",
                envelope.schema_version()
            ))
        })?;
        if model.event_type != envelope.event_type()
            || model.schema_version != envelope_schema_version
        {
            return Err(Error::Validation(format!(
                "outbox row metadata does not match envelope: row=`{}`/{} envelope=`{}`/{}",
                model.event_type,
                model.schema_version,
                envelope.event_type(),
                envelope.schema_version()
            )));
        }
        Ok(envelope)
    }

    fn record_processed(&self, elapsed_ms: u64, succeeded: bool) {
        self.metrics
            .latency_ms_total
            .fetch_add(elapsed_ms, Ordering::Relaxed);
        self.metrics.processed_total.fetch_add(1, Ordering::Relaxed);
        if succeeded {
            self.metrics.success_total.fetch_add(1, Ordering::Relaxed);
        } else {
            self.metrics.failure_total.fetch_add(1, Ordering::Relaxed);
        }
    }

    async fn mark_dispatched(&self, model: &entity::Model) -> Result<()> {
        let result = entity::Entity::update_many()
            .filter(entity::Column::Id.eq(model.id))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.eq(model.claimed_by.clone()))
            .filter(entity::Column::ClaimedAt.eq(model.claimed_at))
            .set(entity::ActiveModel {
                status: Set(SysEventStatus::Dispatched),
                dispatched_at: Set(Some(Utc::now())),
                claimed_by: Set(None),
                claimed_at: Set(None),
                last_error: Set(None),
                next_attempt_at: Set(None),
                ..Default::default()
            })
            .exec(&self.db)
            .await?;
        self.ensure_claim_owned(model.id, result.rows_affected)
    }

    async fn mark_failed_attempt(&self, model: &entity::Model, error: Error) -> Result<()> {
        let retry_count = model.retry_count.saturating_add(1);
        let (status, next_attempt_at, moved_to_dlq) = if retry_count >= self.config.max_attempts {
            (SysEventStatus::Failed, None, true)
        } else {
            (
                SysEventStatus::Pending,
                Some(Utc::now() + self.backoff_duration(retry_count)),
                false,
            )
        };

        let result = entity::Entity::update_many()
            .filter(entity::Column::Id.eq(model.id))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.eq(model.claimed_by.clone()))
            .filter(entity::Column::ClaimedAt.eq(model.claimed_at))
            .set(entity::ActiveModel {
                retry_count: Set(retry_count),
                last_error: Set(Some(error.to_string())),
                claimed_by: Set(None),
                claimed_at: Set(None),
                status: Set(status),
                next_attempt_at: Set(next_attempt_at),
                ..Default::default()
            })
            .exec(&self.db)
            .await?;
        self.ensure_claim_owned(model.id, result.rows_affected)?;

        if moved_to_dlq {
            tracing::error!(event_id = %model.id, retry_count, "Outbox event moved to DLQ (failed)");
            self.metrics.dlq_total.fetch_add(1, Ordering::Relaxed);
        } else {
            tracing::info!(
                event_id = %model.id,
                retry_count,
                next_attempt_at = ?next_attempt_at,
                "Outbox event scheduled for retry"
            );
            self.metrics.retry_total.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    fn ensure_claim_owned(&self, event_id: Uuid, rows_affected: u64) -> Result<()> {
        if rows_affected == 1 {
            Ok(())
        } else {
            Err(Error::External(format!(
                "outbox event {event_id} claim was lost before completion"
            )))
        }
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

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use sea_orm::{ActiveModelTrait, Database, EntityTrait, Set};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use rustok_core::events::{EventTransport, ReliabilityLevel};
    use rustok_events::MarketplaceListingEvent;

    use super::*;
    use crate::migration::SysEventsMigration;

    struct RejectUnexpectedPublish;

    #[async_trait]
    impl EventTransport for RejectUnexpectedPublish {
        async fn publish(&self, _envelope: EventEnvelope) -> Result<()> {
            Err(Error::External(
                "invalid payload must not reach transport".to_string(),
            ))
        }

        fn reliability_level(&self) -> ReliabilityLevel {
            ReliabilityLevel::InMemory
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    struct CaptureContractPublish {
        event_types: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl EventTransport for CaptureContractPublish {
        async fn publish(&self, _envelope: EventEnvelope) -> Result<()> {
            Err(Error::External(
                "contract event must not use root publish".to_string(),
            ))
        }

        async fn publish_contract(&self, envelope: ContractEventEnvelope) -> Result<()> {
            self.event_types
                .lock()
                .expect("capture lock")
                .push(envelope.event_type().to_string());
            Ok(())
        }

        fn reliability_level(&self) -> ReliabilityLevel {
            ReliabilityLevel::InMemory
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    async fn database() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        SysEventsMigration
            .up(&SchemaManager::new(&db))
            .await
            .expect("sys_events migration");
        db
    }

    #[tokio::test]
    async fn invalid_payload_reaches_dlq_instead_of_remaining_claimed() {
        let db = database().await;
        let event_id = Uuid::new_v4();
        entity::ActiveModel {
            id: Set(event_id),
            event_type: Set("invalid.event".to_string()),
            schema_version: Set(1),
            payload: Set(serde_json::json!({ "not": "an envelope" })),
            status: Set(SysEventStatus::Pending),
            retry_count: Set(0),
            next_attempt_at: Set(None),
            last_error: Set(None),
            claimed_by: Set(None),
            claimed_at: Set(None),
            created_at: Set(Utc::now()),
            dispatched_at: Set(None),
        }
        .insert(&db)
        .await
        .expect("insert poison event");

        let relay = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish)).with_config(
            RelayConfig {
                max_attempts: 1,
                worker_id: "poison-test".to_string(),
                ..RelayConfig::default()
            },
        );

        assert_eq!(relay.process_pending_once(Some(1)).await.expect("relay"), 1);

        let event = entity::Entity::find_by_id(event_id)
            .one(&db)
            .await
            .expect("query")
            .expect("event");
        assert_eq!(event.status, SysEventStatus::Failed);
        assert_eq!(event.retry_count, 1);
        assert!(event.claimed_by.is_none());
        assert!(event.claimed_at.is_none());
        assert!(
            event
                .last_error
                .as_deref()
                .is_some_and(|message| message.contains("Serialization error"))
        );
    }

    #[tokio::test]
    async fn sealed_contract_envelope_is_dispatched_without_root_deserialization() {
        let db = database().await;
        let envelope = ContractEventEnvelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            MarketplaceListingEvent::MarketplaceListingPublished {
                listing_id: Uuid::new_v4(),
                seller_id: Uuid::new_v4(),
                master_product_id: Uuid::new_v4(),
                master_variant_id: Uuid::new_v4(),
                market_slug: "us-market".to_string(),
                channel_slug: "web-store".to_string(),
                terms_version: 2,
            },
        )
        .expect("valid contract envelope");
        let event_id = envelope.id();
        entity::ActiveModel {
            id: Set(event_id),
            event_type: Set(envelope.event_type().to_string()),
            schema_version: Set(i16::try_from(envelope.schema_version()).unwrap()),
            payload: Set(serde_json::to_value(&envelope).unwrap()),
            status: Set(SysEventStatus::Pending),
            retry_count: Set(0),
            next_attempt_at: Set(None),
            last_error: Set(None),
            claimed_by: Set(None),
            claimed_at: Set(None),
            created_at: Set(Utc::now()),
            dispatched_at: Set(None),
        }
        .insert(&db)
        .await
        .expect("insert contract event");

        let target = Arc::new(CaptureContractPublish {
            event_types: Mutex::new(Vec::new()),
        });
        let relay = OutboxRelay::new(db.clone(), target.clone()).with_config(RelayConfig {
            worker_id: "contract-test".to_string(),
            ..RelayConfig::default()
        });
        assert_eq!(relay.process_pending_once(Some(1)).await.expect("relay"), 1);

        let event = entity::Entity::find_by_id(event_id)
            .one(&db)
            .await
            .expect("query")
            .expect("event");
        assert_eq!(event.status, SysEventStatus::Dispatched);
        let event_types = target.event_types.lock().expect("capture lock").clone();
        assert_eq!(
            event_types,
            vec!["marketplace.listing.published".to_string()]
        );
    }

    #[tokio::test]
    async fn stale_worker_cannot_complete_a_reclaimed_event() {
        let db = database().await;
        let event_id = Uuid::new_v4();
        entity::ActiveModel {
            id: Set(event_id),
            event_type: Set("invalid.event".to_string()),
            schema_version: Set(1),
            payload: Set(serde_json::json!({ "not": "an envelope" })),
            status: Set(SysEventStatus::Pending),
            retry_count: Set(0),
            next_attempt_at: Set(None),
            last_error: Set(None),
            claimed_by: Set(Some("old-worker".to_string())),
            claimed_at: Set(Some(Utc::now())),
            created_at: Set(Utc::now()),
            dispatched_at: Set(None),
        }
        .insert(&db)
        .await
        .expect("insert claimed event");

        let old_model = entity::Entity::find_by_id(event_id)
            .one(&db)
            .await
            .expect("query")
            .expect("event");
        let mut reclaimed: entity::ActiveModel = old_model.clone().into();
        reclaimed.claimed_by = Set(Some("new-worker".to_string()));
        reclaimed.claimed_at = Set(Some(Utc::now() + chrono::Duration::seconds(1)));
        reclaimed.update(&db).await.expect("reclaim event");

        let relay = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish)).with_config(
            RelayConfig {
                worker_id: "old-worker".to_string(),
                ..RelayConfig::default()
            },
        );
        assert!(relay.mark_dispatched(&old_model).await.is_err());

        let event = entity::Entity::find_by_id(event_id)
            .one(&db)
            .await
            .expect("query")
            .expect("event");
        assert_eq!(event.status, SysEventStatus::Pending);
        assert_eq!(event.claimed_by.as_deref(), Some("new-worker"));
    }
}
