use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::FutureExt;
use rustok_cache::{
    BoundedCacheInvalidationGapTracker, BoundedInvalidationTrackerError, CacheGenerationError,
    CacheInvalidationMessage, CacheInvalidationObservation, CacheInvalidationPayloadError,
    CacheService, LocalCacheInvalidationSubscription, VersionedCacheInvalidation,
    cache_backend_generation_snapshot,
};
use rustok_core::{Error, Result};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::middleware::locale::{
    invalidate_all_tenant_locale_cache, invalidate_tenant_locale_cache,
};
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::tenant_cache_generation::{
    TENANT_CACHE_BACKEND_PREFIX, TENANT_CACHE_GENERATION_CHANNEL,
};

const TENANT_LOCALE_LISTENER_RESTART_DELAY: Duration = Duration::from_secs(1);
const TENANT_LOCALE_RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Default)]
struct TenantLocaleGenerationHealth {
    ready: AtomicBool,
}

impl TenantLocaleGenerationHealth {
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

#[derive(Clone)]
struct TenantLocaleGenerationListener {
    ctx: ServerRuntimeContext,
    cache: CacheService,
    tracker: BoundedCacheInvalidationGapTracker,
    health: Arc<TenantLocaleGenerationHealth>,
}

impl TenantLocaleGenerationListener {
    fn new(
        ctx: ServerRuntimeContext,
        cache: CacheService,
        health: Arc<TenantLocaleGenerationHealth>,
    ) -> Self {
        Self {
            ctx,
            cache,
            tracker: BoundedCacheInvalidationGapTracker::default(),
            health,
        }
    }

    async fn current_generation(&self) -> Result<u64> {
        if self.cache.redis_configuration_present() {
            if !self.cache.redis_client_initialized() {
                return Err(Error::Cache(
                    "Redis is configured but the tenant locale generation client is unavailable"
                        .to_string(),
                ));
            }
            return match self
                .cache
                .namespace_generations()
                .read(TENANT_CACHE_BACKEND_PREFIX)
                .await
            {
                Ok(generation) => Ok(generation.value()),
                Err(CacheGenerationError::GenerationRegressed { local, shared }) => {
                    invalidate_all_tenant_locale_cache(&self.ctx).await;
                    tracing::error!(
                        local,
                        shared,
                        "Tenant locale shared generation regressed; cache cleared and readiness remains failed"
                    );
                    rustok_telemetry::metrics::record_event_error(
                        "tenant.locale.generation",
                        "generation_regressed",
                    );
                    Err(Error::Cache(format!(
                        "tenant locale shared generation regressed from {local} to {shared}"
                    )))
                }
                Err(error) => Err(Error::Cache(error.to_string())),
            };
        }

        let snapshot = cache_backend_generation_snapshot(TENANT_CACHE_BACKEND_PREFIX)
            .map_err(|error| Error::Cache(error.to_string()))?;
        Ok(if snapshot.trusted {
            snapshot.generation
        } else {
            0
        })
    }

    async fn recover_if_advanced(&self) -> Result<u64> {
        let result = self.recover_if_advanced_inner().await;
        match &result {
            Ok(_) => self.health.mark_ready(),
            Err(_) => self.health.mark_failed(),
        }
        result
    }

    async fn recover_if_advanced_inner(&self) -> Result<u64> {
        let generation = self.current_generation().await?;
        let previous = self
            .tracker
            .last_generation(TENANT_CACHE_GENERATION_CHANNEL);

        match previous {
            None => invalidate_all_tenant_locale_cache(&self.ctx).await,
            Some(previous) if generation == previous => {}
            Some(previous) if generation > previous => {
                invalidate_all_tenant_locale_cache(&self.ctx).await;
            }
            Some(previous) => {
                invalidate_all_tenant_locale_cache(&self.ctx).await;
                tracing::error!(
                    previous,
                    current = generation,
                    "Tenant locale generation regressed; cache cleared and readiness remains failed"
                );
                rustok_telemetry::metrics::record_event_error(
                    "tenant.locale.generation",
                    "generation_regressed",
                );
                return Err(Error::Cache(format!(
                    "tenant locale generation regressed from {previous} to {generation}"
                )));
            }
        }

        acknowledge_locale_recovery(&self.tracker, generation)?;
        Ok(generation)
    }

    async fn handle_message(&self, message: CacheInvalidationMessage) -> Result<()> {
        let result = async {
            let event = VersionedCacheInvalidation::from_message(&message)
                .map_err(|error| Error::Cache(error.to_string()))?;
            if event.channel != TENANT_CACHE_GENERATION_CHANNEL {
                return Err(Error::Validation(format!(
                    "unexpected tenant locale invalidation channel {}",
                    event.channel
                )));
            }

            let durable = self.current_generation().await?;
            if durable < event.generation {
                return Err(Error::Cache(format!(
                    "shared tenant locale generation {durable} trails received {}",
                    event.generation
                )));
            }
            self.handle_event(event, durable).await
        }
        .await;
        match &result {
            Ok(()) => self.health.mark_ready(),
            Err(_) => self.health.mark_failed(),
        }
        result
    }

    async fn handle_event(&self, event: VersionedCacheInvalidation, durable: u64) -> Result<()> {
        match self.tracker.observe(&event) {
            CacheInvalidationObservation::InOrder { generation } => {
                if durable > generation {
                    invalidate_all_tenant_locale_cache(&self.ctx).await;
                    acknowledge_locale_recovery(&self.tracker, durable)?;
                    return Ok(());
                }

                if event.key == "*" {
                    invalidate_all_tenant_locale_cache(&self.ctx).await;
                } else {
                    let tenant_id = Uuid::parse_str(event.key.trim()).map_err(|_| {
                        Error::Validation(
                            "tenant locale generation key must contain a tenant UUID or *"
                                .to_string(),
                        )
                    })?;
                    invalidate_tenant_locale_cache(&self.ctx, tenant_id).await;
                }
                acknowledge_locale_applied(&self.tracker, generation)?;
            }
            CacheInvalidationObservation::Duplicate { generation } => {
                if durable > generation {
                    invalidate_all_tenant_locale_cache(&self.ctx).await;
                    acknowledge_locale_recovery(&self.tracker, durable)?;
                }
            }
            CacheInvalidationObservation::Stale { last, .. } => {
                if durable < last {
                    return Err(Error::Cache(format!(
                        "shared tenant locale generation {durable} regressed below applied {last}"
                    )));
                }
                if durable > last {
                    invalidate_all_tenant_locale_cache(&self.ctx).await;
                    acknowledge_locale_recovery(&self.tracker, durable)?;
                }
            }
            CacheInvalidationObservation::UnverifiedFirst { .. }
            | CacheInvalidationObservation::Gap { .. } => {
                invalidate_all_tenant_locale_cache(&self.ctx).await;
                acknowledge_locale_recovery(&self.tracker, durable)?;
            }
        }
        Ok(())
    }
}

fn acknowledge_locale_applied(
    tracker: &BoundedCacheInvalidationGapTracker,
    generation: u64,
) -> Result<()> {
    match tracker.acknowledge_applied(TENANT_CACHE_GENERATION_CHANNEL, generation) {
        Ok(_) => Ok(()),
        Err(BoundedInvalidationTrackerError::Payload(
            CacheInvalidationPayloadError::OffsetRegressed { current, proposed },
        )) if current >= proposed => Ok(()),
        Err(error) => Err(Error::Cache(error.to_string())),
    }
}

fn acknowledge_locale_recovery(
    tracker: &BoundedCacheInvalidationGapTracker,
    generation: u64,
) -> Result<()> {
    match tracker.acknowledge_recovery(TENANT_CACHE_GENERATION_CHANNEL, generation) {
        Ok(_) => Ok(()),
        Err(BoundedInvalidationTrackerError::Payload(
            CacheInvalidationPayloadError::OffsetRegressed { current, proposed },
        )) if current >= proposed => Ok(()),
        Err(error) => Err(Error::Cache(error.to_string())),
    }
}

struct AbortOnDropTenantLocaleTask {
    task: JoinHandle<()>,
}

impl AbortOnDropTenantLocaleTask {
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

impl Drop for AbortOnDropTenantLocaleTask {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct TenantLocaleGenerationRuntime {
    local: AbortOnDropTenantLocaleTask,
    redis: Option<AbortOnDropTenantLocaleTask>,
    reconcile: Option<AbortOnDropTenantLocaleTask>,
    redis_required: bool,
    health: Arc<TenantLocaleGenerationHealth>,
}

impl TenantLocaleGenerationRuntime {
    fn is_running(&self) -> bool {
        self.local.is_running()
            && (!self.redis_required || self.redis.as_ref().is_some_and(|task| task.is_running()))
            && (!self.redis_required
                || self
                    .reconcile
                    .as_ref()
                    .is_some_and(|task| task.is_running()))
    }

    fn is_ready(&self) -> bool {
        self.is_running() && self.health.is_ready()
    }

    fn abort(&self) {
        self.local.abort();
        if let Some(redis) = &self.redis {
            redis.abort();
        }
        if let Some(reconcile) = &self.reconcile {
            reconcile.abort();
        }
        self.health.mark_failed();
    }
}

#[derive(Clone)]
pub struct TenantLocaleGenerationListenerHandle(Arc<TenantLocaleGenerationRuntime>);

impl TenantLocaleGenerationListenerHandle {
    fn new(
        local: JoinHandle<()>,
        redis: Option<JoinHandle<()>>,
        reconcile: Option<JoinHandle<()>>,
        redis_required: bool,
        health: Arc<TenantLocaleGenerationHealth>,
    ) -> Self {
        Self(Arc::new(TenantLocaleGenerationRuntime {
            local: AbortOnDropTenantLocaleTask::new(local),
            redis: redis.map(AbortOnDropTenantLocaleTask::new),
            reconcile: reconcile.map(AbortOnDropTenantLocaleTask::new),
            redis_required,
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
struct TenantLocaleGenerationStartLock(Arc<tokio::sync::Mutex<()>>);

pub async fn start_tenant_locale_generation_listener(
    ctx: &ServerRuntimeContext,
    cache: CacheService,
) {
    let _ = ctx.shared_insert_if_absent(TenantLocaleGenerationStartLock::default());
    let start_lock = ctx
        .shared_get::<TenantLocaleGenerationStartLock>()
        .expect("tenant locale generation start lock must exist after registration");
    let _start_guard = start_lock.0.lock().await;

    if let Some(existing) = ctx.shared_get::<TenantLocaleGenerationListenerHandle>() {
        if existing.is_running() {
            return;
        }
        tracing::warn!("Tenant locale generation listener stopped; replacing runtime");
        existing.abort();
    }

    let health = Arc::new(TenantLocaleGenerationHealth::default());
    let listener =
        TenantLocaleGenerationListener::new(ctx.clone(), cache.clone(), Arc::clone(&health));
    let initial_local = cache
        .invalidations()
        .subscribe_local_channel(TENANT_CACHE_GENERATION_CHANNEL);
    if let Err(error) = listener.recover_if_advanced().await {
        tracing::warn!(%error, "Tenant locale generation startup recovery failed");
        rustok_telemetry::metrics::record_event_error(
            "tenant.locale.generation",
            "startup_recovery",
        );
    }

    let local_task = tokio::spawn(supervise_local_listener(listener.clone(), initial_local));
    let redis_required = cache.redis_configuration_present();
    let redis_task = cache
        .redis_client_initialized()
        .then(|| tokio::spawn(supervise_redis_listener(listener.clone())));
    let reconcile_task =
        redis_required.then(|| tokio::spawn(run_periodic_reconciliation(listener)));

    ctx.shared_insert(TenantLocaleGenerationListenerHandle::new(
        local_task,
        redis_task,
        reconcile_task,
        redis_required,
        health,
    ));
}

async fn supervise_local_listener(
    listener: TenantLocaleGenerationListener,
    initial: LocalCacheInvalidationSubscription,
) {
    let mut initial = Some(initial);
    loop {
        let subscription = initial.take().unwrap_or_else(|| {
            listener
                .cache
                .invalidations()
                .subscribe_local_channel(TENANT_CACHE_GENERATION_CHANNEL)
        });
        let outcome = AssertUnwindSafe(run_local_listener(listener.clone(), subscription))
            .catch_unwind()
            .await;
        listener.health.mark_failed();
        if outcome.is_err() {
            tracing::error!("Tenant locale generation local listener panicked; restarting");
        } else {
            tracing::warn!("Tenant locale generation local listener exited; restarting");
        }
        rustok_telemetry::metrics::record_event_error("tenant.locale.generation", "local_restart");
        tokio::time::sleep(TENANT_LOCALE_LISTENER_RESTART_DELAY).await;
    }
}

async fn run_local_listener(
    listener: TenantLocaleGenerationListener,
    mut subscription: LocalCacheInvalidationSubscription,
) {
    loop {
        match subscription.recv().await {
            Ok(message) => {
                if let Err(error) = listener.handle_message(message).await {
                    tracing::error!(%error, "Tenant locale generation local apply failed");
                    rustok_telemetry::metrics::record_event_error(
                        "tenant.locale.generation",
                        "local_apply",
                    );
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                listener.health.mark_failed();
                tracing::warn!(skipped, "Tenant locale generation local listener lagged");
                if let Err(error) = listener.recover_if_advanced().await {
                    tracing::error!(%error, "Tenant locale generation lag recovery failed");
                    rustok_telemetry::metrics::record_event_error(
                        "tenant.locale.generation",
                        "local_lag_recovery",
                    );
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                listener.health.mark_failed();
                return;
            }
        }
    }
}

async fn supervise_redis_listener(listener: TenantLocaleGenerationListener) {
    loop {
        let outcome = AssertUnwindSafe(run_redis_listener(listener.clone()))
            .catch_unwind()
            .await;
        listener.health.mark_failed();
        match outcome {
            Ok(Ok(())) => {
                tracing::warn!("Tenant locale generation Redis listener exited; reconnecting")
            }
            Ok(Err(error)) => {
                tracing::warn!(%error, "Tenant locale generation Redis listener failed; reconnecting")
            }
            Err(_) => {
                tracing::error!("Tenant locale generation Redis listener panicked; reconnecting")
            }
        }
        rustok_telemetry::metrics::record_event_error("tenant.locale.generation", "redis_restart");
        tokio::time::sleep(TENANT_LOCALE_LISTENER_RESTART_DELAY).await;
    }
}

async fn run_redis_listener(listener: TenantLocaleGenerationListener) -> Result<()> {
    let invalidations = listener.cache.invalidations();
    let ready_listener = listener.clone();
    let handler_listener = listener;
    invalidations
        .consume_subscription_with_ready(
            TENANT_CACHE_GENERATION_CHANNEL,
            move || async move {
                if let Err(error) = ready_listener.recover_if_advanced().await {
                    tracing::error!(%error, "Tenant locale generation Redis ready recovery failed");
                    rustok_telemetry::metrics::record_event_error(
                        "tenant.locale.generation",
                        "redis_ready_recovery",
                    );
                }
            },
            move |message| {
                let handler_listener = handler_listener.clone();
                async move {
                    if let Err(error) = handler_listener.handle_message(message).await {
                        tracing::error!(%error, "Tenant locale generation Redis apply failed");
                        rustok_telemetry::metrics::record_event_error(
                            "tenant.locale.generation",
                            "redis_apply",
                        );
                    }
                }
            },
        )
        .await
        .map_err(Error::Cache)
}

async fn run_periodic_reconciliation(listener: TenantLocaleGenerationListener) {
    run_periodic_reconciliation_with_interval(listener, TENANT_LOCALE_RECONCILE_INTERVAL).await;
}

async fn run_periodic_reconciliation_with_interval(
    listener: TenantLocaleGenerationListener,
    reconcile_interval: Duration,
) {
    let start = tokio::time::Instant::now() + reconcile_interval;
    let mut interval = tokio::time::interval_at(start, reconcile_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        if let Err(error) = listener.recover_if_advanced().await {
            tracing::warn!(%error, "Tenant locale generation periodic reconciliation failed");
            rustok_telemetry::metrics::record_event_error(
                "tenant.locale.generation",
                "periodic_reconciliation",
            );
        }
    }
}

#[cfg(test)]
#[path = "tenant_locale_generation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tenant_locale_generation_durable_ahead_tests.rs"]
mod durable_ahead_tests;
