use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::FutureExt;
use once_cell::sync::Lazy;
use rustok_cache::{
    BoundedCacheInvalidationGapTracker, BoundedInvalidationTrackerError, CacheInvalidationMessage,
    CacheInvalidationObservation, CacheInvalidationOutcome, CacheInvalidationPayloadError,
    CacheService, DurableCacheInvalidationRecord, LocalCacheInvalidationSubscription,
    VersionedCacheInvalidation,
};
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::services::rbac_invalidation_generation::{
    RbacInvalidationGenerationState, ensure_rbac_invalidation_generation_state,
    read_rbac_invalidation_generation,
};
use crate::services::rbac_runtime::{
    invalidate_all_user_permissions_cache, invalidate_user_permissions_cache,
};
use crate::services::server_runtime_context::ServerRuntimeContext;

pub const RBAC_PERMISSION_INVALIDATION_CHANNEL: &str = "rbac.permissions.generation.v1";
const RBAC_PERMISSION_INVALIDATION_CAUSE: &str = "rbac.user.permissions.changed";
const RBAC_PERMISSION_INVALIDATE_ALL_KEY: &str = "*";
const RBAC_PERMISSION_RECONCILE_INTERVAL: Duration = Duration::from_secs(30);
const RBAC_PERMISSION_WORKER_RESTART_DELAY: Duration = Duration::from_secs(1);

static RBAC_INVALIDATION_CACHE_SERVICE: Lazy<RwLock<Option<CacheService>>> =
    Lazy::new(|| RwLock::new(None));

struct AbortOnDropInvalidationTask {
    task: JoinHandle<()>,
}

impl AbortOnDropInvalidationTask {
    fn new(task: JoinHandle<()>) -> Self {
        Self { task }
    }

    fn is_running(&self) -> bool {
        !self.task.is_finished()
    }

    fn abort(&self) {
        self.task.abort();
    }
}

impl Drop for AbortOnDropInvalidationTask {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct RbacCacheInvalidationRuntime {
    local: AbortOnDropInvalidationTask,
    redis: Option<AbortOnDropInvalidationTask>,
    reconcile: AbortOnDropInvalidationTask,
}

impl RbacCacheInvalidationRuntime {
    fn is_running(&self) -> bool {
        self.local.is_running()
            && self.reconcile.is_running()
            && self.redis.as_ref().is_none_or(|redis| redis.is_running())
    }

    fn abort(&self) {
        self.local.abort();
        if let Some(redis) = &self.redis {
            redis.abort();
        }
        self.reconcile.abort();
    }
}

#[derive(Clone)]
pub struct RbacCacheInvalidationListenerHandle(Arc<RbacCacheInvalidationRuntime>);

impl RbacCacheInvalidationListenerHandle {
    fn new(
        local: JoinHandle<()>,
        redis: Option<JoinHandle<()>>,
        reconcile: JoinHandle<()>,
    ) -> Self {
        Self(Arc::new(RbacCacheInvalidationRuntime {
            local: AbortOnDropInvalidationTask::new(local),
            redis: redis.map(AbortOnDropInvalidationTask::new),
            reconcile: AbortOnDropInvalidationTask::new(reconcile),
        }))
    }

    pub fn is_running(&self) -> bool {
        self.0.is_running()
    }

    fn abort(&self) {
        self.0.abort();
    }

    #[cfg(test)]
    fn abort_local(&self) {
        self.0.local.abort();
    }
}

#[derive(Clone, Default)]
struct RbacCacheInvalidationListenerStartLock(Arc<tokio::sync::Mutex<()>>);

#[derive(Debug, Clone, PartialEq, Eq)]
enum RbacInvalidationTarget {
    All,
    User { tenant_id: Uuid, user_id: Uuid },
}

fn acknowledge_rbac_applied_generation(
    tracker: &BoundedCacheInvalidationGapTracker,
    generation: u64,
) -> Result<()> {
    match tracker.acknowledge_applied(RBAC_PERMISSION_INVALIDATION_CHANNEL, generation) {
        Ok(_) => Ok(()),
        Err(BoundedInvalidationTrackerError::Payload(
            CacheInvalidationPayloadError::OffsetRegressed { current, proposed },
        )) if current >= proposed => Ok(()),
        Err(error) => Err(Error::Cache(error.to_string())),
    }
}

fn acknowledge_rbac_recovery(
    tracker: &BoundedCacheInvalidationGapTracker,
    generation: u64,
) -> Result<()> {
    match tracker.acknowledge_recovery(RBAC_PERMISSION_INVALIDATION_CHANNEL, generation) {
        Ok(_) => Ok(()),
        Err(BoundedInvalidationTrackerError::Payload(
            CacheInvalidationPayloadError::OffsetRegressed { current, proposed },
        )) if current >= proposed => Ok(()),
        Err(error) => Err(Error::Cache(error.to_string())),
    }
}

#[derive(Clone)]
struct RbacCacheInvalidationListener {
    db: DatabaseConnection,
    durable_state: RbacInvalidationGenerationState,
    tracker: BoundedCacheInvalidationGapTracker,
}

impl RbacCacheInvalidationListener {
    fn new(db: DatabaseConnection, durable_state: RbacInvalidationGenerationState) -> Self {
        Self {
            db,
            durable_state,
            tracker: BoundedCacheInvalidationGapTracker::default(),
        }
    }

    async fn read_generation(&self) -> Result<u64> {
        read_rbac_invalidation_generation(&self.db).await
    }

    async fn recover_generation_and_clear(&self) -> Result<u64> {
        let recovered_through = self.read_generation().await?;
        invalidate_all_user_permissions_cache().await;
        acknowledge_rbac_recovery(&self.tracker, recovered_through)?;
        self.durable_state.observe_applied(recovered_through);
        Ok(recovered_through)
    }

    async fn reconcile_generation_if_advanced(&self) -> Result<Option<u64>> {
        let recovered_through = self.read_generation().await?;
        let tracker_current = self
            .tracker
            .last_generation(RBAC_PERMISSION_INVALIDATION_CHANNEL);
        let process_current = self.durable_state.current();

        if process_current.is_some_and(|current| current >= recovered_through) {
            if tracker_current.is_none_or(|current| current < recovered_through) {
                acknowledge_rbac_recovery(&self.tracker, recovered_through)?;
            }
            return Ok(None);
        }

        invalidate_all_user_permissions_cache().await;
        acknowledge_rbac_recovery(&self.tracker, recovered_through)?;
        self.durable_state.observe_applied(recovered_through);
        Ok(Some(recovered_through))
    }

    async fn handle_message(&self, message: CacheInvalidationMessage) -> Result<()> {
        let event = VersionedCacheInvalidation::from_message(&message)
            .map_err(|error| Error::Cache(error.to_string()))?;
        if event.channel != RBAC_PERMISSION_INVALIDATION_CHANNEL {
            return Err(Error::Validation(format!(
                "unexpected RBAC cache invalidation channel {}",
                event.channel
            )));
        }

        match self.tracker.observe(&event) {
            CacheInvalidationObservation::InOrder { generation } => {
                match parse_rbac_invalidation_target(&event.key)? {
                    RbacInvalidationTarget::All => {
                        invalidate_all_user_permissions_cache().await;
                    }
                    RbacInvalidationTarget::User { tenant_id, user_id } => {
                        invalidate_user_permissions_cache(&tenant_id, &user_id).await;
                    }
                }
                acknowledge_rbac_applied_generation(&self.tracker, generation)?;
                self.durable_state.observe_applied(generation);
            }
            CacheInvalidationObservation::Duplicate { generation } => {
                self.durable_state.observe_applied(generation);
            }
            CacheInvalidationObservation::Stale { last, .. } => {
                self.durable_state.observe_applied(last);
            }
            CacheInvalidationObservation::UnverifiedFirst { .. }
            | CacheInvalidationObservation::Gap { .. } => {
                if let Some(current) = self
                    .durable_state
                    .current()
                    .filter(|current| *current >= event.generation)
                {
                    acknowledge_rbac_recovery(&self.tracker, current)?;
                } else {
                    let recovered = self.recover_generation_and_clear().await?;
                    if recovered < event.generation {
                        return Err(Error::Cache(format!(
                            "durable RBAC invalidation generation {recovered} trails received {}",
                            event.generation
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}

pub async fn publish_user_rbac_invalidation(
    tenant_id: &Uuid,
    user_id: &Uuid,
    generation: u64,
) -> Result<()> {
    publish_rbac_invalidation(
        Some(*tenant_id),
        rbac_invalidation_key(*tenant_id, *user_id),
        generation,
    )
    .await
}

pub async fn publish_all_rbac_invalidation(generation: u64) -> Result<()> {
    publish_rbac_invalidation(
        None,
        RBAC_PERMISSION_INVALIDATE_ALL_KEY.to_string(),
        generation,
    )
    .await
}

async fn publish_rbac_invalidation(
    tenant_id: Option<Uuid>,
    key: String,
    generation: u64,
) -> Result<()> {
    let Some(cache) = RBAC_INVALIDATION_CACHE_SERVICE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
    else {
        tracing::warn!(
            ?tenant_id,
            %key,
            generation,
            "RBAC distributed invalidation is not initialized; durable generation reconciliation will recover"
        );
        return Ok(());
    };

    let fanout: Result<CacheInvalidationOutcome> = async {
        let emitted_at_unix_ms = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| Error::Cache(error.to_string()))?
                .as_millis(),
        )
        .map_err(|_| Error::Cache("RBAC invalidation timestamp overflow".to_string()))?;
        let record = DurableCacheInvalidationRecord::new(
            Uuid::new_v4(),
            tenant_id,
            RBAC_PERMISSION_INVALIDATION_CHANNEL,
            key.clone(),
            generation,
            emitted_at_unix_ms,
            RBAC_PERMISSION_INVALIDATION_CAUSE,
            None,
        )
        .map_err(|error| Error::Cache(error.to_string()))?;
        cache
            .invalidations()
            .publish_durable(&record)
            .await
            .map_err(|error| Error::Cache(error.to_string()))
    }
    .await;

    let outcome = match fanout {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::warn!(
                ?tenant_id,
                %key,
                generation,
                %error,
                "RBAC invalidation fan-out deferred to durable generation reconciliation"
            );
            rustok_telemetry::metrics::record_event_error(
                RBAC_PERMISSION_INVALIDATION_CHANNEL,
                "fanout_deferred",
            );
            return Ok(());
        }
    };

    if cache.redis_configuration_present() {
        if !outcome.redis_published {
            tracing::warn!(
                ?tenant_id,
                %key,
                generation,
                "RBAC invalidation publication deferred to durable generation reconciliation"
            );
            rustok_telemetry::metrics::record_event_error(
                RBAC_PERMISSION_INVALIDATION_CHANNEL,
                "redis_publish_deferred",
            );
        }
    } else if outcome.local_subscribers == 0 {
        tracing::warn!(
            ?tenant_id,
            %key,
            generation,
            "Local RBAC invalidation delivery deferred to durable generation reconciliation"
        );
        rustok_telemetry::metrics::record_event_error(
            RBAC_PERMISSION_INVALIDATION_CHANNEL,
            "local_publish_deferred",
        );
    }

    Ok(())
}

async fn run_local_invalidation_worker(
    mut local: LocalCacheInvalidationSubscription,
    listener: RbacCacheInvalidationListener,
) {
    loop {
        match local.recv().await {
            Ok(message) => {
                if let Err(error) = listener.handle_message(message).await {
                    tracing::error!(%error, "Local RBAC cache invalidation apply failed");
                    rustok_telemetry::metrics::record_event_error(
                        RBAC_PERMISSION_INVALIDATION_CHANNEL,
                        "local_apply",
                    );
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                tracing::warn!(
                    skipped,
                    "RBAC cache invalidation listener lagged; clearing all permission snapshots"
                );
                if let Err(error) = listener.recover_generation_and_clear().await {
                    tracing::error!(
                        %error,
                        "RBAC cache invalidation recovery after local lag failed"
                    );
                    rustok_telemetry::metrics::record_event_error(
                        RBAC_PERMISSION_INVALIDATION_CHANNEL,
                        "local_recovery",
                    );
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                tracing::error!("Local RBAC cache invalidation subscription closed");
                return;
            }
        }
    }
}

async fn run_redis_invalidation_worker(
    cache: CacheService,
    listener: RbacCacheInvalidationListener,
) {
    let ready_listener = listener.clone();
    let handler_listener = listener;
    let result = cache
        .invalidations()
        .consume_subscription_with_ready(
            RBAC_PERMISSION_INVALIDATION_CHANNEL,
            move || {
                let ready_listener = ready_listener.clone();
                async move {
                    if let Err(error) = ready_listener.recover_generation_and_clear().await {
                        tracing::error!(
                            %error,
                            "RBAC cache recovery after Redis subscribe failed"
                        );
                        rustok_telemetry::metrics::record_event_error(
                            RBAC_PERMISSION_INVALIDATION_CHANNEL,
                            "redis_recovery",
                        );
                    }
                }
            },
            move |message| {
                let handler_listener = handler_listener.clone();
                async move {
                    if let Err(error) = handler_listener.handle_message(message).await {
                        tracing::error!(%error, "Redis RBAC cache invalidation apply failed");
                        rustok_telemetry::metrics::record_event_error(
                            RBAC_PERMISSION_INVALIDATION_CHANNEL,
                            "redis_apply",
                        );
                    }
                }
            },
        )
        .await;
    tracing::warn!(?result, "RBAC Redis invalidation subscription stopped");
}

async fn run_reconcile_invalidation_worker(listener: RbacCacheInvalidationListener) {
    let start = tokio::time::Instant::now() + RBAC_PERMISSION_RECONCILE_INTERVAL;
    let mut interval = tokio::time::interval_at(start, RBAC_PERMISSION_RECONCILE_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        match listener.reconcile_generation_if_advanced().await {
            Ok(Some(generation)) => {
                tracing::warn!(generation, "Reconciled missed RBAC cache invalidations");
            }
            Ok(None) => {}
            Err(error) => {
                tracing::error!(%error, "Periodic RBAC cache invalidation reconciliation failed");
                rustok_telemetry::metrics::record_event_error(
                    RBAC_PERMISSION_INVALIDATION_CHANNEL,
                    "periodic_reconciliation",
                );
            }
        }
    }
}

async fn supervise_rbac_invalidation_worker<F, Fut>(
    worker: &'static str,
    restart_reason: &'static str,
    mut worker_factory: F,
    restart_delay: Duration,
) where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    loop {
        let outcome = AssertUnwindSafe(worker_factory()).catch_unwind().await;
        if outcome.is_err() {
            tracing::error!(
                worker,
                "RBAC cache invalidation worker panicked; restarting"
            );
        } else {
            tracing::error!(worker, "RBAC cache invalidation worker exited; restarting");
        }
        rustok_telemetry::metrics::record_event_error(
            RBAC_PERMISSION_INVALIDATION_CHANNEL,
            restart_reason,
        );
        tokio::time::sleep(restart_delay).await;
    }
}

fn spawn_supervised_rbac_invalidation_worker<F, Fut>(
    worker: &'static str,
    restart_reason: &'static str,
    worker_factory: F,
) -> JoinHandle<()>
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(supervise_rbac_invalidation_worker(
        worker,
        restart_reason,
        worker_factory,
        RBAC_PERMISSION_WORKER_RESTART_DELAY,
    ))
}

pub async fn start_rbac_cache_invalidation_listener(
    ctx: &ServerRuntimeContext,
    cache: CacheService,
) -> Result<()> {
    let _ = ctx.shared_insert_if_absent(RbacCacheInvalidationListenerStartLock::default());
    let start_lock = ctx
        .shared_get::<RbacCacheInvalidationListenerStartLock>()
        .ok_or_else(|| Error::Cache("RBAC invalidation start lock is unavailable".to_string()))?;
    let _start_guard = start_lock.0.lock().await;

    if let Some(existing) = ctx.shared_get::<RbacCacheInvalidationListenerHandle>() {
        if existing.is_running() {
            return Ok(());
        }
        tracing::warn!("RBAC cache invalidation runtime stopped; replacing workers");
        existing.abort();
    }

    let durable_state = ensure_rbac_invalidation_generation_state(ctx);
    let listener = RbacCacheInvalidationListener::new(ctx.db_clone(), durable_state);

    // Subscribe before recovery so a local publication cannot fall into the
    // startup gap. The supervisor takes this receiver on its first attempt and
    // creates a fresh subscription on every later restart.
    let initial_local = cache
        .invalidations()
        .subscribe_local_channel(RBAC_PERMISSION_INVALIDATION_CHANNEL);
    if let Err(error) = listener.recover_generation_and_clear().await {
        tracing::warn!(
            %error,
            "Initial RBAC invalidation recovery is unavailable; durable watchdog will retry"
        );
        invalidate_all_user_permissions_cache().await;
        rustok_telemetry::metrics::record_event_error(
            RBAC_PERMISSION_INVALIDATION_CHANNEL,
            "startup_recovery_deferred",
        );
    }

    let first_local = Arc::new(Mutex::new(Some(initial_local)));
    let local_cache = cache.clone();
    let local_listener = listener.clone();
    let local_task =
        spawn_supervised_rbac_invalidation_worker("local", "local_worker_restart", move || {
            let receiver = first_local
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .unwrap_or_else(|| {
                    local_cache
                        .invalidations()
                        .subscribe_local_channel(RBAC_PERMISSION_INVALIDATION_CHANNEL)
                });
            run_local_invalidation_worker(receiver, local_listener.clone())
        });

    let redis_task = if cache.redis_client_initialized() {
        let redis_cache = cache.clone();
        let redis_listener = listener.clone();
        Some(spawn_supervised_rbac_invalidation_worker(
            "redis",
            "redis_worker_restart",
            move || run_redis_invalidation_worker(redis_cache.clone(), redis_listener.clone()),
        ))
    } else {
        None
    };

    let reconcile_listener = listener;
    let reconcile_task = spawn_supervised_rbac_invalidation_worker(
        "reconcile",
        "reconcile_worker_restart",
        move || run_reconcile_invalidation_worker(reconcile_listener.clone()),
    );

    let runtime = RbacCacheInvalidationListenerHandle::new(local_task, redis_task, reconcile_task);
    ctx.shared_insert(runtime);
    *RBAC_INVALIDATION_CACHE_SERVICE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(cache);
    Ok(())
}

fn rbac_invalidation_key(tenant_id: Uuid, user_id: Uuid) -> String {
    format!("{tenant_id}:{user_id}")
}

fn parse_rbac_invalidation_target(value: &str) -> Result<RbacInvalidationTarget> {
    if value == RBAC_PERMISSION_INVALIDATE_ALL_KEY {
        return Ok(RbacInvalidationTarget::All);
    }
    let (tenant_id, user_id) = parse_rbac_invalidation_key(value)?;
    Ok(RbacInvalidationTarget::User { tenant_id, user_id })
}

fn parse_rbac_invalidation_key(value: &str) -> Result<(Uuid, Uuid)> {
    let (tenant_id, user_id) = value
        .split_once(':')
        .ok_or_else(|| Error::Validation("malformed RBAC cache invalidation key".to_string()))?;
    Ok((
        Uuid::parse_str(tenant_id)
            .map_err(|_| Error::Validation("invalid RBAC invalidation tenant id".to_string()))?,
        Uuid::parse_str(user_id)
            .map_err(|_| Error::Validation("invalid RBAC invalidation user id".to_string()))?,
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::{
        RBAC_PERMISSION_INVALIDATE_ALL_KEY, RBAC_PERMISSION_INVALIDATION_CHANNEL,
        RbacCacheInvalidationListener, RbacCacheInvalidationListenerHandle, RbacInvalidationTarget,
        acknowledge_rbac_applied_generation, acknowledge_rbac_recovery,
        parse_rbac_invalidation_key, parse_rbac_invalidation_target, rbac_invalidation_key,
        supervise_rbac_invalidation_worker,
    };
    use crate::services::rbac_invalidation_generation::RbacInvalidationGenerationState;
    use rustok_cache::{BoundedCacheInvalidationGapTracker, VersionedCacheInvalidation};
    use sea_orm::Database;
    use uuid::Uuid;

    #[test]
    fn rbac_invalidation_key_round_trips() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        assert_eq!(
            parse_rbac_invalidation_target(&rbac_invalidation_key(tenant_id, user_id)).unwrap(),
            RbacInvalidationTarget::User { tenant_id, user_id }
        );
    }

    #[test]
    fn namespace_wide_invalidation_key_is_explicit() {
        assert_eq!(
            parse_rbac_invalidation_target(RBAC_PERMISSION_INVALIDATE_ALL_KEY).unwrap(),
            RbacInvalidationTarget::All
        );
    }

    #[test]
    fn malformed_rbac_invalidation_key_is_rejected() {
        assert!(parse_rbac_invalidation_key("not-a-pair").is_err());
        assert!(parse_rbac_invalidation_key("invalid:also-invalid").is_err());
    }

    #[test]
    fn superseded_rbac_acknowledgements_are_safe_noops() {
        let tracker = BoundedCacheInvalidationGapTracker::default();
        tracker
            .seed(RBAC_PERMISSION_INVALIDATION_CHANNEL, 7)
            .unwrap();

        acknowledge_rbac_applied_generation(&tracker, 6).unwrap();
        acknowledge_rbac_recovery(&tracker, 6).unwrap();

        assert_eq!(
            tracker.last_generation(RBAC_PERMISSION_INVALIDATION_CHANNEL),
            Some(7)
        );
    }

    #[tokio::test]
    async fn watchdog_checkpoint_seeds_unverified_listener_without_database_recovery() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let durable_state = RbacInvalidationGenerationState::default();
        durable_state.observe_applied(5);
        let listener = RbacCacheInvalidationListener::new(db, durable_state);
        let message = VersionedCacheInvalidation::new(
            RBAC_PERMISSION_INVALIDATION_CHANNEL,
            RBAC_PERMISSION_INVALIDATE_ALL_KEY,
            5,
            1,
        )
        .unwrap()
        .to_message()
        .unwrap();

        listener.handle_message(message).await.unwrap();

        assert_eq!(
            listener
                .tracker
                .last_generation(RBAC_PERMISSION_INVALIDATION_CHANNEL),
            Some(5)
        );
    }

    #[tokio::test]
    async fn listener_handle_reports_terminal_workers() {
        let local = tokio::spawn(async { std::future::pending::<()>().await });
        let reconcile = tokio::spawn(async { std::future::pending::<()>().await });
        let handle = RbacCacheInvalidationListenerHandle::new(local, None, reconcile);
        assert!(handle.is_running());
        handle.abort_local();
        tokio::task::yield_now().await;
        assert!(!handle.is_running());
        handle.abort();
    }

    #[tokio::test]
    async fn invalidation_worker_supervisor_restarts_after_panic() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let worker_attempts = attempts.clone();
        let supervisor = tokio::spawn(supervise_rbac_invalidation_worker(
            "test",
            "test_worker_restart",
            move || {
                let attempt = worker_attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        panic!("invalidation worker regression fixture");
                    }
                    std::future::pending::<()>().await;
                }
            },
            Duration::from_millis(1),
        ));

        tokio::time::timeout(Duration::from_secs(1), async {
            while attempts.load(Ordering::SeqCst) < 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("invalidation worker supervisor should restart the worker");
        supervisor.abort();
    }
}
