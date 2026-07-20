use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::FutureExt;
use rustok_cache::{
    CacheInvalidationMessage, CacheService, DurableCacheInvalidationRecord,
    LocalCacheInvalidationSubscription, VersionedCacheInvalidation,
};
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::middleware::channel::invalidate_all_channel_cache_local;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub const CHANNEL_RESOLUTION_INVALIDATION_CHANNEL: &str = "channel.resolution.generation.v1";
const CHANNEL_RESOLUTION_INVALIDATION_CAUSE: &str = "channel.resolution.changed";
const CHANNEL_RESOLUTION_INVALIDATION_KEY: &str = "*";
const CHANNEL_RESOLUTION_RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
const CHANNEL_RESOLUTION_WORKER_RESTART_DELAY: Duration = Duration::from_secs(1);

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

#[derive(Default)]
struct ChannelCacheInvalidationHealth {
    ready: AtomicBool,
}

impl ChannelCacheInvalidationHealth {
    fn mark_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }

    fn mark_failed(&self) {
        self.ready.store(false, Ordering::Release);
    }

    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

struct ChannelCacheInvalidationRuntime {
    local: AbortOnDropInvalidationTask,
    redis: Option<AbortOnDropInvalidationTask>,
    reconcile: AbortOnDropInvalidationTask,
    health: Arc<ChannelCacheInvalidationHealth>,
}

impl ChannelCacheInvalidationRuntime {
    fn is_running(&self) -> bool {
        self.local.is_running()
            && self.reconcile.is_running()
            && self
                .redis
                .as_ref()
                .is_none_or(AbortOnDropInvalidationTask::is_running)
    }

    fn is_ready(&self) -> bool {
        self.is_running() && self.health.is_ready()
    }

    fn abort(&self) {
        self.local.abort();
        if let Some(redis) = &self.redis {
            redis.abort();
        }
        self.reconcile.abort();
        self.health.mark_failed();
    }
}

#[derive(Clone)]
pub struct ChannelCacheInvalidationListenerHandle(Arc<ChannelCacheInvalidationRuntime>);

impl ChannelCacheInvalidationListenerHandle {
    fn new(
        local: JoinHandle<()>,
        redis: Option<JoinHandle<()>>,
        reconcile: JoinHandle<()>,
        health: Arc<ChannelCacheInvalidationHealth>,
    ) -> Self {
        Self(Arc::new(ChannelCacheInvalidationRuntime {
            local: AbortOnDropInvalidationTask::new(local),
            redis: redis.map(AbortOnDropInvalidationTask::new),
            reconcile: AbortOnDropInvalidationTask::new(reconcile),
            health,
        }))
    }

    pub fn is_running(&self) -> bool {
        self.0.is_running()
    }

    pub fn is_ready(&self) -> bool {
        self.0.is_ready()
    }

    fn abort(&self) {
        self.0.abort();
    }
}

#[derive(Clone, Default)]
struct ChannelCacheInvalidationListenerStartLock(Arc<tokio::sync::Mutex<()>>);

#[derive(Clone, Default)]
struct AppliedChannelResolutionGeneration(Arc<RwLock<Option<u64>>>);

impl AppliedChannelResolutionGeneration {
    fn current(&self) -> Option<u64> {
        *self
            .0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn replace(&self, generation: u64) {
        *self
            .0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(generation);
    }
}

#[derive(Clone)]
struct ChannelCacheInvalidationListener {
    ctx: ServerRuntimeContext,
    db: DatabaseConnection,
    applied: AppliedChannelResolutionGeneration,
    reconcile_lock: Arc<tokio::sync::Mutex<()>>,
    health: Arc<ChannelCacheInvalidationHealth>,
}

impl ChannelCacheInvalidationListener {
    fn new(
        ctx: ServerRuntimeContext,
        health: Arc<ChannelCacheInvalidationHealth>,
    ) -> Self {
        Self {
            db: ctx.db_clone(),
            ctx,
            applied: AppliedChannelResolutionGeneration::default(),
            reconcile_lock: Arc::new(tokio::sync::Mutex::new(())),
            health,
        }
    }

    async fn read_generation(&self) -> Result<u64> {
        rustok_channel::read_resolution_invalidation_generation(&self.db)
            .await
            .map_err(|error| Error::Cache(error.to_string()))
    }

    async fn reconcile_generation(&self) -> Result<Option<u64>> {
        let result = self.reconcile_generation_inner().await;
        match &result {
            Ok(_) => self.health.mark_ready(),
            Err(_) => self.health.mark_failed(),
        }
        result
    }

    async fn reconcile_generation_inner(&self) -> Result<Option<u64>> {
        let _guard = self.reconcile_lock.lock().await;
        let generation = self.read_generation().await?;
        let previous = self.applied.current();
        if previous == Some(generation) {
            return Ok(None);
        }

        invalidate_all_channel_cache_local(&self.ctx).await;
        self.applied.replace(generation);
        if let Some(previous) = previous
            && generation < previous
        {
            tracing::error!(
                previous,
                current = generation,
                "Durable channel resolution generation regressed; cache baseline was rebuilt"
            );
            rustok_telemetry::metrics::record_event_error(
                CHANNEL_RESOLUTION_INVALIDATION_CHANNEL,
                "generation_regressed",
            );
        }
        Ok(Some(generation))
    }

    async fn handle_message(&self, message: CacheInvalidationMessage) -> Result<()> {
        let result = self.handle_message_inner(message).await;
        if result.is_err() {
            self.health.mark_failed();
        }
        result
    }

    async fn handle_message_inner(&self, message: CacheInvalidationMessage) -> Result<()> {
        let event = VersionedCacheInvalidation::from_message(&message)
            .map_err(|error| Error::Cache(error.to_string()))?;
        if event.channel != CHANNEL_RESOLUTION_INVALIDATION_CHANNEL {
            return Err(Error::Validation(format!(
                "unexpected channel cache invalidation channel {}",
                event.channel
            )));
        }
        if event.key != CHANNEL_RESOLUTION_INVALIDATION_KEY {
            return Err(Error::Validation(format!(
                "unexpected channel cache invalidation key {}",
                event.key
            )));
        }

        let reconciled = self.reconcile_generation().await?;
        let durable = reconciled.or_else(|| self.applied.current()).unwrap_or(0);
        if durable < event.generation {
            return Err(Error::Cache(format!(
                "durable channel resolution generation {durable} trails received {}",
                event.generation
            )));
        }
        Ok(())
    }
}

pub async fn publish_channel_resolution_invalidation(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
) {
    let generation = match rustok_channel::read_resolution_invalidation_generation(ctx.db()).await {
        Ok(generation) => generation,
        Err(error) => {
            tracing::error!(
                %tenant_id,
                %error,
                "Failed to read durable channel resolution generation after mutation"
            );
            rustok_telemetry::metrics::record_event_error(
                CHANNEL_RESOLUTION_INVALIDATION_CHANNEL,
                "generation_read_after_mutation",
            );
            return;
        }
    };

    let Some(cache) = ctx.shared_get::<CacheService>() else {
        tracing::warn!(
            %tenant_id,
            generation,
            "Channel cache invalidation fan-out is not initialized; periodic database reconciliation will recover"
        );
        return;
    };

    let emitted_at_unix_ms = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => elapsed.as_millis().min(u128::from(u64::MAX)) as u64,
        Err(error) => {
            tracing::error!(%error, "Channel cache invalidation timestamp precedes Unix epoch");
            return;
        }
    };
    let record = match DurableCacheInvalidationRecord::new(
        Uuid::new_v4(),
        Some(tenant_id),
        CHANNEL_RESOLUTION_INVALIDATION_CHANNEL,
        CHANNEL_RESOLUTION_INVALIDATION_KEY,
        generation,
        emitted_at_unix_ms,
        CHANNEL_RESOLUTION_INVALIDATION_CAUSE,
        None,
    ) {
        Ok(record) => record,
        Err(error) => {
            tracing::error!(
                %tenant_id,
                generation,
                %error,
                "Invalid channel cache invalidation record"
            );
            return;
        }
    };

    match cache.invalidations().publish_durable(&record).await {
        Ok(outcome) => {
            if cache.redis_configuration_present() && !outcome.redis_published {
                tracing::warn!(
                    %tenant_id,
                    generation,
                    "Channel cache invalidation Redis fan-out failed; durable reconciliation will recover"
                );
                rustok_telemetry::metrics::record_event_error(
                    CHANNEL_RESOLUTION_INVALIDATION_CHANNEL,
                    "redis_publish_deferred",
                );
            } else if !cache.redis_configuration_present() && outcome.local_subscribers == 0 {
                tracing::warn!(
                    %tenant_id,
                    generation,
                    "Channel cache invalidation has no local subscriber; durable reconciliation will recover"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                %tenant_id,
                generation,
                %error,
                "Channel cache invalidation fan-out failed; durable reconciliation will recover"
            );
            rustok_telemetry::metrics::record_event_error(
                CHANNEL_RESOLUTION_INVALIDATION_CHANNEL,
                "fanout_deferred",
            );
        }
    }
}

async fn run_local_worker(
    mut local: LocalCacheInvalidationSubscription,
    listener: ChannelCacheInvalidationListener,
) {
    loop {
        match local.recv().await {
            Ok(message) => {
                if let Err(error) = listener.handle_message(message).await {
                    tracing::error!(%error, "Local channel cache invalidation apply failed");
                    rustok_telemetry::metrics::record_event_error(
                        CHANNEL_RESOLUTION_INVALIDATION_CHANNEL,
                        "local_apply",
                    );
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                listener.health.mark_failed();
                tracing::warn!(skipped, "Channel cache invalidation listener lagged");
                if let Err(error) = listener.reconcile_generation().await {
                    tracing::error!(%error, "Channel cache recovery after local lag failed");
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                listener.health.mark_failed();
                tracing::error!("Local channel cache invalidation subscription closed");
                return;
            }
        }
    }
}

async fn run_redis_worker(cache: CacheService, listener: ChannelCacheInvalidationListener) {
    let ready_listener = listener.clone();
    let handler_listener = listener.clone();
    let result = cache
        .invalidations()
        .consume_subscription_with_ready(
            CHANNEL_RESOLUTION_INVALIDATION_CHANNEL,
            move || {
                let ready_listener = ready_listener.clone();
                async move {
                    if let Err(error) = ready_listener.reconcile_generation().await {
                        tracing::error!(
                            %error,
                            "Channel cache recovery after Redis subscribe failed"
                        );
                    }
                }
            },
            move |message| {
                let handler_listener = handler_listener.clone();
                async move {
                    if let Err(error) = handler_listener.handle_message(message).await {
                        tracing::error!(
                            %error,
                            "Redis channel cache invalidation apply failed"
                        );
                    }
                }
            },
        )
        .await;
    listener.health.mark_failed();
    tracing::warn!(?result, "Channel cache Redis invalidation subscription stopped");
}

async fn run_reconcile_worker(listener: ChannelCacheInvalidationListener) {
    let start = tokio::time::Instant::now() + CHANNEL_RESOLUTION_RECONCILE_INTERVAL;
    let mut interval = tokio::time::interval_at(start, CHANNEL_RESOLUTION_RECONCILE_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        match listener.reconcile_generation().await {
            Ok(Some(generation)) => {
                tracing::warn!(generation, "Reconciled missed channel cache invalidation");
            }
            Ok(None) => {}
            Err(error) if is_missing_generation_state(&error) => {
                tracing::debug!(
                    "Durable channel resolution generation is not installed yet; reconciliation will retry"
                );
            }
            Err(error) => {
                tracing::error!(
                    %error,
                    "Periodic channel cache invalidation reconciliation failed"
                );
                rustok_telemetry::metrics::record_event_error(
                    CHANNEL_RESOLUTION_INVALIDATION_CHANNEL,
                    "periodic_reconciliation",
                );
            }
        }
    }
}

async fn supervise_worker<F, Fut>(
    worker: &'static str,
    restart_reason: &'static str,
    mut worker_factory: F,
) where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    loop {
        let outcome = AssertUnwindSafe(worker_factory()).catch_unwind().await;
        if outcome.is_err() {
            tracing::error!(worker, "Channel cache invalidation worker panicked; restarting");
        } else {
            tracing::error!(worker, "Channel cache invalidation worker exited; restarting");
        }
        rustok_telemetry::metrics::record_event_error(
            CHANNEL_RESOLUTION_INVALIDATION_CHANNEL,
            restart_reason,
        );
        tokio::time::sleep(CHANNEL_RESOLUTION_WORKER_RESTART_DELAY).await;
    }
}

fn spawn_supervised_worker<F, Fut>(
    worker: &'static str,
    restart_reason: &'static str,
    worker_factory: F,
) -> JoinHandle<()>
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(supervise_worker(
        worker,
        restart_reason,
        worker_factory,
    ))
}

pub async fn start_channel_cache_invalidation_listener(
    ctx: &ServerRuntimeContext,
    cache: CacheService,
) -> Result<()> {
    let _ = ctx.shared_insert_if_absent(ChannelCacheInvalidationListenerStartLock::default());
    let start_lock = ctx
        .shared_get::<ChannelCacheInvalidationListenerStartLock>()
        .ok_or_else(|| Error::Cache("channel invalidation start lock is unavailable".to_string()))?;
    let _start_guard = start_lock.0.lock().await;

    if let Some(existing) = ctx.shared_get::<ChannelCacheInvalidationListenerHandle>() {
        if existing.is_running() {
            return Ok(());
        }
        tracing::warn!("Channel cache invalidation runtime stopped; replacing workers");
        existing.abort();
    }

    let health = Arc::new(ChannelCacheInvalidationHealth::default());
    let listener = ChannelCacheInvalidationListener::new(ctx.clone(), health.clone());
    let initial_local = cache
        .invalidations()
        .subscribe_local_channel(CHANNEL_RESOLUTION_INVALIDATION_CHANNEL);
    if let Err(error) = listener.reconcile_generation().await {
        if !is_missing_generation_state(&error) {
            tracing::warn!(%error, "Initial channel cache generation recovery deferred");
        }
    }

    let first_local = Arc::new(Mutex::new(Some(initial_local)));
    let local_cache = cache.clone();
    let local_listener = listener.clone();
    let local_task = spawn_supervised_worker("local", "local_worker_restart", move || {
        let receiver = first_local
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .unwrap_or_else(|| {
                local_cache
                    .invalidations()
                    .subscribe_local_channel(CHANNEL_RESOLUTION_INVALIDATION_CHANNEL)
            });
        run_local_worker(receiver, local_listener.clone())
    });

    let redis_task = if cache.redis_client_initialized() {
        let redis_cache = cache.clone();
        let redis_listener = listener.clone();
        Some(spawn_supervised_worker(
            "redis",
            "redis_worker_restart",
            move || run_redis_worker(redis_cache.clone(), redis_listener.clone()),
        ))
    } else {
        None
    };

    let reconcile_listener = listener;
    let reconcile_task = spawn_supervised_worker(
        "reconcile",
        "reconcile_worker_restart",
        move || run_reconcile_worker(reconcile_listener.clone()),
    );

    ctx.shared_insert(ChannelCacheInvalidationListenerHandle::new(
        local_task,
        redis_task,
        reconcile_task,
        health,
    ));
    Ok(())
}

fn is_missing_generation_state(error: &Error) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("channel_resolution_invalidation_state")
        && (message.contains("no such table")
            || message.contains("undefinedtable")
            || message.contains("does not exist"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        is_missing_generation_state, AppliedChannelResolutionGeneration,
        ChannelCacheInvalidationHealth,
    };
    use crate::error::Error;

    #[test]
    fn applied_generation_can_rebaseline_after_database_restore() {
        let state = AppliedChannelResolutionGeneration::default();
        state.replace(9);
        state.replace(3);
        assert_eq!(state.current(), Some(3));
    }

    #[test]
    fn readiness_requires_successful_reconciliation() {
        let health = Arc::new(ChannelCacheInvalidationHealth::default());
        assert!(!health.is_ready());
        health.mark_ready();
        assert!(health.is_ready());
        health.mark_failed();
        assert!(!health.is_ready());
    }

    #[test]
    fn missing_generation_table_is_recognized() {
        assert!(is_missing_generation_state(&Error::Cache(
            "no such table: channel_resolution_invalidation_state".to_string()
        )));
        assert!(!is_missing_generation_state(&Error::Cache(
            "no such table: unrelated_table".to_string()
        )));
        assert!(!is_missing_generation_state(&Error::Cache(
            "connection refused".to_string()
        )));
    }
}
