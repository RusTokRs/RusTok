use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
    sea_query::{LockBehavior, LockType},
};
use serde_json::from_value;
use uuid::Uuid;

use rustok_core::events::EventTransport;
use rustok_core::{Error, Result};
use rustok_events::{ContractEventEnvelope, EventEnvelope};

use crate::entity;
use crate::entity::SysEventStatus;
use crate::ports::OutboxRelayRunOnceProjection;

/// Upper bound on rows claimed in one relay pass. Matches the server
/// runtime cap so a library caller cannot materialise an unbounded batch.
pub const MAX_RELAY_BATCH_SIZE: u64 = 10_000;
/// Upper bound on simultaneous dispatches across one relay's clones.
pub const MAX_RELAY_CONCURRENCY: usize = 1_024;
const MAX_RELAY_INTERVAL: Duration = Duration::from_secs(3650 * 86_400);

#[derive(Clone, Debug)]
pub struct RelayConfig {
    pub batch_size: u64,
    pub max_attempts: i32,
    pub backoff_base: Duration,
    pub backoff_max: Duration,
    pub max_concurrency: usize,
    pub claim_ttl: Duration,
    /// Hard bound on one transport dispatch. Must stay strictly below
    /// `claim_ttl`: a dispatch that outlives its claim lease can be
    /// reclaimed and delivered a second time while the first attempt is
    /// still running, and its completion update is then rejected.
    pub publish_timeout: Duration,
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
            publish_timeout: Duration::from_secs(30),
            worker_id: format!("relay-{}", Uuid::new_v4()),
        }
    }
}

impl RelayConfig {
    pub fn validate(&self) -> Result<()> {
        if self.batch_size == 0 || self.batch_size > MAX_RELAY_BATCH_SIZE {
            return Err(Error::Validation(format!(
                "outbox relay batch_size must be between 1 and {MAX_RELAY_BATCH_SIZE}"
            )));
        }
        if self.max_concurrency > MAX_RELAY_CONCURRENCY {
            return Err(Error::Validation(format!(
                "outbox relay max_concurrency must not exceed {MAX_RELAY_CONCURRENCY}"
            )));
        }
        if self.publish_timeout.is_zero() {
            return Err(Error::Validation(
                "outbox relay publish_timeout must be greater than zero".to_string(),
            ));
        }
        if self.claim_ttl.saturating_sub(self.publish_timeout) <= Duration::from_millis(1) {
            return Err(Error::Validation(
                "outbox relay claim_ttl must exceed publish_timeout by more than one millisecond"
                    .to_string(),
            ));
        }
        if self.max_attempts <= 0 {
            return Err(Error::Validation(
                "outbox relay max_attempts must be greater than zero".to_string(),
            ));
        }
        if self.max_concurrency == 0 {
            return Err(Error::Validation(
                "outbox relay max_concurrency must be greater than zero".to_string(),
            ));
        }
        if self.claim_ttl.is_zero() {
            return Err(Error::Validation(
                "outbox relay claim_ttl must be greater than zero".to_string(),
            ));
        }
        // `claimed_by` is capped at 128 bytes and also carries a per-claim
        // fencing nonce (`{worker_id}:{uuid}`), so the worker id itself must
        // leave room for it.
        if self.worker_id.trim().is_empty() || self.worker_id.len() > 64 {
            return Err(Error::Validation(
                "outbox relay worker_id must contain 1 to 64 bytes".to_string(),
            ));
        }
        if self.backoff_base < Duration::from_millis(1) {
            return Err(Error::Validation(
                "outbox relay backoff_base must be at least one millisecond".to_string(),
            ));
        }
        if self.backoff_max < self.backoff_base {
            return Err(Error::Validation(
                "outbox relay backoff_max must be greater than or equal to backoff_base"
                    .to_string(),
            ));
        }
        if self.claim_ttl > MAX_RELAY_INTERVAL || self.backoff_max > MAX_RELAY_INTERVAL {
            return Err(Error::Validation("outbox relay intervals must not exceed 3650 days".to_string()));
        }
        Ok(())
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

enum DispatchOutcome {
    Succeeded,
    Retried,
    DeadLettered,
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

    fn identity(&self) -> (Uuid, Uuid) {
        match self {
            Self::Root(envelope) => (envelope.id, envelope.tenant_id),
            Self::Contract(envelope) => (envelope.id(), envelope.tenant_id()),
        }
    }

    fn schema_version(&self) -> u16 {
        match self {
            Self::Root(envelope) => envelope.schema_version,
            Self::Contract(envelope) => envelope.schema_version(),
        }
    }
}

/// Polls `sys_events`, claims due rows, and dispatches them to the target
/// transport with retry/backoff and DLQ semantics.
///
/// Delivery is at-least-once and **unordered**: rows are claimed in
/// `created_at` order but dispatched with up to
/// `RelayConfig::max_concurrency` in flight, so events belonging to one
/// tenant or aggregate may reach the transport out of order. Consumers
/// must be idempotent and tolerant of reordering and duplicate delivery.
#[derive(Clone)]
pub struct OutboxRelay {
    db: DatabaseConnection,
    target: Arc<dyn EventTransport>,
    config: RelayConfig,
    metrics: Arc<RelayMetrics>,
    pass_lock: Arc<tokio::sync::Mutex<()>>,
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
            pass_lock: Arc::new(tokio::sync::Mutex::new(())),
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
        self.config.validate()?;
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
        Ok(self.process_pending_once_report(max_batch_hint).await?.claimed_count)
    }

    pub(crate) async fn process_pending_once_report(
        &self,
        max_batch_hint: Option<u64>,
    ) -> Result<OutboxRelayRunOnceProjection> {
        self.config.validate()?;
        if max_batch_hint == Some(0) {
            return Err(Error::Validation(
                "outbox max_batch_hint must be greater than zero".to_string(),
            ));
        }
        let batch_size = max_batch_hint
            .unwrap_or(self.config.batch_size)
            .min(self.config.batch_size) as usize;
        // Keep the pass guard alive inside each spawned task as well. Dropping
        // a timed-out pass aborts its JoinSet, but a later pass must wait until
        // those tasks have actually released their guard clones.
        let guard = Arc::new(self.pass_lock.clone().lock_owned().await);
        let eligible_at = rustok_outbox_storage::observe_database_clock(&self.db).await?;
        let mut cursor = None;
        let mut report = OutboxRelayRunOnceProjection {
            claimed_count: 0, success_total: 0, failure_total: 0,
            retry_total: 0, dlq_total: 0, processed_total: 0,
        };

        while report.claimed_count < batch_size {
            // Reserve no more rows than can begin dispatch in this wave.
            // A forward-only cursor prevents reprocessing a row in this pass,
            // even if an operator requeues it or its retry delay elapses.
            let limit = (batch_size - report.claimed_count).min(self.config.max_concurrency);
            let claimed = self.claim_batch(limit as u64, eligible_at, cursor).await?;
            if claimed.is_empty() {
                break;
            }
            cursor = claimed.last().map(|row| (row.created_at, row.id));
            report.claimed_count += claimed.len();

            let mut tasks = tokio::task::JoinSet::new();
            let mut first_error = None;
            for model in claimed {
                let relay = self.clone();
                let task_guard = Arc::clone(&guard);
                tasks.spawn(async move {
                    let _guard = task_guard;
                    relay.process_claimed_event(&model).await
                });
            }
            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok(Ok(outcome)) => {
                        report.processed_total += 1;
                        match outcome {
                            DispatchOutcome::Succeeded => report.success_total += 1,
                            DispatchOutcome::Retried => {
                                report.failure_total += 1;
                                report.retry_total += 1;
                            }
                            DispatchOutcome::DeadLettered => {
                                report.failure_total += 1;
                                report.dlq_total += 1;
                            }
                        }
                    }
                    Ok(Err(error)) => {
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                    Err(error) => {
                        tracing::error!(error = ?error, "Outbox relay dispatch task failed");
                        if first_error.is_none() {
                            first_error = Some(Error::External(format!(
                                "outbox relay dispatch task failed: {error}"
                            )));
                        }
                    }
                }
            }
            if let Some(error) = first_error {
                return Err(error);
            }
        }
        Ok(report)
    }

    #[cfg(test)]
    fn validate_config(&self) -> Result<()> {
        self.config.validate()
    }

    async fn claim_batch(
        &self,
        batch_size: u64,
        eligible_at: DateTime<Utc>,
        cursor: Option<(DateTime<Utc>, Uuid)>,
    ) -> Result<Vec<entity::Model>> {
        let txn = self.db.begin().await?;
        let backend = txn.get_database_backend();
        if backend == DatabaseBackend::Sqlite {
            // Avoid upgrading a stale read snapshot into a writer.
            txn.execute_unprepared("UPDATE sys_events SET claimed_at = claimed_at WHERE 1 = 0")
                .await?;
        }
        let claim_ttl = chrono::Duration::from_std(self.config.claim_ttl)
            .map_err(|_| Error::Validation("outbox relay claim_ttl is out of range".to_string()))?;
        let stale_before = eligible_at.checked_sub_signed(claim_ttl)
            .ok_or_else(|| Error::Validation("outbox relay lease cutoff is out of range".to_string()))?;
        // Fencing owner for this claim pass: unique per pass, so two relay
        // processes configured with the same worker_id (and two passes of
        // the same process) can never complete each other's claims. The
        // completion updates match on this exact value.
        let worker_id = format!("{}:{}", self.config.worker_id, Uuid::new_v4());

        let mut candidates_query = entity::Entity::find()
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(
                Condition::any()
                    .add(entity::Column::NextAttemptAt.is_null())
                    .add(entity::Column::NextAttemptAt.lte(eligible_at)),
            )
            .filter(
                Condition::any()
                    .add(entity::Column::ClaimedAt.is_null())
                    .add(entity::Column::ClaimedAt.lte(stale_before)),
            )
            .order_by_asc(entity::Column::CreatedAt)
            .order_by_asc(entity::Column::Id)
            .limit(batch_size);
        if let Some((created_at, id)) = cursor {
            candidates_query = candidates_query.filter(
                Condition::any()
                    .add(entity::Column::CreatedAt.gt(created_at))
                    .add(Condition::all()
                        .add(entity::Column::CreatedAt.eq(created_at))
                        .add(entity::Column::Id.gt(id))),
            );
        }
        let candidates = if backend == DatabaseBackend::Postgres {
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

        let now = rustok_outbox_storage::database_now(&txn).await?;
        if now < eligible_at {
            return Err(Error::External("outbox clock regressed within relay pass".to_string()));
        }
        entity::Entity::update_many()
            .filter(entity::Column::Id.is_in(candidate_ids.clone()))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(
                Condition::any()
                    .add(entity::Column::NextAttemptAt.is_null())
                    .add(entity::Column::NextAttemptAt.lte(eligible_at)),
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
            .order_by_asc(entity::Column::CreatedAt)
            .order_by_asc(entity::Column::Id)
            .all(&txn)
            .await?;

        txn.commit().await?;
        Ok(claimed)
    }

    // A claim may expire during scheduling or database contention.
    // Re-arm only the exact pending claim, before invoking the target.
    async fn refresh_claim(&self, model: &entity::Model) -> Result<entity::Model> {
        if model.claimed_by.is_none() || model.claimed_at.is_none() {
            return Err(Error::External(format!(
                "outbox event {} has no dispatch claim",
                model.id
            )));
        }
        let txn = self.db.begin().await?;
        // Acquire the exact pending row before observing its remaining time.
        let locked = entity::Entity::update_many()
            .filter(entity::Column::Id.eq(model.id))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.eq(model.claimed_by.clone()))
            .filter(entity::Column::ClaimedAt.eq(model.claimed_at))
            .set(entity::ActiveModel {
                claimed_by: Set(model.claimed_by.clone()),
                ..Default::default()
            })
            .exec(&txn)
            .await?;
        self.ensure_claim_owned(model.id, locked.rows_affected)?;
        let now = rustok_outbox_storage::database_now(&txn).await?;
        let claimed_at = model.claimed_at.expect("checked claim timestamp");
        let ttl = chrono::Duration::from_std(self.config.claim_ttl)
            .map_err(|_| Error::Validation("outbox relay claim_ttl is out of range".to_string()))?;
        if claimed_at > now || now.signed_duration_since(claimed_at) >= ttl {
            txn.rollback().await?;
            return Err(Error::External(format!("outbox event {} claim expired before dispatch", model.id)));
        }
        // Rotate the token even when renewal occurs in the same clock tick.
        let token = format!("{}:{}", self.config.worker_id, Uuid::new_v4());
        let result = entity::Entity::update_many()
            .filter(entity::Column::Id.eq(model.id))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.eq(model.claimed_by.clone()))
            .filter(entity::Column::ClaimedAt.eq(model.claimed_at))
            .set(entity::ActiveModel {
                claimed_by: Set(Some(token.clone())),
                claimed_at: Set(Some(now)),
                ..Default::default()
            })
            .exec(&txn)
            .await?;
        self.ensure_claim_owned(model.id, result.rows_affected)?;
        txn.commit().await?;
        let mut refreshed = model.clone();
        refreshed.claimed_by = Some(token);
        refreshed.claimed_at = Some(now);
        Ok(refreshed)
    }

    async fn process_claimed_event(&self, model: &entity::Model) -> Result<DispatchOutcome> {
        let started = Instant::now();
        let refreshed = self.refresh_claim(model).await?;
        let model = &refreshed;
        let event_id = model.id;
        let envelope = match Self::decode_envelope(model) {
            Ok(envelope) => envelope,
            Err(error) => {
                // A payload that cannot be decoded or no longer matches its
                // registered schema will never become dispatchable: send it
                // straight to the DLQ instead of burning the retry budget.
                tracing::warn!(
                    event_id = %event_id,
                    error = %error,
                    "Outbox event payload is invalid"
                );
                let outcome = self.mark_failed_attempt(model, error, true).await?;
                self.record_processed(
                    &model.event_type,
                    started.elapsed().as_millis() as u64,
                    false,
                );
                return Ok(outcome);
            }
        };

        let publish_timeout = self.config.publish_timeout;
        // Renewal and decoding consume the lease too. Do not start an
        // external effect unless the configured timeout fits the budget.
        if started.elapsed() >= self.config.claim_ttl.saturating_sub(publish_timeout).saturating_sub(Duration::from_millis(1)) {
            self.release_unpublished_claim(model).await?;
            return Err(Error::External(format!(
                "outbox event {event_id} dispatch lease budget exhausted before publication"
            )));
        }
        let publish_result = match envelope {
            RelayEnvelope::Root(envelope) => {
                tokio::time::timeout(publish_timeout, self.target.publish(envelope)).await
            }
            RelayEnvelope::Contract(envelope) => {
                tokio::time::timeout(publish_timeout, self.target.publish_contract(envelope)).await
            }
        };
        let publish_result = match publish_result {
            Ok(result) => result,
            Err(_elapsed) => Err(Error::External(format!(
                "outbox dispatch timed out after {} ms",
                publish_timeout.as_millis()
            ))),
        };
        let elapsed_ms = started.elapsed().as_millis() as u64;

        match publish_result {
            Ok(()) => {
                self.mark_dispatched(model).await?;
                tracing::info!(event_id = %event_id, latency_ms = elapsed_ms, "Outbox event dispatched");
                self.record_processed(&model.event_type, elapsed_ms, true);
                Ok(DispatchOutcome::Succeeded)
            }
            Err(err) => {
                tracing::warn!(event_id = %event_id, error = %err, "Outbox event dispatch failed");
                let terminal = Self::is_terminal_dispatch_error(&err);
                let outcome = self.mark_failed_attempt(model, err, terminal).await?;
                self.record_processed(&model.event_type, elapsed_ms, false);
                Ok(outcome)
            }
        }
    }

    /// Permanent dispatch failures (invalid envelope, unregistered schema,
    /// undecodable payload) cannot succeed on a later attempt and go to the
    /// DLQ immediately. Everything else (transport, database, timeouts) is
    /// treated as transient and follows the retry/backoff policy.
    fn is_terminal_dispatch_error(error: &Error) -> bool {
        matches!(error, Error::Validation(_) | Error::Serialization(_))
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
        let (id, tenant_id) = envelope.identity();
        if !model.matches_envelope_identity(id, tenant_id)
            || model.event_type != envelope.event_type()
            || model.schema_version != envelope_schema_version
        {
            return Err(Error::Validation(format!(
                "outbox row identity, tenant or schema metadata does not match envelope: row=`{}`/{} envelope=`{}`/{}",
                model.event_type,
                model.schema_version,
                envelope.event_type(),
                envelope.schema_version()
            )));
        }
        Ok(envelope)
    }

    fn record_processed(&self, event_type: &str, elapsed_ms: u64, succeeded: bool) {
        // Percentile-capable histogram in telemetry; the cumulative
        // `latency_ms_total` counter stays for cheap average derivation.
        rustok_telemetry::metrics::record_event_dispatch_latency_ms(
            "outbox_relay",
            event_type,
            elapsed_ms as f64,
        );
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

    // No broker effect has started, so this exact claim can be released
    // without incrementing the delivery retry budget.
    async fn release_unpublished_claim(&self, model: &entity::Model) -> Result<()> {
        let result = entity::Entity::update_many()
            .filter(entity::Column::Id.eq(model.id))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.eq(model.claimed_by.clone()))
            .filter(entity::Column::ClaimedAt.eq(model.claimed_at))
            .set(entity::ActiveModel {
                claimed_by: Set(None),
                claimed_at: Set(None),
                ..Default::default()
            })
            .exec(&self.db)
            .await?;
        self.ensure_claim_owned(model.id, result.rows_affected)
    }

    async fn mark_dispatched(&self, model: &entity::Model) -> Result<()> {
        let result = entity::Entity::update_many()
            .filter(entity::Column::Id.eq(model.id))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.eq(model.claimed_by.clone()))
            .filter(entity::Column::ClaimedAt.eq(model.claimed_at))
            .set(entity::ActiveModel {
                status: Set(SysEventStatus::Dispatched),
                dispatched_at: Set(Some(rustok_outbox_storage::database_now(&self.db).await?)),
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

    async fn mark_failed_attempt(
        &self,
        model: &entity::Model,
        error: Error,
        terminal: bool,
    ) -> Result<DispatchOutcome> {
        let retry_count = model.retry_count.saturating_add(1);
        let (status, next_attempt_at, moved_to_dlq) =
            if terminal || retry_count >= self.config.max_attempts {
                (SysEventStatus::Failed, None, true)
            } else {
                (
                    SysEventStatus::Pending,
                    Some(rustok_outbox_storage::database_now(&self.db).await?
                        .checked_add_signed(self.jittered_backoff(retry_count, Uuid::new_v4().as_u128())?)
                        .ok_or_else(|| Error::Validation("outbox retry deadline is out of range".to_string()))?),
                    false,
                )
            };

        let error_str = error.to_string();
        let last_error = if error_str.chars().count() > 2048 {
            error_str.chars().take(2048).collect::<String>()
        } else {
            error_str
        };

        let result = entity::Entity::update_many()
            .filter(entity::Column::Id.eq(model.id))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.eq(model.claimed_by.clone()))
            .filter(entity::Column::ClaimedAt.eq(model.claimed_at))
            .set(entity::ActiveModel {
                retry_count: Set(retry_count),
                last_error: Set(Some(last_error)),
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
            Ok(DispatchOutcome::DeadLettered)
        } else {
            tracing::info!(
                event_id = %model.id,
                retry_count,
                next_attempt_at = ?next_attempt_at,
                "Outbox event scheduled for retry"
            );
            self.metrics.retry_total.fetch_add(1, Ordering::Relaxed);
            Ok(DispatchOutcome::Retried)
        }
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

    // Equal jitter spreads a shared outage's retry traffic without exceeding
    // the configured exponential ceiling. SQL clock precision is one ms.
    fn jittered_backoff(&self, retry_count: i32, sample: u128) -> Result<chrono::Duration> {
        let ceiling = self.backoff_duration(retry_count)?.num_milliseconds().max(1);
        let floor = (ceiling / 2).max(1);
        let span = (ceiling - floor + 1) as u128;
        Ok(chrono::Duration::milliseconds(floor + (sample % span) as i64))
    }

    fn backoff_duration(&self, retry_count: i32) -> Result<chrono::Duration> {
        let mut delay = self.config.backoff_base.min(self.config.backoff_max);
        for _ in 1..retry_count {
            if delay.is_zero() || delay >= self.config.backoff_max {
                break;
            }
            delay = delay.saturating_mul(2).min(self.config.backoff_max);
        }
        chrono::Duration::from_std(delay)
            .map_err(|_| Error::Validation("outbox retry backoff is out of range".to_string()))
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
    async fn config_rejects_a_publish_timeout_that_outlives_the_claim_lease() {
        let relay = OutboxRelay::new(database().await, Arc::new(RejectUnexpectedPublish))
            .with_config(RelayConfig {
                claim_ttl: Duration::from_secs(10),
                publish_timeout: Duration::from_secs(10),
                ..RelayConfig::default()
            });
        assert!(relay.validate_config().is_err());

        let relay = relay.with_config(RelayConfig {
            claim_ttl: Duration::from_secs(10),
            publish_timeout: Duration::from_secs(5),
            ..RelayConfig::default()
        });
        assert!(relay.validate_config().is_ok());
    }

    #[tokio::test]
    async fn backoff_reaches_cap_without_overflow_or_fixed_exponent_ceiling() {
        let relay = OutboxRelay::new(database().await, Arc::new(RejectUnexpectedPublish))
            .with_config(RelayConfig {
                backoff_base: Duration::from_millis(1),
                backoff_max: Duration::from_secs(1000),
                ..RelayConfig::default()
            });
        assert_eq!(relay.backoff_duration(1).unwrap(), chrono::Duration::milliseconds(1));
        assert_eq!(relay.backoff_duration(18).unwrap(), chrono::Duration::milliseconds(131_072));
        assert_eq!(relay.jittered_backoff(18, 0).unwrap(), chrono::Duration::milliseconds(65_536));
        assert_eq!(relay.jittered_backoff(18, 65_536).unwrap(), chrono::Duration::milliseconds(131_072));
        assert_eq!(relay.jittered_backoff(1, u128::MAX).unwrap(), chrono::Duration::milliseconds(1));
        assert_eq!(relay.backoff_duration(i32::MAX).unwrap(), chrono::Duration::seconds(1000));
        let invalid = relay.with_config(RelayConfig {
            backoff_max: Duration::MAX,
            ..RelayConfig::default()
        });
        assert!(invalid.validate_config().is_err());
    }

    #[test]
    fn permanent_dispatch_errors_are_terminal_transient_ones_are_not() {
        assert!(OutboxRelay::is_terminal_dispatch_error(&Error::Validation(
            "bad envelope".to_string()
        )));
        assert!(OutboxRelay::is_terminal_dispatch_error(
            &Error::Serialization(serde_json::from_str::<serde_json::Value>("x").unwrap_err())
        ));
        assert!(!OutboxRelay::is_terminal_dispatch_error(&Error::External(
            "broker unavailable".to_string()
        )));
    }

    #[tokio::test]
    async fn invalid_payload_reaches_dlq_instead_of_remaining_claimed() {
        let db = database().await;
        let event_id = Uuid::new_v4();
        entity::Entity::insert(entity::ActiveModel {
            id: Set(event_id),
            tenant_id: Set(None),
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
        })
        .exec_without_returning(&db)
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
        entity::Entity::insert(entity::ActiveModel {
            id: Set(event_id),
            tenant_id: Set(Some(envelope.tenant_id())),
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
        })
        .exec_without_returning(&db)
        .await
        .expect("insert contract event");

        let stored = entity::Entity::find_by_id(event_id)
            .one(&db)
            .await
            .expect("query")
            .expect("event");
        assert!(OutboxRelay::decode_envelope(&stored).is_ok());
        let mut wrong_id = stored.clone();
        wrong_id.id = Uuid::new_v4();
        assert!(OutboxRelay::decode_envelope(&wrong_id).is_err());
        let mut wrong_tenant = stored.clone();
        wrong_tenant.tenant_id = Some(Uuid::new_v4());
        assert!(OutboxRelay::decode_envelope(&wrong_tenant).is_err());

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
    async fn queued_claim_is_refreshed_and_old_snapshot_is_fenced() {
        let db = database().await;
        let envelope = EventEnvelope::new(
            Uuid::new_v4(),
            None,
            rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
        );
        let event_id = envelope.id;
        crate::OutboxTransport::new(db.clone())
            .publish(envelope)
            .await
            .expect("persist event");
        entity::Entity::update_many()
            .filter(entity::Column::Id.eq(event_id))
            .set(entity::ActiveModel {
                claimed_by: Set(Some("queued-worker".to_string())),
                claimed_at: Set(Some(Utc::now() - chrono::Duration::seconds(1))),
                ..Default::default()
            })
            .exec(&db)
            .await
            .expect("queue claim");
        let queued = entity::Entity::find_by_id(event_id)
            .one(&db)
            .await
            .expect("query")
            .expect("event");
        let relay = OutboxRelay::new(db, Arc::new(RejectUnexpectedPublish));
        assert!(OutboxRelay::decode_envelope(&queued).is_ok());
        let mut wrong_id = queued.clone();
        wrong_id.id = Uuid::new_v4();
        assert!(OutboxRelay::decode_envelope(&wrong_id).is_err());
        let mut wrong_tenant = queued.clone();
        wrong_tenant.tenant_id = Some(Uuid::new_v4());
        assert!(OutboxRelay::decode_envelope(&wrong_tenant).is_err());

        let renewed = relay.refresh_claim(&queued).await.expect("renew claim");
        assert_ne!(renewed.claimed_by, queued.claimed_by);
        assert!(renewed.claimed_at > queued.claimed_at);
        assert!(relay.mark_dispatched(&queued).await.is_err());
        relay.mark_dispatched(&renewed).await.expect("current claim");
    }

    #[tokio::test]
    async fn expired_claim_cannot_be_refreshed_without_reclaim() {
        let db = database().await;
        let envelope = EventEnvelope::new(
            Uuid::new_v4(), None,
            rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
        );
        let id = envelope.id;
        crate::OutboxTransport::new(db.clone()).publish(envelope).await.unwrap();
        entity::Entity::update_many()
            .filter(entity::Column::Id.eq(id))
            .set(entity::ActiveModel {
                claimed_by: Set(Some("expired-worker".into())),
                claimed_at: Set(Some(Utc::now() - chrono::Duration::minutes(2))),
                ..Default::default()
            })
            .exec(&db).await.unwrap();
        let row = entity::Entity::find_by_id(id).one(&db).await.unwrap().unwrap();
        let relay = OutboxRelay::new(db, Arc::new(RejectUnexpectedPublish));
        assert!(relay.refresh_claim(&row).await.is_err());
    }

    #[tokio::test]
    async fn stale_worker_cannot_complete_a_reclaimed_event() {
        let db = database().await;
        let event_id = Uuid::new_v4();
        entity::Entity::insert(entity::ActiveModel {
            id: Set(event_id),
            tenant_id: Set(None),
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
        })
        .exec_without_returning(&db)
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
        assert!(relay.refresh_claim(&old_model).await.is_err());
        assert!(relay.process_claimed_event(&old_model).await.is_err());
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
