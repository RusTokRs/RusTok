use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, QueryFilter,
    DatabaseTransaction, PaginatorTrait, QueryOrder, QuerySelect, Set, Statement, TransactionTrait,
    sea_query::{LockBehavior, LockType},
};
use serde_json::from_value;
use uuid::Uuid;

use rustok_core::events::EventTransport;
use rustok_core::{Error, Result};
use rustok_events::{ContractEventEnvelope, EventEnvelope};

use crate::{attempts, checkpoints, entity};
use crate::entity::SysEventStatus;
use crate::ports::OutboxRelayRunOnceProjection;

/// Upper bound on rows claimed in one relay pass. Matches the server
/// runtime cap so a library caller cannot materialise an unbounded batch.
pub const MAX_RELAY_BATCH_SIZE: u64 = 10_000;
/// Upper bound on local and cluster-policy simultaneous dispatches.
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
        if self.backoff_base.subsec_nanos() % 1_000_000 != 0
            || self.backoff_max.subsec_nanos() % 1_000_000 != 0
        {
            return Err(Error::Validation("outbox retry backoff must use whole milliseconds".into()));
        }
        if self.claim_ttl.subsec_nanos() % 1_000_000 != 0 {
            return Err(Error::Validation("outbox claim_ttl must use whole milliseconds".to_string()));
        }
        if self.claim_ttl > MAX_RELAY_INTERVAL || self.backoff_max > MAX_RELAY_INTERVAL {
            return Err(Error::Validation("outbox relay intervals must not exceed 3650 days".to_string()));
        }
        Ok(())
    }

    /// Verify the durable shared policy before starting broker resources.
    pub async fn verify_execution_admission(&self, db: &DatabaseConnection) -> Result<()> {
        self.validate()?;
        let txn = db.begin().await?;
        self.lock_execution_policy(&txn).await?;
        rustok_outbox_storage::database_now(&txn).await?;
        txn.commit().await?;
        Ok(())
    }

    // Lock order: execution policy -> sys_events. No broker I/O under
    // this lock. READ COMMITTED sees all preceding committed admissions.
    async fn lock_execution_policy(&self, txn: &DatabaseTransaction) -> Result<u64> {
        let backend = txn.get_database_backend();
        if backend == DatabaseBackend::Postgres {
            txn.execute_unprepared("SET TRANSACTION ISOLATION LEVEL READ COMMITTED").await?;
        }
        let locked = txn.execute_unprepared(
            "UPDATE outbox_execution_policy SET max_in_flight = max_in_flight WHERE id = 1",
        ).await?;
        if locked.rows_affected() != 1 {
            return Err(Error::External("outbox execution policy is missing".to_string()));
        }
        let read = || async {
            let row = txn.query_one_raw(Statement::from_string(backend,
                "SELECT max_in_flight, claim_ttl_ms, retry_max_attempts, retry_backoff_base_ms, retry_backoff_max_ms FROM outbox_execution_policy WHERE id = 1",
            )).await?.ok_or_else(|| Error::External("outbox execution policy is missing".into()))?;
            Ok::<_, Error>((row.try_get::<i64>("", "max_in_flight")?,
                row.try_get::<Option<i64>>("", "claim_ttl_ms")?,
                row.try_get::<Option<i64>>("", "retry_max_attempts")?,
                row.try_get::<Option<i64>>("", "retry_backoff_base_ms")?,
                row.try_get::<Option<i64>>("", "retry_backoff_max_ms")?))
        };
        let (cap, persisted_ttl, attempts, base, max) = read().await?;
        let ttl = i64::try_from(self.claim_ttl.as_millis())
            .map_err(|_| Error::Validation("outbox claim TTL is out of range".into()))?;
        let expected_attempts = i64::from(self.max_attempts);
        let expected_base = i64::try_from(self.backoff_base.as_millis())
            .map_err(|_| Error::Validation("outbox retry base is out of range".into()))?;
        let expected_max = i64::try_from(self.backoff_max.as_millis())
            .map_err(|_| Error::Validation("outbox retry maximum is out of range".into()))?;
        if !(1..=MAX_RELAY_CONCURRENCY as i64).contains(&cap) || ttl <= 1 {
            return Err(Error::Validation("invalid durable outbox execution policy".into()));
        }
        if persisted_ttl.is_some_and(|stored| stored != ttl) {
            return Err(Error::Validation("outbox relay claim_ttl differs from durable execution policy; coordinated reconfiguration required".into()));
        }
        let expected_retry = (Some(expected_attempts), Some(expected_base), Some(expected_max));
        let persisted_retry = (attempts, base, max);
        if persisted_retry != (None, None, None) && persisted_retry != expected_retry {
            return Err(Error::Validation("outbox retry settings differ from durable execution policy; coordinated reconfiguration required".into()));
        }
        if persisted_ttl.is_none() || persisted_retry == (None, None, None) {
            let sql = match backend {
                DatabaseBackend::Postgres => "UPDATE outbox_execution_policy SET claim_ttl_ms = $1, retry_max_attempts = $2, retry_backoff_base_ms = $3, retry_backoff_max_ms = $4 WHERE id = 1",
                DatabaseBackend::Sqlite => "UPDATE outbox_execution_policy SET claim_ttl_ms = ?, retry_max_attempts = ?, retry_backoff_base_ms = ?, retry_backoff_max_ms = ? WHERE id = 1",
                _ => return Err(Error::Validation("unsupported outbox execution backend".into())),
            };
            let changed = txn.execute_raw(Statement::from_sql_and_values(backend, sql,
                [ttl.into(), expected_attempts.into(), expected_base.into(), expected_max.into()])).await?;
            if changed.rows_affected() != 1
                || read().await? != (cap, Some(ttl), expected_retry.0, expected_retry.1, expected_retry.2)
            {
                return Err(Error::External("outbox execution policy binding readback mismatch".into()));
            }
        }
        Ok(cap as u64)
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
    delivery_route: String,
}

impl std::fmt::Debug for OutboxRelay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutboxRelay")
            .field("config", &self.config)
            .field("metrics", &self.metrics.snapshot())
            .finish_non_exhaustive()
    }
}


#[derive(Debug)]
enum AttemptAdmission {
    Ready(Uuid),
    BudgetExhausted,
    RouteConflict,
}

#[derive(Clone)]
struct DeliveryScope {
    relay: OutboxRelay,
    model: entity::Model,
    attempt_id: Uuid,
}

tokio::task_local! {
    static DELIVERY_SCOPE: DeliveryScope;
}

/// Execute the primary leg of acknowledged fanout. Only the current relay
/// task can record/skip durable acceptance. Checkpoint-enabled callers
/// without that scope fail closed. Do not spawn this call into another task.
pub async fn publish_primary_with_checkpoint<F>(
    event_id: Uuid,
    local_required: bool,
    checkpoint_enabled: bool,
    publish: F,
) -> Result<()>
where
    F: std::future::Future<Output = Result<()>> + Send,
{
    let Ok(scope) = DELIVERY_SCOPE.try_with(Clone::clone) else {
        if checkpoint_enabled {
            return Err(Error::Validation("checkpoint-enabled fanout requires an outbox dispatch scope".into()));
        }
        return publish.await;
    };
    if scope.model.id != event_id {
        return Err(Error::Validation("outbox fanout event does not match dispatch scope".into()));
    }
    if scope.relay.prepare_primary(&scope, local_required, checkpoint_enabled).await? {
        return Ok(());
    }
    publish.await?;
    if checkpoint_enabled {
        scope.relay.confirm_primary(&scope).await?;
    }
    Ok(())
}

impl OutboxRelay {
    pub fn new(db: DatabaseConnection, target: Arc<dyn EventTransport>) -> Self {
        Self {
            db,
            target,
            config: RelayConfig::default(),
            metrics: Arc::new(RelayMetrics::default()),
            pass_lock: Arc::new(tokio::sync::Mutex::new(())),
            delivery_route: "unclassified-v1".into(),
        }
    }

    pub fn with_config(mut self, config: RelayConfig) -> Self {
        self.config = config;
        self
    }

    /// Stable host-owned route identity, including the delivery profile.
    /// Existing event routes are never silently rebound.
    pub fn with_delivery_route(mut self, route: impl Into<String>) -> Self {
        self.delivery_route = route.into();
        self
    }

    pub fn metrics(&self) -> RelayMetricsSnapshot {
        self.metrics.snapshot()
    }

    pub async fn run(&self) -> Result<()> {
        self.validate_config()?;
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
        self.validate_config()?;
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

        let mut tasks = tokio::task::JoinSet::new();
        let mut first_error = None;
        loop {
            if first_error.is_none() && report.claimed_count < batch_size
                && tasks.len() < self.config.max_concurrency
            {
                let limit = (batch_size - report.claimed_count)
                    .min(self.config.max_concurrency - tasks.len());
                // A claim error must not abort already running publications.
                match self.claim_batch(limit as u64, eligible_at, cursor).await {
                    Ok(claimed) => {
                        if let Some(last) = claimed.last() {
                            cursor = Some((last.created_at, last.id));
                        }
                        report.claimed_count += claimed.len();
                        for model in claimed {
                            let relay = self.clone();
                            let task_guard = Arc::clone(&guard);
                            tasks.spawn(async move {
                                let _guard = task_guard;
                                relay.process_claimed_event(&model).await
                            });
                        }
                    }
                    Err(error) => first_error = Some(error),
                }
            }
            // If other processes own the capacity, return rather than spin.
            let Some(result) = tasks.join_next().await else { break };
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
        Ok(report)
    }

    fn validate_config(&self) -> Result<()> {
        self.config.validate()?;
        if self.delivery_route.is_empty()
            || self.delivery_route.len() > 128
            || self.delivery_route.trim() != self.delivery_route
        {
            return Err(Error::Validation("invalid outbox delivery route key".into()));
        }
        Ok(())
    }

    async fn claim_batch(
        &self,
        batch_size: u64,
        eligible_at: DateTime<Utc>,
        cursor: Option<(DateTime<Utc>, Uuid)>,
    ) -> Result<Vec<entity::Model>> {
        let txn = self.db.begin().await?;
        let backend = txn.get_database_backend();
        let cap = self.config.lock_execution_policy(&txn).await?;
        let claim_ttl = chrono::Duration::from_std(self.config.claim_ttl)
            .map_err(|_| Error::Validation("outbox relay claim_ttl is out of range".to_string()))?;
        let stale_before = eligible_at.checked_sub_signed(claim_ttl)
            .ok_or_else(|| Error::Validation("outbox relay lease cutoff is out of range".to_string()))?;
        // Count live reservations using the same conservative horizon
        // as the reclaim query, under the shared policy writer lock.
        let active = entity::Entity::find()
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedAt.gt(stale_before))
            .count(&txn).await?;
        let batch_size = batch_size.min(cap.saturating_sub(active));
        if batch_size == 0 {
            txn.commit().await?;
            return Ok(Vec::new());
        }
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
        let claimed_count = entity::Entity::update_many()
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
            .await?
            .rows_affected;
        // Both backends hold the admission writer fence; PostgreSQL also
        // locks every selected candidate. A short write is an invariant
        // failure, not a successful empty batch.
        if claimed_count != candidate_ids.len() as u64 {
            return Err(Error::External("outbox claim batch write was incomplete".into()));
        }

        let claimed = entity::Entity::find()
            .filter(entity::Column::Id.is_in(candidate_ids))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.eq(worker_id.clone()))
            .filter(entity::Column::ClaimedAt.eq(now))
            .order_by_asc(entity::Column::CreatedAt)
            .order_by_asc(entity::Column::Id)
            .all(&txn)
            .await?;

        if claimed.len() as u64 != claimed_count {
            return Err(Error::External("outbox claim batch readback did not match the write".into()));
        }
        for (mut expected, saved) in candidates.into_iter().zip(&claimed) {
            expected.claimed_by = Some(worker_id.clone());
            expected.claimed_at = Some(now);
            if &expected != saved {
                return Err(Error::External(format!(
                    "outbox event {} claim batch transition readback mismatch", expected.id
                )));
            }
        }
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
        self.config.lock_execution_policy(&txn).await?;
        // PostgreSQL takes the source row NOWAIT; SQLite already owns the
        // database writer through execution-policy admission. Neither needs
        // a synthetic event UPDATE before sampling the lease.
        let query = entity::Entity::find_by_id(model.id)
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.eq(model.claimed_by.clone()))
            .filter(entity::Column::ClaimedAt.eq(model.claimed_at));
        let current = if txn.get_database_backend() == DatabaseBackend::Postgres {
            query.lock_with_behavior(LockType::Update, LockBehavior::Nowait).one(&txn).await?
        } else {
            query.one(&txn).await?
        };
        if current.as_ref() != Some(model) {
            return Err(Error::External(format!(
                "outbox event {} claim snapshot changed before refresh", model.id
            )));
        }
        let now = rustok_outbox_storage::database_now(&txn).await?;
        let claimed_at = model.claimed_at.ok_or_else(|| Error::External("missing outbox claim timestamp".into()))?;
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
        let mut expected = model.clone();
        expected.claimed_by = Some(token);
        expected.claimed_at = Some(now);
        Self::verify_event_state(&txn, &expected).await?;
        txn.commit().await?;
        Ok(expected)
    }

    async fn process_claimed_event(&self, model: &entity::Model) -> Result<DispatchOutcome> {
        let started = Instant::now();
        let refreshed = self.refresh_claim(model).await?;
        let model = &refreshed;
        let event_id = model.id;
        if let Some(outcome) = self.recover_interrupted_attempt(model).await? {
            self.record_processed(&model.event_type, started.elapsed().as_millis() as u64, false);
            return Ok(outcome);
        }
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
                let outcome = self.mark_failed_attempt(model, error, true, None).await?;
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
            self.release_unpublished_claim(model, None).await?;
            return Err(Error::External(format!(
                "outbox event {event_id} dispatch lease budget exhausted before publication"
            )));
        }
        let attempt_id = match self.begin_dispatch_attempt(model).await? {
            AttemptAdmission::Ready(id) => id,
            AttemptAdmission::BudgetExhausted => {
                self.release_unpublished_claim(model, None).await?;
                return Err(Error::External(format!("outbox event {event_id} dispatch lease budget exhausted before intent")));
            }
            AttemptAdmission::RouteConflict => {
                // The persisted route cannot be rebound by retrying. No
                // dispatch intent exists, so a charged pending retry would
                // also violate the database event-state guard. Quarantine
                // immediately and preserve the route/checkpoint for recovery.
                let outcome = self.mark_failed_attempt(
                    model,
                    Error::Validation("outbox delivery route changed; reconcile event before replay".into()),
                    true,
                    None,
                ).await?;
                self.record_processed(&model.event_type, started.elapsed().as_millis() as u64, false);
                return Ok(outcome);
            }
        };
        // Journal admission itself consumes time. Do not start the target if
        // the durable intent commit used the remaining monotonic lease budget.
        if started.elapsed() >= self.config.claim_ttl.saturating_sub(publish_timeout).saturating_sub(Duration::from_millis(1)) {
            self.release_unpublished_claim(model, Some(attempt_id)).await?;
            return Err(Error::External(format!("outbox event {event_id} dispatch lease budget exhausted after intent")));
        }
        let scope = DeliveryScope { relay: self.clone(), model: model.clone(), attempt_id };
        let publish_result = DELIVERY_SCOPE.scope(scope, async {
            tokio::time::timeout(publish_timeout, async {
                match envelope {
                    RelayEnvelope::Root(envelope) => self.target.publish(envelope).await,
                    RelayEnvelope::Contract(envelope) => self.target.publish_contract(envelope).await,
                }
            }).await
        }).await;
        let publish_result = match publish_result {
            Ok(result) => result,
            Err(_elapsed) => Err(Error::External(format!(
                "outbox dispatch outcome unconfirmed after timeout ({} ms); retry may redeliver",
                publish_timeout.as_millis()
            ))),
        };
        let elapsed_ms = started.elapsed().as_millis() as u64;

        match publish_result {
            Ok(()) => {
                self.mark_dispatched(model, attempt_id).await?;
                tracing::info!(event_id = %event_id, latency_ms = elapsed_ms, "Outbox event dispatched");
                self.record_processed(&model.event_type, elapsed_ms, true);
                Ok(DispatchOutcome::Succeeded)
            }
            Err(err) => {
                tracing::warn!(event_id = %event_id, error = %err, "Outbox event dispatch failed");
                // The transport can have performed a remote effect before
                // reporting a later local validation/serialization failure.
                // Only relay-side envelope rejection is proven pre-publication.
                let outcome = self.mark_failed_attempt(model, err, false, Some((attempt_id, Self::claim_token(model)?))).await?;
                self.record_processed(&model.event_type, elapsed_ms, false);
                Ok(outcome)
            }
        }
    }

    /// Permanent dispatch failures (invalid envelope, unregistered schema,
    /// undecodable payload) cannot succeed on a later attempt and go to the
    /// DLQ immediately. Everything else (transport, database, timeouts) is
    /// treated as transient and follows the retry/backoff policy.
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


    // Event row -> attempt row. Never hold either lock across transport I/O.
    async fn lock_current_claim(&self, txn: &DatabaseTransaction, model: &entity::Model) -> Result<()> {
        if model.claimed_by.is_none() || model.claimed_at.is_none() {
            return Err(Error::External("outbox dispatch has no claim".into()));
        }
        let result = entity::Entity::update_many()
            .filter(entity::Column::Id.eq(model.id))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.eq(model.claimed_by.clone()))
            .filter(entity::Column::ClaimedAt.eq(model.claimed_at))
            // No mutation of the claim itself; acquire the event writer lock.
            .set(entity::ActiveModel { retry_count: Set(model.retry_count), ..Default::default() })
            .filter(entity::Column::RetryCount.eq(model.retry_count))
            .exec(txn).await?;
        self.ensure_claim_owned(model.id, result.rows_affected)?;
        Self::verify_event_state(txn, model).await
    }

    fn claim_token(model: &entity::Model) -> Result<&str> {
        model.claimed_by.as_deref()
            .ok_or_else(|| Error::External("missing outbox claim token".into()))
    }

    // Affected-row counts do not detect an UPDATE rewritten by a trigger.
    async fn verify_event_state(txn: &DatabaseTransaction, expected: &entity::Model) -> Result<()> {
        let saved = entity::Entity::find_by_id(expected.id).one(txn).await?;
        if saved.as_ref() != Some(expected) {
            return Err(Error::External(format!("outbox event {} transition readback mismatch", expected.id)));
        }
        Ok(())
    }

    async fn bind_delivery_route(&self, txn: &DatabaseTransaction, model: &entity::Model) -> Result<bool> {
        if let Some(row) = checkpoints::Entity::find_by_id(model.id).one(txn).await? {
            if row.route_key != self.delivery_route {
                return Ok(false);
            }
        } else {
            let affected = checkpoints::Entity::insert(checkpoints::ActiveModel {
                event_id: Set(model.id), route_key: Set(self.delivery_route.clone()),
                local_required: Set(None), primary_confirmed_at: Set(None),
            }).exec_without_returning(txn).await?;
            if affected != 1 {
                return Err(Error::External("outbox delivery route checkpoint was not inserted".into()));
            }
            let saved = checkpoints::Entity::find_by_id(model.id).one(txn).await?
                .ok_or_else(|| Error::External("outbox delivery route checkpoint readback failed".into()))?;
            if saved.route_key != self.delivery_route || saved.local_required.is_some()
                || saved.primary_confirmed_at.is_some()
            {
                return Err(Error::External("outbox delivery route checkpoint was altered during admission".into()));
            }
        }
        Ok(true)
    }

    async fn lock_delivery_scope(&self, txn: &DatabaseTransaction, scope: &DeliveryScope) -> Result<DateTime<Utc>> {
        self.lock_current_claim(txn, &scope.model).await?;
        let now = rustok_outbox_storage::database_now(txn).await?;
        let claimed_at = scope.model.claimed_at.ok_or_else(|| Error::External("missing outbox claim timestamp".into()))?;
        let ttl = chrono::Duration::from_std(self.config.claim_ttl)
            .map_err(|_| Error::Validation("outbox lease duration out of range".into()))?;
        if now < claimed_at || now.signed_duration_since(claimed_at) >= ttl {
            return Err(Error::External("outbox fanout lease expired".into()));
        }
        let open = attempts::Entity::find_by_id(scope.attempt_id)
            .filter(attempts::Column::EventId.eq(scope.model.id))
            .filter(attempts::Column::ClaimToken.eq(scope.model.claimed_by.clone().ok_or_else(|| Error::External("missing claim token".into()))?))
            .filter(attempts::Column::State.eq(attempts::OPEN))
            .one(txn).await?;
        if open.is_none() {
            return Err(Error::External("outbox fanout dispatch intent is no longer open".into()));
        }
        Ok(now)
    }

    async fn prepare_primary(&self, scope: &DeliveryScope, local_required: bool, checkpoint_enabled: bool) -> Result<bool> {
        let txn = self.db.begin().await?;
        self.lock_delivery_scope(&txn, scope).await?;
        let row = checkpoints::Entity::find_by_id(scope.model.id).one(&txn).await?
            .ok_or_else(|| Error::External("missing outbox delivery checkpoint".into()))?;
        if row.route_key != self.delivery_route || row.local_required.is_some_and(|old| old != local_required) {
            return Err(Error::Validation("outbox fanout topology changed; reconcile event before retry".into()));
        }
        if row.local_required.is_none() {
            let result = checkpoints::Entity::update_many()
                .filter(checkpoints::Column::EventId.eq(scope.model.id))
                .filter(checkpoints::Column::LocalRequired.is_null())
                .set(checkpoints::ActiveModel { local_required: Set(Some(local_required)), ..Default::default() })
                .exec(&txn).await?;
            self.ensure_claim_owned(scope.model.id, result.rows_affected)?;
        }
        // A confirmed primary is durable evidence. A misconfigured wrapper
        // must not bypass that evidence by disabling checkpointing on retry.
        if !checkpoint_enabled && row.primary_confirmed_at.is_some() {
            return Err(Error::Validation("outbox primary checkpoint cannot be disabled after confirmation".into()));
        }
        let skip = row.primary_confirmed_at.is_some();
        let mut expected = row;
        expected.local_required = Some(local_required);
        let saved = checkpoints::Entity::find_by_id(scope.model.id).one(&txn).await?;
        if saved.as_ref() != Some(&expected) {
            return Err(Error::External("outbox fanout topology readback mismatch".into()));
        }
        Self::verify_event_state(&txn, &scope.model).await?;
        txn.commit().await?;
        Ok(skip)
    }

    async fn confirm_primary(&self, scope: &DeliveryScope) -> Result<()> {
        let txn = self.db.begin().await?;
        let now = self.lock_delivery_scope(&txn, scope).await?;
        let mut expected = checkpoints::Entity::find_by_id(scope.model.id).one(&txn).await?
            .ok_or_else(|| Error::External("missing outbox delivery checkpoint".into()))?;
        let result = checkpoints::Entity::update_many()
            .filter(checkpoints::Column::EventId.eq(scope.model.id))
            .filter(checkpoints::Column::RouteKey.eq(self.delivery_route.clone()))
            .filter(checkpoints::Column::LocalRequired.is_not_null())
            .filter(checkpoints::Column::PrimaryConfirmedAt.is_null())
            .set(checkpoints::ActiveModel { primary_confirmed_at: Set(Some(now)), ..Default::default() })
            .exec(&txn).await?;
        self.ensure_claim_owned(scope.model.id, result.rows_affected)?;
        expected.primary_confirmed_at = Some(now);
        let saved = checkpoints::Entity::find_by_id(scope.model.id).one(&txn).await?;
        if saved.as_ref() != Some(&expected) {
            return Err(Error::External("outbox primary confirmation readback mismatch".into()));
        }
        Self::verify_event_state(&txn, &scope.model).await?;
        txn.commit().await?;
        Ok(())
    }

    async fn begin_dispatch_attempt(&self, model: &entity::Model) -> Result<AttemptAdmission> {
        let txn = self.db.begin().await?;
        self.lock_current_claim(&txn, model).await?;
        let now = rustok_outbox_storage::database_now(&txn).await?;
        let claimed_at = model.claimed_at.ok_or_else(|| Error::External("missing outbox claim timestamp".into()))?;
        let budget = chrono::Duration::from_std(
            self.config.claim_ttl.saturating_sub(self.config.publish_timeout).saturating_sub(Duration::from_millis(1))
        ).map_err(|_| Error::Validation("outbox dispatch budget out of range".into()))?;
        if now < claimed_at || now.signed_duration_since(claimed_at) >= budget {
            txn.rollback().await?;
            return Ok(AttemptAdmission::BudgetExhausted);
        }
        if !self.bind_delivery_route(&txn, model).await? {
            txn.rollback().await?;
            return Ok(AttemptAdmission::RouteConflict);
        }
        let id = Uuid::new_v4();
        let token = model.claimed_by.clone().ok_or_else(|| Error::External("missing outbox claim token".into()))?;
        let affected = attempts::Entity::insert(attempts::ActiveModel {
            id: Set(id),
            event_id: Set(model.id),
            claim_token: Set(token.clone()),
            started_at: Set(now),
            finished_at: Set(None),
            state: Set(attempts::OPEN.into()),
        }).exec_without_returning(&txn).await?;
        if affected != 1 {
            return Err(Error::External("outbox dispatch intent was not inserted".into()));
        }
        let saved = attempts::Entity::find_by_id(id).one(&txn).await?
            .ok_or_else(|| Error::External("outbox dispatch intent readback failed".into()))?;
        if saved.event_id != model.id || saved.claim_token != token || saved.started_at != now
            || saved.finished_at.is_some() || saved.state != attempts::OPEN
        {
            return Err(Error::External("outbox dispatch intent was altered during admission".into()));
        }
        txn.commit().await?;
        Ok(AttemptAdmission::Ready(id))
    }

    async fn finish_dispatch_attempt(
        &self, txn: &DatabaseTransaction, event_id: Uuid, attempt_id: Uuid,
        claim_token: &str, state: &str, now: DateTime<Utc>,
    ) -> Result<()> {
        let mut expected = attempts::Entity::find_by_id(attempt_id).one(txn).await?
            .ok_or_else(|| Error::External(format!("outbox event {event_id} dispatch attempt is missing")))?;
        if expected.event_id != event_id || expected.claim_token != claim_token
            || expected.state != attempts::OPEN || expected.finished_at.is_some()
        {
            return Err(Error::External(format!("outbox event {event_id} dispatch attempt authority mismatch")));
        }
        let result = attempts::Entity::update_many()
            .filter(attempts::Column::Id.eq(attempt_id))
            .filter(attempts::Column::EventId.eq(event_id))
            .filter(attempts::Column::ClaimToken.eq(claim_token))
            .filter(attempts::Column::State.eq(attempts::OPEN))
            .filter(attempts::Column::FinishedAt.is_null())
            .filter(attempts::Column::StartedAt.lte(now))
            .set(attempts::ActiveModel {
                state: Set(state.to_owned()), finished_at: Set(Some(now)), ..Default::default()
            }).exec(txn).await?;
        if result.rows_affected != 1 {
            return Err(Error::External(format!("outbox event {event_id} open dispatch attempt was lost")));
        }
        expected.state = state.to_owned();
        expected.finished_at = Some(now);
        let saved = attempts::Entity::find_by_id(attempt_id).one(txn).await?;
        if saved.as_ref() != Some(&expected) {
            return Err(Error::External(format!("outbox event {event_id} attempt outcome readback mismatch")));
        }
        Ok(())
    }

    // The refreshed claim fences the prior worker before this lookup.
    // A previous open intent is ambiguous even if no broker call occurred.
    // Charge exactly one failed attempt, atomically closing the old intent.
    async fn recover_interrupted_attempt(&self, model: &entity::Model) -> Result<Option<DispatchOutcome>> {
        let Some(open) = attempts::Entity::find()
            .filter(attempts::Column::EventId.eq(model.id))
            .filter(attempts::Column::State.eq(attempts::OPEN))
            .one(&self.db).await? else { return Ok(None) };
        if model.claimed_by.as_deref() == Some(open.claim_token.as_str()) {
            return Err(Error::External("current outbox claim already has an open dispatch attempt".into()));
        }
        let error = Error::External("outbox previous dispatch outcome unconfirmed after claim recovery; retry may redeliver".into());
        self.mark_failed_attempt(model, error, false, Some((open.id, open.claim_token.as_str()))).await.map(Some)
    }

    // No broker effect has started, so this exact claim can be released
    // without incrementing the delivery retry budget.
    async fn release_unpublished_claim(&self, model: &entity::Model, attempt: Option<Uuid>) -> Result<()> {
        let txn = self.db.begin().await?;
        self.lock_current_claim(&txn, model).await?;
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
            .exec(&txn)
            .await?;
        self.ensure_claim_owned(model.id, result.rows_affected)?;
        if let Some(id) = attempt {
            let now = rustok_outbox_storage::database_now(&txn).await?;
            self.finish_dispatch_attempt(&txn, model.id, id, Self::claim_token(model)?, attempts::NOT_STARTED, now).await?;
        }
        let mut expected = model.clone();
        expected.claimed_by = None;
        expected.claimed_at = None;
        Self::verify_event_state(&txn, &expected).await?;
        txn.commit().await?;
        Ok(())
    }

    async fn mark_dispatched(&self, model: &entity::Model, attempt: Uuid) -> Result<()> {
        let txn = self.db.begin().await?;
        self.lock_current_claim(&txn, model).await?;
        let now = rustok_outbox_storage::database_now(&txn).await?;
        let result = entity::Entity::update_many()
            .filter(entity::Column::Id.eq(model.id))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.eq(model.claimed_by.clone()))
            .filter(entity::Column::ClaimedAt.eq(model.claimed_at))
            .set(entity::ActiveModel {
                status: Set(SysEventStatus::Dispatched),
                dispatched_at: Set(Some(now)),
                claimed_by: Set(None),
                claimed_at: Set(None),
                last_error: Set(None),
                next_attempt_at: Set(None),
                ..Default::default()
            })
            .exec(&txn)
            .await?;
        self.ensure_claim_owned(model.id, result.rows_affected)?;
        self.finish_dispatch_attempt(&txn, model.id, attempt, Self::claim_token(model)?, attempts::CONFIRMED, now).await?;
        let mut expected = model.clone();
        expected.status = SysEventStatus::Dispatched;
        expected.dispatched_at = Some(now);
        expected.claimed_by = None;
        expected.claimed_at = None;
        expected.last_error = None;
        expected.next_attempt_at = None;
        Self::verify_event_state(&txn, &expected).await?;
        txn.commit().await?;
        Ok(())
    }

    async fn mark_failed_attempt(
        &self,
        model: &entity::Model,
        error: Error,
        terminal: bool,
        attempt: Option<(Uuid, &str)>,
    ) -> Result<DispatchOutcome> {
        let txn = self.db.begin().await?;
        self.lock_current_claim(&txn, model).await?;
        let now = rustok_outbox_storage::database_now(&txn).await?;
        let retry_count = model.retry_count.checked_add(1).unwrap_or(i32::MAX);
        let terminal = terminal || model.retry_count == i32::MAX;
        let (status, next_attempt_at, moved_to_dlq) =
            if terminal || retry_count >= self.config.max_attempts {
                (SysEventStatus::Failed, None, true)
            } else {
                (
                    SysEventStatus::Pending,
                    Some(now
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
                last_error: Set(Some(last_error.clone())),
                claimed_by: Set(None),
                claimed_at: Set(None),
                status: Set(status.clone()),
                next_attempt_at: Set(next_attempt_at),
                ..Default::default()
            })
            .exec(&txn)
            .await?;
        self.ensure_claim_owned(model.id, result.rows_affected)?;
        if let Some((id, token)) = attempt {
            self.finish_dispatch_attempt(&txn, model.id, id, token, attempts::UNCERTAIN, now).await?;
        }
        let mut expected = model.clone();
        expected.retry_count = retry_count;
        expected.last_error = Some(last_error);
        expected.claimed_by = None;
        expected.claimed_at = None;
        expected.status = status;
        expected.next_attempt_at = next_attempt_at;
        Self::verify_event_state(&txn, &expected).await?;
        txn.commit().await?;

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

    #[derive(Default)]
    struct CountUnexpectedPublish(AtomicU64);

    #[async_trait]
    impl EventTransport for CountUnexpectedPublish {
        async fn publish(&self, _envelope: EventEnvelope) -> Result<()> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Err(Error::External("unexpected transport invocation".into()))
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
    async fn invalid_delivery_route_is_rejected_before_claiming_events() {
        let db = database().await;
        let envelope = EventEnvelope::new(
            Uuid::new_v4(),
            None,
            rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
        );
        let id = envelope.id;
        crate::OutboxTransport::new(db.clone()).publish(envelope).await.unwrap();

        for route in [String::new(), " padded ".into(), "x".repeat(129)] {
            let relay = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish))
                .with_delivery_route(route);
            assert!(matches!(relay.process_pending_once(None).await, Err(Error::Validation(_))));
            assert!(matches!(relay.run().await, Err(Error::Validation(_))));
            let row = entity::Entity::find_by_id(id).one(&db).await.unwrap().unwrap();
            assert_eq!(row.status, SysEventStatus::Pending);
            assert!(row.claimed_by.is_none());
            assert!(row.claimed_at.is_none());
            assert_eq!(row.retry_count, 0);
            assert!(checkpoints::Entity::find_by_id(id).one(&db).await.unwrap().is_none());
        }
    }

    #[tokio::test]
    async fn suppressed_dispatch_admission_never_invokes_transport() {
        for table in ["outbox_delivery_checkpoints", "outbox_dispatch_attempts"] {
            let db = database().await;
            let envelope = EventEnvelope::new(
                Uuid::new_v4(), None,
                rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
            );
            let id = envelope.id;
            crate::OutboxTransport::new(db.clone()).publish(envelope).await.unwrap();
            db.execute_unprepared(&format!(
                "CREATE TRIGGER suppress_admission BEFORE INSERT ON {table} BEGIN SELECT RAISE(IGNORE); END"
            )).await.unwrap();
            let transport = Arc::new(CountUnexpectedPublish::default());
            let relay = OutboxRelay::new(db.clone(), transport.clone());
            assert!(relay.process_pending_once(None).await.is_err(), "{table}");
            assert_eq!(transport.0.load(Ordering::Relaxed), 0, "{table}");
            let event = entity::Entity::find_by_id(id).one(&db).await.unwrap().unwrap();
            assert_eq!(event.status, SysEventStatus::Pending);
            assert_eq!(event.retry_count, 0);
            assert!(event.last_error.is_none());
            assert_eq!(relay.metrics().processed_total, 0);
            assert!(checkpoints::Entity::find_by_id(id).one(&db).await.unwrap().is_none());
            assert_eq!(attempts::Entity::find().filter(attempts::Column::EventId.eq(id)).count(&db).await.unwrap(), 0);
        }
    }

    #[tokio::test]
    async fn rewritten_execution_policy_binding_is_rolled_back() {
        let db = database().await;
        db.execute_unprepared(
            "CREATE TRIGGER rewrite_execution_binding AFTER UPDATE OF claim_ttl_ms ON outbox_execution_policy
             WHEN OLD.claim_ttl_ms IS NULL AND NEW.claim_ttl_ms IS NOT NULL
             BEGIN UPDATE outbox_execution_policy SET claim_ttl_ms = NEW.claim_ttl_ms + 1 WHERE id = 1; END",
        ).await.unwrap();
        let config = RelayConfig::default();
        let error = config.verify_execution_admission(&db).await.unwrap_err();
        assert!(error.to_string().contains("execution policy binding readback mismatch"));
        let row = db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite,
            "SELECT claim_ttl_ms FROM outbox_execution_policy WHERE id = 1",
        )).await.unwrap().unwrap();
        assert_eq!(row.try_get::<Option<i64>>("", "claim_ttl_ms").unwrap(), None);
        db.execute_unprepared("DROP TRIGGER rewrite_execution_binding").await.unwrap();
        db.execute_unprepared(
            "CREATE TRIGGER rewrite_retry_binding AFTER UPDATE OF retry_max_attempts ON outbox_execution_policy
             WHEN OLD.retry_max_attempts IS NULL AND NEW.retry_max_attempts IS NOT NULL
             BEGIN UPDATE outbox_execution_policy SET retry_max_attempts = NEW.retry_max_attempts + 1 WHERE id = 1; END",
        ).await.unwrap();
        let error = config.verify_execution_admission(&db).await.unwrap_err();
        assert!(error.to_string().contains("execution policy binding readback mismatch"));
        let row = db.query_one_raw(Statement::from_string(DatabaseBackend::Sqlite,
            "SELECT claim_ttl_ms, retry_max_attempts, retry_backoff_base_ms, retry_backoff_max_ms FROM outbox_execution_policy WHERE id = 1",
        )).await.unwrap().unwrap();
        for column in ["claim_ttl_ms", "retry_max_attempts", "retry_backoff_base_ms", "retry_backoff_max_ms"] {
            assert_eq!(row.try_get::<Option<i64>>("", column).unwrap(), None);
        }
        db.execute_unprepared("DROP TRIGGER rewrite_retry_binding").await.unwrap();
        config.verify_execution_admission(&db).await.unwrap();
    }

    #[tokio::test]
    async fn rewritten_claim_and_refresh_roll_back_the_full_transition() {
        let db = database().await;
        let envelope = EventEnvelope::new(
            Uuid::new_v4(), None,
            rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
        );
        let id = envelope.id;
        crate::OutboxTransport::new(db.clone()).publish(envelope).await.unwrap();
        let before = entity::Entity::find_by_id(id).one(&db).await.unwrap().unwrap();
        let relay = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish));
        let horizon = rustok_outbox_storage::observe_database_clock(&db).await.unwrap();
        // Fault injection isolates the runtime readback from the state guard.
        db.execute_unprepared("DROP TRIGGER outbox_event_state_update_guard").await.unwrap();
        let rewrite = "CREATE TRIGGER rewrite_claim AFTER UPDATE OF claimed_by ON sys_events
             WHEN NEW.claimed_by IS NOT NULL AND NEW.claimed_by IS NOT OLD.claimed_by
             BEGIN UPDATE sys_events SET retry_count = NEW.retry_count + 1 WHERE id = NEW.id; END";
        db.execute_unprepared(rewrite).await.unwrap();
        let error = relay.claim_batch(1, horizon, None).await.unwrap_err();
        assert!(error.to_string().contains("claim batch transition readback mismatch"));
        assert_eq!(entity::Entity::find_by_id(id).one(&db).await.unwrap(), Some(before));
        db.execute_unprepared("DROP TRIGGER rewrite_claim").await.unwrap();

        let claimed = relay.claim_batch(1, horizon, None).await.unwrap().pop().unwrap();
        let mut stale = claimed.clone();
        stale.retry_count += 1;
        assert!(relay.refresh_claim(&stale).await.is_err());
        db.execute_unprepared(rewrite).await.unwrap();
        let error = relay.refresh_claim(&claimed).await.unwrap_err();
        assert!(error.to_string().contains("transition readback mismatch"));
        assert_eq!(entity::Entity::find_by_id(id).one(&db).await.unwrap(), Some(claimed.clone()));
        db.execute_unprepared("DROP TRIGGER rewrite_claim").await.unwrap();
        relay.refresh_claim(&claimed).await.unwrap();
    }

    #[tokio::test]
    async fn suppressed_claim_batch_rolls_back_other_claims() {
        let db = database().await;
        let mut ids = Vec::new();
        for _ in 0..2 {
            let envelope = EventEnvelope::new(
                Uuid::new_v4(), None,
                rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
            );
            ids.push(envelope.id);
            crate::OutboxTransport::new(db.clone()).publish(envelope).await.unwrap();
        }
        db.execute_unprepared(
            "CREATE TRIGGER suppress_claim BEFORE UPDATE OF claimed_by ON sys_events
             WHEN NEW.claimed_by IS NOT NULL AND OLD.id = (SELECT id FROM sys_events ORDER BY id LIMIT 1)
             BEGIN SELECT RAISE(IGNORE); END"
        ).await.unwrap();
        let relay = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish));
        assert!(relay.process_pending_once(None).await.is_err());
        for id in ids {
            let row = entity::Entity::find_by_id(id).one(&db).await.unwrap().unwrap();
            assert!(row.claimed_by.is_none());
            assert_eq!(row.retry_count, 0);
        }
    }

    #[tokio::test]
    async fn rewritten_dispatch_outcomes_roll_back_event_and_attempt_together() {
        for trigger in [
            "CREATE TRIGGER rewrite_outcome AFTER UPDATE OF status ON sys_events WHEN NEW.status = 'dispatched' BEGIN UPDATE sys_events SET retry_count = NEW.retry_count + 1 WHERE id = NEW.id; END",
            "CREATE TRIGGER rewrite_outcome AFTER UPDATE OF state ON outbox_dispatch_attempts WHEN NEW.state = 'confirmed' BEGIN UPDATE outbox_dispatch_attempts SET state = 'uncertain' WHERE id = NEW.id; END",
        ] {
            let db = database().await;
            let envelope = EventEnvelope::new(
                Uuid::new_v4(), None,
                rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
            );
            let id = envelope.id;
            crate::OutboxTransport::new(db.clone()).publish(envelope).await.unwrap();
            let relay = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish));
            let horizon = rustok_outbox_storage::observe_database_clock(&db).await.unwrap();
            let claimed = relay.claim_batch(1, horizon, None).await.unwrap().pop().unwrap();
            let renewed = relay.refresh_claim(&claimed).await.unwrap();
            let AttemptAdmission::Ready(attempt) = relay.begin_dispatch_attempt(&renewed).await.unwrap() else { panic!("intent"); };
            let before = attempts::Entity::find_by_id(attempt).one(&db).await.unwrap().unwrap();
            // Exercise relay readback independently of the database history guard.
            db.execute_unprepared("DROP TRIGGER outbox_attempt_update_guard").await.unwrap();
            db.execute_unprepared(trigger).await.unwrap();
            assert!(relay.mark_dispatched(&renewed, attempt).await.is_err());
            assert_eq!(entity::Entity::find_by_id(id).one(&db).await.unwrap(), Some(renewed.clone()));
            assert_eq!(attempts::Entity::find_by_id(attempt).one(&db).await.unwrap(), Some(before));
            db.execute_unprepared("DROP TRIGGER rewrite_outcome").await.unwrap();
            relay.mark_dispatched(&renewed, attempt).await.unwrap();
        }
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
        RelayConfig::default().verify_execution_admission(&db).await.expect("bind execution policy");
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
        let claim_at = rustok_outbox_storage::database_now(&db).await.unwrap();
        entity::Entity::update_many()
            .filter(entity::Column::Id.eq(event_id))
            .set(entity::ActiveModel {
                claimed_by: Set(Some("queued-worker".to_string())),
                claimed_at: Set(Some(claim_at)),
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

        tokio::time::sleep(Duration::from_millis(2)).await;
        let renewed = relay.refresh_claim(&queued).await.expect("renew claim");
        assert_eq!(renewed.retry_count, 0);
        assert_ne!(renewed.claimed_by, queued.claimed_by);
        assert!(renewed.claimed_at > queued.claimed_at);
        let AttemptAdmission::Ready(attempt) = relay.begin_dispatch_attempt(&renewed).await.expect("admission") else { panic!("intent"); };
        assert!(relay.mark_dispatched(&queued, attempt).await.is_err());
        // A missing attempt must roll back the event ACK in the same transaction.
        assert!(relay.mark_dispatched(&renewed, Uuid::new_v4()).await.is_err());
        let unchanged = entity::Entity::find_by_id(renewed.id).one(&relay.db).await.unwrap().unwrap();
        assert_eq!(unchanged.status, SysEventStatus::Pending);
        assert_eq!(unchanged.claimed_by, renewed.claimed_by);
        relay.mark_dispatched(&renewed, attempt).await.expect("current claim");
        assert_eq!(attempts::Entity::find_by_id(attempt).one(&relay.db).await.unwrap().unwrap().state, attempts::CONFIRMED);
    }

    #[tokio::test]
    async fn confirmed_primary_survives_retry_but_route_cannot_be_rebound() {
        let db = database().await;
        let envelope = EventEnvelope::new(
            Uuid::new_v4(), None,
            rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
        );
        let event_id = envelope.id;
        crate::OutboxTransport::new(db.clone()).publish(envelope).await.unwrap();
        let relay = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish))
            .with_delivery_route("external-fanout-v1")
            .with_config(RelayConfig {
                backoff_base: Duration::from_millis(1),
                backoff_max: Duration::from_millis(1),
                ..RelayConfig::default()
            });
        let horizon = rustok_outbox_storage::observe_database_clock(&db).await.unwrap();
        let claimed = relay.claim_batch(1, horizon, None).await.unwrap().pop().unwrap();
        let renewed = relay.refresh_claim(&claimed).await.unwrap();
        let AttemptAdmission::Ready(attempt_id) = relay.begin_dispatch_attempt(&renewed).await.unwrap() else { panic!("intent"); };
        let scope = DeliveryScope { relay: relay.clone(), model: renewed.clone(), attempt_id };
        DELIVERY_SCOPE.scope(scope, async {
            publish_primary_with_checkpoint(event_id, true, true, async { Ok(()) }).await.unwrap();
        }).await;
        for sql in [
            "UPDATE outbox_delivery_checkpoints SET primary_confirmed_at = NULL",
            "UPDATE outbox_delivery_checkpoints SET local_required = FALSE",
            "DELETE FROM outbox_delivery_checkpoints",
            "DELETE FROM outbox_dispatch_attempts",
            "DELETE FROM sys_events",
        ] {
            assert!(db.execute_unprepared(sql).await.is_err(), "{sql}");
        }
        relay.mark_failed_attempt(&renewed, Error::External("local failure".into()), false, Some((attempt_id, OutboxRelay::claim_token(&renewed).unwrap()))).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;

        // A freshly constructed relay reuses only the committed primary checkpoint.
        let restarted = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish))
            .with_delivery_route("external-fanout-v1")
            .with_config(relay.config.clone());
        let horizon = rustok_outbox_storage::observe_database_clock(&db).await.unwrap();
        let claimed = restarted.claim_batch(1, horizon, None).await.unwrap().pop().unwrap();
        let renewed = restarted.refresh_claim(&claimed).await.unwrap();
        let changed = restarted.clone().with_delivery_route("local-only-v1");
        assert!(matches!(changed.begin_dispatch_attempt(&renewed).await.unwrap(), AttemptAdmission::RouteConflict));
        let AttemptAdmission::Ready(attempt_id) = restarted.begin_dispatch_attempt(&renewed).await.unwrap() else { panic!("intent"); };
        let scope = DeliveryScope { relay: restarted.clone(), model: renewed.clone(), attempt_id };
        DELIVERY_SCOPE.scope(scope, async {
            assert!(publish_primary_with_checkpoint(event_id, false, true, async { Ok(()) }).await.is_err());
            assert!(publish_primary_with_checkpoint(event_id, true, false, async { Ok(()) }).await.is_err());
            publish_primary_with_checkpoint(event_id, true, true, async {
                Err(Error::External("confirmed primary must not be invoked again".into()))
            }).await.unwrap();
        }).await;
        restarted.mark_dispatched(&renewed, attempt_id).await.unwrap();
    }

    #[tokio::test]
    async fn route_conflict_quarantines_without_opening_another_dispatch_intent() {
        let db = database().await;
        let envelope = EventEnvelope::new(
            Uuid::new_v4(), None,
            rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
        );
        let event_id = envelope.id;
        crate::OutboxTransport::new(db.clone()).publish(envelope).await.unwrap();
        let original = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish))
            .with_delivery_route("original-v1");
        let horizon = rustok_outbox_storage::observe_database_clock(&db).await.unwrap();
        let claimed = original.claim_batch(1, horizon, None).await.unwrap().pop().unwrap();
        let renewed = original.refresh_claim(&claimed).await.unwrap();
        let AttemptAdmission::Ready(attempt_id) = original.begin_dispatch_attempt(&renewed).await.unwrap() else {
            panic!("expected dispatch intent");
        };
        original.release_unpublished_claim(&renewed, Some(attempt_id)).await.unwrap();
        let checkpoint = checkpoints::Entity::find_by_id(event_id).one(&db).await.unwrap().unwrap();

        let target = Arc::new(CountUnexpectedPublish::default());
        let changed = OutboxRelay::new(db.clone(), target.clone())
            .with_delivery_route("incompatible-v1");
        // Default budget is five: this must not attempt an illegal pending
        // retry, wait for lease expiry, or consume the whole retry budget.
        let report = changed.process_pending_once_report(None).await.unwrap();
        assert_eq!(report.claimed_count, 1);
        assert_eq!(report.processed_total, 1);
        assert_eq!(report.failure_total, 1);
        assert_eq!(report.dlq_total, 1);
        assert_eq!(report.retry_total, 0);
        assert_eq!(target.0.load(Ordering::Relaxed), 0);

        let failed = entity::Entity::find_by_id(event_id).one(&db).await.unwrap().unwrap();
        assert_eq!(failed.status, SysEventStatus::Failed);
        assert_eq!(failed.retry_count, 1);
        assert!(failed.claimed_by.is_none() && failed.claimed_at.is_none());
        assert!(failed.next_attempt_at.is_none());
        assert!(failed.last_error.as_deref().is_some_and(|error| error.contains("delivery route changed")));
        assert_eq!(
            checkpoints::Entity::find_by_id(event_id).one(&db).await.unwrap().as_ref(),
            Some(&checkpoint),
        );
        let history = attempts::Entity::find()
            .filter(attempts::Column::EventId.eq(event_id))
            .all(&db).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, attempt_id);
        assert_eq!(history[0].state, attempts::NOT_STARTED);
        assert_eq!(changed.process_pending_once(None).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn abandoned_intent_consumes_retry_budget_without_republishing() {
        let db = database().await;
        let envelope = EventEnvelope::new(
            Uuid::new_v4(), None,
            rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
        );
        let id = envelope.id;
        crate::OutboxTransport::new(db.clone()).publish(envelope).await.unwrap();
        let relay = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish)).with_config(
            RelayConfig { max_attempts: 1, ..Default::default() }
        );
        let horizon = rustok_outbox_storage::observe_database_clock(&db).await.unwrap();
        let claimed = relay.claim_batch(1, horizon, None).await.unwrap().pop().unwrap();
        let renewed = relay.refresh_claim(&claimed).await.unwrap();
        let AttemptAdmission::Ready(attempt) = relay.begin_dispatch_attempt(&renewed).await.unwrap() else { panic!("intent"); };
        // Simulate process loss after durable intent, before SQL completion.
        entity::Entity::update_many().filter(entity::Column::Id.eq(id))
            .set(entity::ActiveModel { claimed_at: Set(Some(Utc::now() - chrono::Duration::minutes(2))), ..Default::default() })
            .exec(&db).await.unwrap();
        assert_eq!(relay.process_pending_once(None).await.unwrap(), 1);
        let row = entity::Entity::find_by_id(id).one(&db).await.unwrap().unwrap();
        assert_eq!(row.retry_count, 1);
        assert_eq!(row.status, SysEventStatus::Failed);
        let history = attempts::Entity::find().filter(attempts::Column::EventId.eq(id)).all(&db).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, attempt);
        assert_eq!(history[0].state, attempts::UNCERTAIN);
        assert!(history[0].finished_at.is_some());
        assert!(relay.mark_dispatched(&renewed, attempt).await.is_err());
        assert_eq!(relay.process_pending_once(None).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn proven_pre_call_cancellation_closes_intent_without_retry_charge() {
        let db = database().await;
        let envelope = EventEnvelope::new(
            Uuid::new_v4(), None,
            rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
        );
        let id = envelope.id;
        crate::OutboxTransport::new(db.clone()).publish(envelope).await.unwrap();
        let relay = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish));
        let horizon = rustok_outbox_storage::observe_database_clock(&db).await.unwrap();
        let claimed = relay.claim_batch(1, horizon, None).await.unwrap().pop().unwrap();
        let renewed = relay.refresh_claim(&claimed).await.unwrap();
        let AttemptAdmission::Ready(attempt) = relay.begin_dispatch_attempt(&renewed).await.unwrap() else { panic!("intent"); };
        relay.release_unpublished_claim(&renewed, Some(attempt)).await.unwrap();
        let row = entity::Entity::find_by_id(id).one(&db).await.unwrap().unwrap();
        assert_eq!(row.retry_count, 0);
        assert_eq!(row.status, SysEventStatus::Pending);
        assert!(row.claimed_by.is_none());
        assert_eq!(attempts::Entity::find_by_id(attempt).one(&db).await.unwrap().unwrap().state, attempts::NOT_STARTED);
    }

    #[tokio::test]
    async fn expired_claim_cannot_be_refreshed_without_reclaim() {
        let db = database().await;
        let config = RelayConfig {
            claim_ttl: Duration::from_millis(5),
            publish_timeout: Duration::from_millis(1),
            ..Default::default()
        };
        config.verify_execution_admission(&db).await.expect("bind execution policy");
        let envelope = EventEnvelope::new(
            Uuid::new_v4(), None,
            rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
        );
        let id = envelope.id;
        crate::OutboxTransport::new(db.clone()).publish(envelope).await.unwrap();
        let claim_at = rustok_outbox_storage::database_now(&db).await.unwrap();
        entity::Entity::update_many()
            .filter(entity::Column::Id.eq(id))
            .set(entity::ActiveModel {
                claimed_by: Set(Some("expired-worker".into())),
                claimed_at: Set(Some(claim_at)),
                ..Default::default()
            })
            .exec(&db).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        let row = entity::Entity::find_by_id(id).one(&db).await.unwrap().unwrap();
        let relay = OutboxRelay::new(db, Arc::new(RejectUnexpectedPublish)).with_config(config);
        assert!(relay.refresh_claim(&row).await.is_err());
    }

    #[tokio::test]
    async fn unresolved_attempt_blocks_forged_dispatch_ack_and_retention() {
        let db = database().await;
        let transport = crate::OutboxTransport::new(db.clone());
        let envelope = EventEnvelope::new(
            Uuid::new_v4(), None,
            rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
        );
        let id = envelope.id;
        transport.publish(envelope).await.unwrap();
        let relay = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish));
        let horizon = rustok_outbox_storage::observe_database_clock(&db).await.unwrap();
        let claimed = relay.claim_batch(1, horizon, None).await.unwrap().pop().unwrap();
        let renewed = relay.refresh_claim(&claimed).await.unwrap();
        tokio::time::sleep(Duration::from_millis(2)).await;
        let AttemptAdmission::Ready(attempt) = relay.begin_dispatch_attempt(&renewed).await.unwrap() else { panic!("intent"); };
        // A forged completion timestamp predating the open intent is rejected.
        assert!(entity::Entity::update_many().filter(entity::Column::Id.eq(id))
            .set(entity::ActiveModel {
                status: Set(SysEventStatus::Dispatched),
                dispatched_at: Set(Some(renewed.claimed_at.clone().expect("claim timestamp"))),
                claimed_by: Set(None),
                claimed_at: Set(None),
                ..Default::default()
            }).exec(&db).await.is_err());
        assert!(transport.acknowledge(id).await.is_err());
        let retention = crate::OutboxRetention::new(db.clone()).with_config(
            crate::OutboxRetentionConfig {
                retention: chrono::Duration::milliseconds(1), ..Default::default()
            },
        );
        assert_eq!(retention.prune_once().await.unwrap().pruned, 0);
        assert!(db.execute_unprepared("DELETE FROM sys_events").await.is_err());
        relay.release_unpublished_claim(&renewed, Some(attempt)).await.unwrap();
        transport.acknowledge(id).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(retention.prune_once().await.unwrap().pruned, 1);
    }

    #[tokio::test]
    async fn stale_worker_cannot_complete_a_reclaimed_event() {
        let db = database().await;
        let config = RelayConfig {
            claim_ttl: Duration::from_millis(5),
            publish_timeout: Duration::from_millis(1),
            worker_id: "old-worker".to_string(),
            ..Default::default()
        };
        config.verify_execution_admission(&db).await.expect("bind execution policy");
        let envelope = EventEnvelope::new(
            Uuid::new_v4(), None,
            rustok_events::DomainEvent::UserLoggedIn { user_id: Uuid::new_v4() },
        );
        let event_id = envelope.id;
        crate::OutboxTransport::new(db.clone()).publish(envelope).await.unwrap();
        let first_claim_at = rustok_outbox_storage::database_now(&db).await.unwrap();
        let mut first_claim: entity::ActiveModel = entity::Entity::find_by_id(event_id)
            .one(&db).await.unwrap().unwrap().into();
        first_claim.claimed_by = Set(Some("old-worker".to_string()));
        first_claim.claimed_at = Set(Some(first_claim_at));
        first_claim.update(&db).await.expect("first claim");
        let old_model = entity::Entity::find_by_id(event_id).one(&db).await.unwrap().unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;
        let second_claim_at = rustok_outbox_storage::database_now(&db).await.unwrap();
        let mut reclaimed: entity::ActiveModel = old_model.clone().into();
        reclaimed.claimed_by = Set(Some("new-worker".to_string()));
        reclaimed.claimed_at = Set(Some(second_claim_at));
        reclaimed.update(&db).await.expect("reclaim event");

        let relay = OutboxRelay::new(db.clone(), Arc::new(RejectUnexpectedPublish)).with_config(config);
        assert!(relay.refresh_claim(&old_model).await.is_err());
        assert!(relay.process_claimed_event(&old_model).await.is_err());
        assert!(relay.mark_dispatched(&old_model, Uuid::new_v4()).await.is_err());

        let event = entity::Entity::find_by_id(event_id)
            .one(&db)
            .await
            .expect("query")
            .expect("event");
        assert_eq!(event.status, SysEventStatus::Pending);
        assert_eq!(event.claimed_by.as_deref(), Some("new-worker"));
    }
}
