use std::any::Any;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rustok_cache::{
    BoundedCacheEventDedupe, BoundedCacheInvalidationGapTracker, BoundedInvalidationTrackerError,
    CacheBackendGenerationError, CacheInvalidationMessage, CacheInvalidationObservation,
    CacheInvalidationPayloadError, CacheService, DurableCacheInvalidationRecord,
    VersionedCacheInvalidation, bind_cache_backend_generation_aliases,
    cache_backend_generation_snapshot, observe_cache_backend_generation,
};
use rustok_core::events::{
    DomainEvent, EventConsumerRuntime, EventEnvelope, EventTransport, ReliabilityLevel,
};
use rustok_core::{Error, Result};
use rustok_events::{ContractEventEnvelope, ContractEventPayload};
use tokio::sync::broadcast;

use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::stop_handle::{StopHandle, await_worker_startup_or_stop, run_until_stopped};
use crate::services::tenant_cache_generation_status::{
    TenantCacheGenerationListenerSnapshot, TenantCacheGenerationListenerState,
};

/// Durable sequence namespace. Both physical tenant cache backends alias this generation state.
pub const TENANT_CACHE_BACKEND_PREFIX: &str = "rustok:tenant:v2";
/// Physical prefixes used by `middleware::tenant::TenantCacheInfrastructure`.
pub const TENANT_CACHE_DATA_BACKEND_PREFIX: &str = "tenant-cache:v2:data";
pub const TENANT_CACHE_NEGATIVE_BACKEND_PREFIX: &str = "tenant-cache:v2:negative";
pub const TENANT_CACHE_GENERATION_CHANNEL: &str = "tenant.cache.generation.v1";
/// Invalidation-record key for platform-wide module policy events (security
/// state, static distribution releases): there is no single tenant to
/// invalidate, so listeners drop every cached effective policy.
const PLATFORM_POLICY_INVALIDATION_KEY: &str = "platform";
const LISTENER_RESTART_DELAY: Duration = Duration::from_secs(1);
const GENERATION_RECONCILE_INTERVAL: Duration = Duration::from_secs(30);
const PREFLIGHT_GENERATION: u64 = 1;

pub(crate) fn bind_tenant_backend_generations() -> Result<()> {
    bind_cache_backend_generation_aliases(
        TENANT_CACHE_BACKEND_PREFIX,
        &[
            TENANT_CACHE_DATA_BACKEND_PREFIX,
            TENANT_CACHE_NEGATIVE_BACKEND_PREFIX,
        ],
    )
    .map_err(|error| Error::Cache(error.to_string()))?;
    Ok(())
}

pub(crate) fn observe_tenant_backend_generation(generation: u64) -> Result<()> {
    // A concurrent durable reconciliation may already have advanced the shared state beyond this
    // event. Treat that specific regression as a safe superseded no-op; all other failures remain
    // fail-closed. The physical prefixes alias this one canonical state.
    match observe_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX, generation) {
        Ok(_) => Ok(()),
        Err(CacheBackendGenerationError::GenerationRegressed { current, proposed })
            if current >= proposed =>
        {
            Ok(())
        }
        Err(error) => Err(Error::Cache(error.to_string())),
    }
}

#[derive(Clone)]
pub struct TenantCacheGenerationTransport {
    inner: Arc<dyn EventTransport>,
    cache: CacheService,
    successful_rotations: Arc<BoundedCacheEventDedupe>,
}

impl TenantCacheGenerationTransport {
    pub fn new(inner: Arc<dyn EventTransport>, cache: CacheService) -> Self {
        Self {
            inner,
            cache,
            successful_rotations: Arc::new(BoundedCacheEventDedupe::default()),
        }
    }

    fn validated_record(
        envelope: &EventEnvelope,
        tenant_id: Option<uuid::Uuid>,
        key: String,
        generation: u64,
        emitted_at_unix_ms: u64,
    ) -> Result<DurableCacheInvalidationRecord> {
        DurableCacheInvalidationRecord::new(
            envelope.id,
            tenant_id,
            TENANT_CACHE_GENERATION_CHANNEL,
            key,
            generation,
            emitted_at_unix_ms,
            envelope.event_type.clone(),
            envelope.trace_id.clone(),
        )
        .map_err(|error| Error::Cache(error.to_string()))
    }

    async fn publish_generation_if_needed(&self, envelope: &EventEnvelope) -> Result<()> {
        let (tenant_id, invalidation_key) = if platform_wide_policy_event(&envelope.event) {
            (None, PLATFORM_POLICY_INVALIDATION_KEY.to_string())
        } else if let Some(tenant_id) =
            tenant_cache_event_tenant_id(&envelope.event, Some(envelope.tenant_id))
        {
            (Some(tenant_id), tenant_id.to_string())
        } else {
            return Ok(());
        };

        // Hold the bounded per-event stripe across probe, validation, rotation, publication, and
        // commit. This closes the concurrent false-negative window without precommitting failed
        // work. All deterministic validation completes before the durable counter is advanced.
        let _event_guard = self.successful_rotations.serialize_event(envelope.id).await;
        if self.successful_rotations.is_duplicate(envelope.id) {
            tracing::debug!(
                event_id = %envelope.id,
                event_type = %envelope.event_type,
                "Tenant cache generation already rotated for event retry"
            );
            return Ok(());
        }

        let emitted_at_unix_ms =
            u64::try_from(envelope.timestamp.timestamp_millis()).map_err(|_| {
                Error::Validation(
                    "tenant cache invalidation timestamp precedes Unix epoch".to_string(),
                )
            })?;
        let _preflight = Self::validated_record(
            envelope,
            tenant_id,
            invalidation_key.clone(),
            PREFLIGHT_GENERATION,
            emitted_at_unix_ms,
        )?;

        let generation = self
            .cache
            .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
            .await
            .map_err(|error| Error::Cache(error.to_string()))?;
        let record = Self::validated_record(
            envelope,
            tenant_id,
            invalidation_key,
            generation.generation,
            emitted_at_unix_ms,
        )?;
        let outcome = self
            .cache
            .invalidations()
            .publish_durable(&record)
            .await
            .map_err(|error| Error::Cache(error.to_string()))?;

        if self.cache.redis_configuration_present() {
            if !outcome.redis_published {
                return Err(Error::Cache(
                    "tenant cache generation advanced but Redis invalidation publish failed"
                        .to_string(),
                ));
            }
        } else if outcome.local_subscribers == 0 {
            return Err(Error::Cache(
                "tenant cache generation advanced without a local invalidation subscriber"
                    .to_string(),
            ));
        }

        // INVARIANT: Dedupe observation after successful cache generation rotation.
        self.successful_rotations.observe(envelope.id);
        Ok(())
    }
}

#[async_trait]
impl EventTransport for TenantCacheGenerationTransport {
    async fn publish(&self, envelope: EventEnvelope) -> Result<()> {
        // Rotation happens before downstream delivery. Once rotation succeeds, retries preserve
        // the same event ID and skip only the already-completed invalidation work.
        self.publish_generation_if_needed(&envelope).await?;
        self.inner.publish(envelope).await
    }

    async fn publish_contract(&self, envelope: ContractEventEnvelope) -> Result<()> {
        if matches!(
            envelope
                .payload()
                .map_err(|error| Error::Validation(error.to_string()))?,
            ContractEventPayload::Root(_)
        ) {
            let root = envelope
                .into_root_envelope()
                .map_err(|error| Error::Validation(error.to_string()))?;
            return self.publish(root).await;
        }
        // This wrapper's root-specific work does not reinterpret bounded-family facts.
        self.inner.publish_contract(envelope).await
    }

    async fn acknowledge(&self, event_id: uuid::Uuid) -> Result<()> {
        self.inner.acknowledge(event_id).await
    }

    // Transactional appends are storage-only; generation rotation stays a
    // publish-time concern of the relay path above.
    fn supports_transactional_writes(&self) -> bool {
        self.inner.supports_transactional_writes()
    }

    async fn write_root_in_transaction(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        envelope: EventEnvelope,
    ) -> Result<()> {
        self.inner
            .write_root_in_transaction(transaction, envelope)
            .await
    }

    async fn write_contract_in_transaction(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        envelope: ContractEventEnvelope,
    ) -> Result<()> {
        self.inner
            .write_contract_in_transaction(transaction, envelope)
            .await
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        self.inner.reliability_level()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn tenant_cache_event_tenant_id(
    event: &DomainEvent,
    envelope_tenant_id: Option<uuid::Uuid>,
) -> Option<uuid::Uuid> {
    match event {
        DomainEvent::TenantCreated { tenant_id }
        | DomainEvent::TenantUpdated { tenant_id }
        | DomainEvent::TenantModuleToggled { tenant_id, .. }
        | DomainEvent::LocaleEnabled { tenant_id, .. }
        | DomainEvent::LocaleDisabled { tenant_id, .. }
        | DomainEvent::ModuleArtifactTenantEnabled { tenant_id, .. }
        | DomainEvent::ModuleArtifactTenantDisabled { tenant_id, .. }
        | DomainEvent::ModuleArtifactDataPurged { tenant_id, .. }
        | DomainEvent::ModuleArtifactSettingsPurged { tenant_id, .. }
        | DomainEvent::ModuleArtifactSettingsRestored { tenant_id, .. } => Some(*tenant_id),
        DomainEvent::ModuleArtifactActivated { .. }
        | DomainEvent::ModuleArtifactRolledBack { .. }
        | DomainEvent::ModuleTransitionFinalized { .. }
        | DomainEvent::ModuleArtifactAdmitted { .. }
        | DomainEvent::ModuleArtifactReverified { .. }
        | DomainEvent::ModuleArtifactUninstalled { .. }
        | DomainEvent::ModuleArtifactDeactivated { .. }
        | DomainEvent::ModuleTransitionFailedClosed { .. }
        | DomainEvent::ModuleEffectivePolicyRevisionChanged { .. }
        | DomainEvent::PlatformSettingsChanged { .. } => envelope_tenant_id,
        _ => None,
    }
}

/// Module policy events whose effect is platform-wide: a release security
/// state or a static-distribution release change alters enablement for every
/// tenant, so no per-tenant key can express the invalidation. Listeners react
/// to these by dropping all cached effective policies (the cache TTL remains
/// as the final fail-safe if an event is ever missed).
fn platform_wide_policy_event(event: &DomainEvent) -> bool {
    matches!(
        event,
        DomainEvent::ModuleArtifactSecurityStateChanged { .. }
            | DomainEvent::ModuleStaticDistributionReleaseAdmitted { .. }
            | DomainEvent::ModuleStaticDistributionReleaseActivated { .. }
            | DomainEvent::ModuleStaticDistributionReleaseRevoked { .. }
            | DomainEvent::ModuleStaticDistributionRolloutStatusChanged { .. }
            | DomainEvent::ModuleStaticDistributionRecoveryConverged { .. }
            | DomainEvent::ModuleStaticPromotionApproved { .. }
            | DomainEvent::ModuleArtifactNodeReconciliationStatusChanged { .. }
    )
}

/// Fence this process's effective-policy cache synchronously before an outbox
/// event is handed to downstream consumers. The durable listener still
/// invalidates every other process; this local step closes the window between
/// publishing the durable invalidation record and the local subscriber
/// consuming it.
pub(crate) fn invalidate_effective_policy_cache_for_event(
    ctx: &ServerRuntimeContext,
    envelope: &EventEnvelope,
) -> bool {
    let Some(policy_cache) = ctx.shared_get::<rustok_modules::ModuleEffectivePolicyCache>() else {
        return false;
    };

    if platform_wide_policy_event(&envelope.event) {
        policy_cache.clear();
        return true;
    }

    if let Some(tenant_id) =
        tenant_cache_event_tenant_id(&envelope.event, Some(envelope.tenant_id))
    {
        policy_cache.invalidate_tenant(tenant_id);
        return true;
    }

    false
}

#[derive(Clone)]
struct TenantCacheGenerationListener {
    cache: CacheService,
    effective_policy_cache: Option<rustok_modules::ModuleEffectivePolicyCache>,
    tracker: BoundedCacheInvalidationGapTracker,
    // Local bus, Redis subscriber and reconciler must not interleave
    // generation observation, policy invalidation and checkpoint commit.
    operation_gate: Arc<tokio::sync::Mutex<()>>,
}

impl TenantCacheGenerationListener {
    fn new(
        cache: CacheService,
        effective_policy_cache: Option<rustok_modules::ModuleEffectivePolicyCache>,
    ) -> Self {
        Self {
            cache,
            effective_policy_cache,
            tracker: BoundedCacheInvalidationGapTracker::default(),
            operation_gate: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    async fn recover_shared_generation(&self) -> Result<u64> {
        let _operation_guard = self.operation_gate.lock().await;
        self.recover_shared_generation_locked().await
    }

    // Caller owns operation_gate; the message handler uses this inner form
    // after observing a gap to avoid recursively acquiring an async mutex.
    async fn recover_shared_generation_locked(&self) -> Result<u64> {
        let value = if self.cache.redis_configuration_present() {
            if !self.cache.redis_client_initialized() {
                return Err(Error::Cache(
                    "Redis is configured but the tenant generation client is unavailable"
                        .to_string(),
                ));
            }
            self.cache
                .namespace_generations()
                .read(TENANT_CACHE_BACKEND_PREFIX)
                .await
                .map_err(|error| Error::Cache(error.to_string()))?
                .value()
        } else {
            let snapshot = cache_backend_generation_snapshot(TENANT_CACHE_BACKEND_PREFIX)
                .map_err(|error| Error::Cache(error.to_string()))?;
            if snapshot.trusted {
                snapshot.generation
            } else {
                observe_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX, 0)
                    .map_err(|error| Error::Cache(error.to_string()))?
                    .generation
            }
        };

        observe_tenant_backend_generation(value)?;
        // The durable generation covers changes to *any* tenant, including
        // events this process never received. Advancing its checkpoint without
        // dropping cached effective policies would leave other tenants' module
        // authority stale until TTL expiration. Apply the conservative full
        // invalidation before acknowledging recovery, including first boot.
        if self
            .tracker
            .last_generation(TENANT_CACHE_GENERATION_CHANNEL)
            != Some(value)
        {
            if let Some(policy_cache) = &self.effective_policy_cache {
                policy_cache.clear();
            }
            self.cache.clear_shared_backends();
        }
        self.tracker
            .acknowledge_recovery(TENANT_CACHE_GENERATION_CHANNEL, value)
            .map_err(|error| Error::Cache(error.to_string()))?;
        Ok(value)
    }

    fn acknowledge_applied_generation(&self, generation: u64) -> Result<()> {
        match self
            .tracker
            .acknowledge_applied(TENANT_CACHE_GENERATION_CHANNEL, generation)
        {
            Ok(_) => Ok(()),
            Err(BoundedInvalidationTrackerError::Payload(
                CacheInvalidationPayloadError::OffsetRegressed { current, proposed },
            )) if current >= proposed => Ok(()),
            Err(error) => Err(Error::Cache(error.to_string())),
        }
    }

    async fn handle_message(&self, message: CacheInvalidationMessage) -> Result<()> {
        let _operation_guard = self.operation_gate.lock().await;
        let started_at = Instant::now();
        let event = VersionedCacheInvalidation::from_message(&message)
            .map_err(|error| Error::Cache(error.to_string()))?;
        if event.channel != TENANT_CACHE_GENERATION_CHANNEL {
            return Err(Error::Validation(format!(
                "unexpected tenant cache invalidation channel {}",
                event.channel
            )));
        }

        let acknowledge_generation = match self.tracker.observe(&event) {
            CacheInvalidationObservation::InOrder { generation } => {
                observe_tenant_backend_generation(generation)?;
                // Do not acknowledge the generation until its effects are
                // visible; otherwise another consumer can skip an uncompleted
                // cache invalidation after observing the advanced checkpoint.
                Some(generation)
            }
            CacheInvalidationObservation::Duplicate { .. }
            | CacheInvalidationObservation::Stale { .. } => None,
            CacheInvalidationObservation::UnverifiedFirst { .. }
            | CacheInvalidationObservation::Gap { .. } => {
                let recovered = self.recover_shared_generation_locked().await?;
                if recovered < event.generation {
                    return Err(Error::Cache(format!(
                        "shared tenant cache generation {recovered} trails received {}",
                        event.generation
                    )));
                }
                None
            }
        };

        if let Some(policy_cache) = &self.effective_policy_cache {
            if event.key == PLATFORM_POLICY_INVALIDATION_KEY || event.key == "*" {
                // Platform-wide policy events and manual global invalidation
                // both affect every tenant's effective availability.
                policy_cache.clear();
            } else if let Ok(tenant_id) = uuid::Uuid::parse_str(&event.key)
                && !tenant_id.is_nil()
            {
                policy_cache.invalidate_tenant(tenant_id);
            } else {
                // The public invalidation envelope can carry an arbitrary
                // bounded key. Unknown tenant coordinates must not acknowledge
                // an event while leaving the unrelated policy cache intact.
                policy_cache.clear();
                tracing::warn!(
                    error_code = "tenant_cache_generation_invalid_policy_key",
                    "Invalid tenant cache generation key; invalidated all effective policies"
                );
            }
        }
        self.cache.clear_shared_backends();
        if let Some(generation) = acknowledge_generation {
            self.acknowledge_applied_generation(generation)?;
        }

        rustok_telemetry::metrics::record_event_dispatch_latency_ms(
            "tenant_cache_generation",
            "tenant.cache.generation",
            started_at.elapsed().as_secs_f64() * 1000.0,
        );
        Ok(())
    }
}

#[derive(Clone)]
pub struct TenantCacheGenerationListenerHandle {
    state: Arc<TenantCacheGenerationListenerState>,
    required_task_count: usize,
    tasks: Arc<std::sync::Mutex<Option<Vec<tokio::task::JoinHandle<()>>>>>,
}

impl TenantCacheGenerationListenerHandle {
    fn is_running(&self) -> bool {
        self.tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|tasks| {
                tasks.len() >= self.required_task_count
                    && tasks.iter().all(|task| !task.is_finished())
            })
    }

    pub(crate) fn take_tasks(&self) -> Vec<tokio::task::JoinHandle<()>> {
        self.tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .unwrap_or_default()
    }
}

#[derive(Clone, Default)]
struct TenantCacheGenerationListenerStartLock(Arc<tokio::sync::Mutex<()>>);

pub async fn tenant_cache_generation_listener_snapshot(
    ctx: &ServerRuntimeContext,
) -> TenantCacheGenerationListenerSnapshot {
    match ctx.shared_get::<TenantCacheGenerationListenerHandle>() {
        Some(handle) => {
            if !handle.is_running() {
                handle
                    .state
                    .mark_runtime_stopped("tenant cache generation background task stopped")
                    .await;
            }
            handle.state.snapshot().await
        }
        None => TenantCacheGenerationListenerSnapshot::disabled(
            "tenant cache generation listener is not initialized",
        ),
    }
}

pub async fn start_tenant_cache_generation_listener(
    ctx: &ServerRuntimeContext,
    cache: CacheService,
) -> Result<()> {
    bind_tenant_backend_generations()?;
    ctx.shared_insert_if_absent(TenantCacheGenerationListenerStartLock::default());
    let start_lock = ctx
        .shared_get::<TenantCacheGenerationListenerStartLock>()
        .ok_or_else(|| Error::Cache("tenant listener start lock is unavailable".to_string()))?;
    let stop = StopHandle::ensure(ctx);
    let Some(_start_guard) = await_worker_startup_or_stop(&stop, start_lock.0.lock()).await
    else {
        return Ok(());
    };
    if let Some(existing) = ctx.shared_get::<TenantCacheGenerationListenerHandle>() {
        if existing.is_running() {
            return Ok(());
        }
        tracing::warn!("Tenant cache generation listener stopped; replacing runtime");
        if let Some(stale) = ctx.shared_take::<TenantCacheGenerationListenerHandle>() {
            let tasks = stale.take_tasks();
            for task in &tasks {
                task.abort();
            }
            let Some(()) = await_worker_startup_or_stop(&stop, async {
                for task in tasks {
                    if let Err(join_err) = task.await {
                        if !join_err.is_cancelled() {
                            tracing::warn!(
                                failure_code = "tenant_cache_generation_listener_join_failed",
                                "Aborted stale tenant cache generation listener task failed unexpectedly"
                            );
                        }
                    }
                }
            }).await else {
                return Ok(());
            };
        }
    }

    let redis_required = cache.redis_configuration_present();
    let state = TenantCacheGenerationListenerState::new(redis_required);
    let listener =
        TenantCacheGenerationListener::new(cache.clone(), Some(ctx.effective_policy_cache()));
    let Some(()) = await_worker_startup_or_stop(&stop, async {
        match listener.recover_shared_generation().await {
            Ok(_) if !redis_required => state.mark_local_healthy().await,
            Ok(_) => state.mark_reconciliation_healthy().await,
            Err(_error) if redis_required => {
                state
                    .mark_reconciliation_degraded("tenant cache generation startup recovery failed")
                    .await;
                tracing::warn!(
                    error_code = "tenant_cache_generation_startup_recovery_failed",
                    "Tenant cache generation startup recovery failed; isolated boot namespace remains active"
                );
                rustok_telemetry::metrics::record_event_error(
                    "tenant.cache.generation",
                    "startup_recovery",
                );
            }
            Err(_error) => {
                state
                    .mark_local_degraded("local tenant cache generation startup recovery failed")
                    .await;
                tracing::warn!(
                    error_code = "tenant_cache_generation_local_startup_recovery_failed",
                    "Local tenant cache generation startup recovery failed"
                );
                rustok_telemetry::metrics::record_event_error(
                    "tenant.cache.generation",
                    "startup_recovery",
                );
            }
        }
    }).await else {
        return Ok(());
    };

    let mut tasks = Vec::new();
    // A configured-but-uninitialized Redis client cannot spawn the subscriber or
    // reconciler; readiness remains degraded, but the local listener is still a
    // valid live supervisor and must not be restarted on every repeated init.
    let required_task_count = if cache.redis_client_initialized() {
        3
    } else {
        1
    };
    let mut local = cache
        .invalidations()
        .subscribe_local_channel(TENANT_CACHE_GENERATION_CHANNEL);
    let local_listener = listener.clone();
    let local_state = Arc::clone(&state);
    let local_task = tokio::spawn(run_until_stopped(stop.subscribe(), async move {
        let runtime = EventConsumerRuntime::new("tenant_cache_generation_local");
        runtime.restarted("startup");
        loop {
            match local.recv().await {
                Ok(message) => match local_listener.handle_message(message).await {
                    Ok(()) => {
                        local_state.mark_local_healthy().await;
                    }
                    Err(_error) => {
                        local_state
                            .mark_local_degraded("local tenant cache generation apply failed")
                            .await;
                        tracing::error!(
                            error_code = "tenant_cache_generation_local_apply_failed",
                            "Local tenant cache generation apply failed"
                        );
                        rustok_telemetry::metrics::record_event_error(
                            "tenant.cache.generation",
                            "local_apply",
                        );
                    }
                },
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    runtime.lagged(skipped);
                    match local_listener.recover_shared_generation().await {
                        Ok(_) => local_state.mark_local_healthy().await,
                        Err(_error) => {
                            local_state
                                .mark_local_degraded("tenant cache generation recovery after local lag failed")
                                .await;
                            tracing::error!(
                                error_code = "tenant_cache_generation_local_lag_recovery_failed",
                                "Tenant cache generation recovery after local lag failed"
                            );
                            rustok_telemetry::metrics::record_event_error(
                                "tenant.cache.generation",
                                "local_lag_recovery",
                            );
                        }
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    runtime.closed();
                    local_state
                        .mark_local_degraded("local tenant generation subscription closed")
                        .await;
                    break;
                }
            }
        }
    }));
    tasks.push(local_task);

    if cache.redis_client_initialized() {
        let redis_listener = listener.clone();
        let invalidations = cache.invalidations();
        let redis_state = Arc::clone(&state);
        let redis_task = tokio::spawn(run_until_stopped(stop.subscribe(), async move {
            let runtime = EventConsumerRuntime::new("tenant_cache_generation_redis");
            let mut reason = "startup";
            loop {
                runtime.restarted(reason);
                redis_state.mark_subscriber_starting();
                let ready_listener = redis_listener.clone();
                let ready_state = Arc::clone(&redis_state);
                let handler_listener = redis_listener.clone();
                let handler_state = Arc::clone(&redis_state);
                let _result = invalidations
                    .consume_subscription_with_ready(
                        TENANT_CACHE_GENERATION_CHANNEL,
                        move || {
                            let ready_listener = ready_listener.clone();
                            let ready_state = Arc::clone(&ready_state);
                            async move {
                                match ready_listener.recover_shared_generation().await {
                                    Ok(_) => {
                                        ready_state.mark_subscriber_ready_after_recovery().await;
                                    }
                                    Err(_error) => {
                                        ready_state
                                            .mark_subscriber_degraded(
                                                "tenant cache generation post-subscribe recovery failed",
                                            )
                                            .await;
                                        tracing::warn!(
                                            error_code = "tenant_cache_generation_post_subscribe_recovery_failed",
                                            "Tenant cache generation post-subscribe recovery failed"
                                        );
                                        rustok_telemetry::metrics::record_event_error(
                                            "tenant.cache.generation",
                                            "redis_ready_recovery",
                                        );
                                    }
                                }
                            }
                        },
                        move |message| {
                            let handler_listener = handler_listener.clone();
                            let handler_state = Arc::clone(&handler_state);
                            async move {
                                match handler_listener.handle_message(message).await {
                                    Ok(()) => {
                                        handler_state.mark_subscriber_activity_healthy().await;
                                    }
                                    Err(_error) => {
                                        handler_state
                                            .mark_subscriber_degraded("Redis tenant cache generation apply failed")
                                            .await;
                                        tracing::error!(
                                            error_code = "tenant_cache_generation_redis_apply_failed",
                                            "Redis tenant cache generation apply failed"
                                        );
                                        rustok_telemetry::metrics::record_event_error(
                                            "tenant.cache.generation",
                                            "redis_apply",
                                        );
                                    }
                                }
                            }
                        },
                    )
                    .await;
                redis_state
                    .mark_subscriber_degraded("Redis tenant generation subscriber stopped")
                    .await;
                tracing::warn!(
                    failure_code = "tenant_cache_generation_subscriber_stopped",
                    "Tenant cache generation Redis subscriber stopped"
                );
                reason = "reconnect";
                tokio::time::sleep(LISTENER_RESTART_DELAY).await;
            }
        }));
        tasks.push(redis_task);

        let reconcile_listener = listener.clone();
        let reconcile_state = Arc::clone(&state);
        let reconcile_task = tokio::spawn(run_until_stopped(stop.subscribe(), async move {
            let start = tokio::time::Instant::now() + GENERATION_RECONCILE_INTERVAL;
            let mut interval = tokio::time::interval_at(start, GENERATION_RECONCILE_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                match reconcile_listener.recover_shared_generation().await {
                    Ok(_) => reconcile_state.mark_reconciliation_healthy().await,
                    Err(_error) => {
                        reconcile_state
                            .mark_reconciliation_degraded(
                                "periodic tenant cache generation reconciliation failed",
                            )
                            .await;
                        tracing::warn!(
                            error_code = "tenant_cache_generation_periodic_reconciliation_failed",
                            "Periodic tenant cache generation reconciliation failed"
                        );
                        rustok_telemetry::metrics::record_event_error(
                            "tenant.cache.generation",
                            "periodic_reconciliation",
                        );
                    }
                }
            }
        }));
        tasks.push(reconcile_task);
    }

    ctx.shared_insert(TenantCacheGenerationListenerHandle {
        state,
        required_task_count,
        tasks: Arc::new(std::sync::Mutex::new(Some(tasks))),
    });
    Ok(())
}

pub(crate) async fn take_listener_tasks_for_shutdown(
    ctx: &ServerRuntimeContext,
) -> Vec<tokio::task::JoinHandle<()>> {
    let start_lock = ctx.shared_get_or_insert_with(TenantCacheGenerationListenerStartLock::default);
    let _start_guard = match tokio::time::timeout(Duration::from_secs(5), start_lock.0.lock()).await
    {
        Ok(guard) => guard,
        Err(_) => {
            tracing::warn!("Timed out waiting for tenant cache generation startup during shutdown");
            return Vec::new();
        }
    };
    ctx.shared_take::<TenantCacheGenerationListenerHandle>()
        .map(|listener| listener.take_tasks())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_events::EventEnvelope;
    use std::sync::OnceLock;
    use uuid::Uuid;

    fn generation_test_lock() -> &'static tokio::sync::Mutex<()> {
        static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    fn tenant_event(event: DomainEvent) -> EventEnvelope {
        let tenant_id =
            tenant_cache_event_tenant_id(&event, None).unwrap_or_else(|| Uuid::from_u128(42));
        EventEnvelope::new(tenant_id, None, event)
    }

    #[test]
    fn tenant_mutations_rotate_the_tenant_cache_namespace() {
        let tenant_id = Uuid::from_u128(42);
        for event in [
            DomainEvent::TenantCreated { tenant_id },
            DomainEvent::TenantUpdated { tenant_id },
            DomainEvent::TenantModuleToggled {
                tenant_id,
                module_slug: "blog".to_string(),
                enabled: true,
            },
            DomainEvent::LocaleEnabled {
                tenant_id,
                locale: "en".to_string(),
            },
            DomainEvent::LocaleDisabled {
                tenant_id,
                locale: "en".to_string(),
            },
            DomainEvent::ModuleArtifactTenantEnabled {
                installation_id: Uuid::new_v4(),
                tenant_id,
                revision: 1,
            },
            DomainEvent::ModuleArtifactTenantDisabled {
                installation_id: Uuid::new_v4(),
                tenant_id,
                revision: 2,
            },
            DomainEvent::ModuleArtifactDataPurged {
                tenant_id,
                module_slug: "blog".to_string(),
                data_contract_revision: 1,
                namespace_revision: 1,
                purged_records: 10,
            },
            DomainEvent::ModuleArtifactSettingsPurged {
                recovery_point_id: Uuid::new_v4(),
                tenant_id,
                installation_id: Uuid::new_v4(),
                settings_instance_id: Uuid::new_v4(),
                tombstone_revision: 1,
            },
            DomainEvent::ModuleArtifactSettingsRestored {
                recovery_point_id: Uuid::new_v4(),
                tenant_id,
                target_installation_id: Some(Uuid::new_v4()),
                settings_instance_id: Uuid::new_v4(),
            },
            DomainEvent::ModuleArtifactActivated {
                installation_id: Uuid::new_v4(),
                predecessor_installation_id: None,
                revision: 1,
            },
            DomainEvent::ModuleArtifactRolledBack {
                installation_id: Uuid::new_v4(),
                target_installation_id: Uuid::new_v4(),
            },
            DomainEvent::ModuleTransitionFinalized {
                operation_id: Uuid::new_v4(),
                module_slug: "blog".to_string(),
                revision: 1,
                released_holds: 0,
            },
            DomainEvent::PlatformSettingsChanged {
                category: "general".to_string(),
                changed_by: Uuid::new_v4(),
            },
        ] {
            assert_eq!(
                tenant_cache_event_tenant_id(&event, Some(tenant_id)),
                Some(tenant_id)
            );
        }
    }

    #[test]
    fn module_policy_events_are_classified_for_cache_invalidation() {
        let tenant_id = Uuid::from_u128(42);
        // Envelope-tenant lifecycle events invalidate that tenant only.
        for event in [
            DomainEvent::ModuleArtifactAdmitted {
                installation_id: Uuid::new_v4(),
                artifact_digest: "sha256:abc".to_string(),
                media_type: "application/wasm".to_string(),
                size_bytes: 1,
            },
            DomainEvent::ModuleArtifactUninstalled {
                installation_id: Uuid::new_v4(),
                revision: 1,
            },
            DomainEvent::ModuleEffectivePolicyRevisionChanged {
                consumer_key: "effective-policy".to_string(),
                previous_revision: None,
                next_revision: "rev-2".to_string(),
            },
        ] {
            assert_eq!(
                tenant_cache_event_tenant_id(&event, Some(tenant_id)),
                Some(tenant_id)
            );
            assert!(!platform_wide_policy_event(&event));
        }

        // Release-scoped security changes are platform-wide: every tenant's
        // cached policy must be dropped, not just the envelope tenant's.
        let quarantined = DomainEvent::ModuleArtifactSecurityStateChanged {
            module_slug: "blog".to_string(),
            module_version: "1.0.0".to_string(),
            payload_digest: "sha256:abc".to_string(),
            security_revision: 2,
            status: "quarantined".to_string(),
            policy_revision: "rev-2".to_string(),
            reason_code: "cve".to_string(),
        };
        assert!(platform_wide_policy_event(&quarantined));
    }

    #[test]
    fn physical_backend_prefixes_match_tenant_middleware_contract() {
        assert_eq!(TENANT_CACHE_DATA_BACKEND_PREFIX, "tenant-cache:v2:data");
        assert_eq!(
            TENANT_CACHE_NEGATIVE_BACKEND_PREFIX,
            "tenant-cache:v2:negative"
        );
    }

    #[tokio::test]
    async fn transport_rotates_both_physical_backends_before_local_delivery() {
        let _guard = generation_test_lock().lock().await;
        bind_tenant_backend_generations().unwrap();
        let cache = CacheService::from_url(None);
        let data_backend = cache
            .backend_weighted(
                TENANT_CACHE_DATA_BACKEND_PREFIX,
                Duration::from_secs(60),
                4096,
            )
            .await;
        let negative_backend = cache
            .backend_weighted(
                TENANT_CACHE_NEGATIVE_BACKEND_PREFIX,
                Duration::from_secs(60),
                4096,
            )
            .await;
        data_backend
            .set("key".to_string(), b"data".to_vec())
            .await
            .unwrap();
        negative_backend
            .set("key".to_string(), b"negative".to_vec())
            .await
            .unwrap();

        let primary = rustok_core::events::MemoryTransport::with_capacity(8);
        let mut receiver = primary.subscribe();
        let transport = TenantCacheGenerationTransport::new(Arc::new(primary), cache.clone());
        let mut invalidations = cache
            .invalidations()
            .subscribe_local_channel(TENANT_CACHE_GENERATION_CHANNEL);
        let before = cache_backend_generation_snapshot(TENANT_CACHE_BACKEND_PREFIX)
            .unwrap()
            .generation;
        let envelope = tenant_event(DomainEvent::TenantUpdated {
            tenant_id: Uuid::from_u128(42),
        });

        transport.publish(envelope.clone()).await.unwrap();
        let message = invalidations.recv().await.unwrap();
        let event = VersionedCacheInvalidation::from_message(&message).unwrap();
        assert_eq!(event.generation, before + 1);
        assert_eq!(receiver.recv().await.unwrap().id, envelope.id);
        assert_eq!(data_backend.get("key").await.unwrap(), None);
        assert_eq!(negative_backend.get("key").await.unwrap(), None);
        assert_eq!(
            cache_backend_generation_snapshot(TENANT_CACHE_DATA_BACKEND_PREFIX).unwrap(),
            cache_backend_generation_snapshot(TENANT_CACHE_NEGATIVE_BACKEND_PREFIX).unwrap()
        );
    }

    #[tokio::test]
    async fn malformed_envelope_does_not_advance_generation() {
        let _guard = generation_test_lock().lock().await;
        bind_tenant_backend_generations().unwrap();
        let cache = CacheService::from_url(None);
        let transport = TenantCacheGenerationTransport::new(
            Arc::new(rustok_core::events::MemoryTransport::with_capacity(8)),
            cache,
        );
        let before = cache_backend_generation_snapshot(TENANT_CACHE_BACKEND_PREFIX)
            .unwrap()
            .generation;
        let mut envelope = tenant_event(DomainEvent::TenantUpdated {
            tenant_id: Uuid::from_u128(44),
        });
        envelope.timestamp = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(-1).unwrap();

        assert!(
            transport
                .publish_generation_if_needed(&envelope)
                .await
                .is_err()
        );
        assert_eq!(
            cache_backend_generation_snapshot(TENANT_CACHE_BACKEND_PREFIX)
                .unwrap()
                .generation,
            before
        );
    }

    #[tokio::test]
    async fn retry_delivers_downstream_without_rotating_generation_twice() {
        let _guard = generation_test_lock().lock().await;
        bind_tenant_backend_generations().unwrap();
        let cache = CacheService::from_url(None);
        let primary = rustok_core::events::MemoryTransport::with_capacity(8);
        let mut receiver = primary.subscribe();
        let transport = TenantCacheGenerationTransport::new(Arc::new(primary), cache.clone());
        let mut invalidations = cache
            .invalidations()
            .subscribe_local_channel(TENANT_CACHE_GENERATION_CHANNEL);
        let envelope = tenant_event(DomainEvent::TenantUpdated {
            tenant_id: Uuid::from_u128(43),
        });

        transport.publish(envelope.clone()).await.unwrap();
        let first_message = invalidations.recv().await.unwrap();
        let first_event = VersionedCacheInvalidation::from_message(&first_message).unwrap();
        assert_eq!(receiver.recv().await.unwrap().id, envelope.id);

        transport.publish(envelope.clone()).await.unwrap();
        assert_eq!(receiver.recv().await.unwrap().id, envelope.id);
        assert_eq!(
            cache_backend_generation_snapshot(TENANT_CACHE_BACKEND_PREFIX)
                .unwrap()
                .generation,
            first_event.generation
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(10), invalidations.recv())
                .await
                .is_err()
        );
    }

    #[test]
    fn superseded_apply_is_a_safe_noop() {
        let _guard = generation_test_lock().blocking_lock();
        bind_tenant_backend_generations().unwrap();
        let current = cache_backend_generation_snapshot(TENANT_CACHE_BACKEND_PREFIX)
            .unwrap()
            .generation
            .saturating_add(2);
        observe_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX, current).unwrap();
        assert!(observe_tenant_backend_generation(current - 1).is_ok());
        assert_eq!(
            cache_backend_generation_snapshot(TENANT_CACHE_BACKEND_PREFIX)
                .unwrap()
                .generation,
            current
        );

        let listener = TenantCacheGenerationListener::new(CacheService::from_url(None), None);
        listener
            .tracker
            .seed(TENANT_CACHE_GENERATION_CHANNEL, current)
            .unwrap();
        assert!(listener.acknowledge_applied_generation(current - 1).is_ok());
        assert_eq!(
            listener
                .tracker
                .last_generation(TENANT_CACHE_GENERATION_CHANNEL),
            Some(current)
        );
    }

    #[tokio::test]
    async fn listener_recovers_gaps_into_both_physical_backends() {
        let _guard = generation_test_lock().lock().await;
        bind_tenant_backend_generations().unwrap();
        let cache = CacheService::from_url(None);
        let listener = TenantCacheGenerationListener::new(cache.clone(), None);
        let base = cache_backend_generation_snapshot(TENANT_CACHE_BACKEND_PREFIX)
            .unwrap()
            .generation;
        listener
            .tracker
            .seed(TENANT_CACHE_GENERATION_CHANNEL, base)
            .unwrap();
        let _first = cache
            .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
            .await
            .unwrap();
        let second = cache
            .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
            .await
            .unwrap();
        let event = VersionedCacheInvalidation::new(
            TENANT_CACHE_GENERATION_CHANNEL,
            Uuid::from_u128(42).to_string(),
            second.generation,
            1_000,
        )
        .unwrap();

        listener
            .handle_message(event.to_message().unwrap())
            .await
            .unwrap();
        for prefix in [
            TENANT_CACHE_BACKEND_PREFIX,
            TENANT_CACHE_DATA_BACKEND_PREFIX,
            TENANT_CACHE_NEGATIVE_BACKEND_PREFIX,
        ] {
            assert_eq!(
                cache_backend_generation_snapshot(prefix)
                    .unwrap()
                    .generation,
                second.generation
            );
        }
    }

    fn cached_policy_fixture() -> rustok_modules::ModuleEffectivePolicy {
        // An empty owner policy is a valid, revision-bound snapshot. Using the
        // public serde contract avoids reaching into module-private writers.
        serde_json::from_value(serde_json::json!({
            "policy_revision": format!("sha256:{}", "a".repeat(64)),
            "enabled_modules": [],
            "decisions": {}
        }))
        .expect("valid empty effective policy fixture")
    }

    #[tokio::test]
    async fn missed_generation_recovery_clears_all_tenant_policies() {
        let _guard = generation_test_lock().lock().await;
        bind_tenant_backend_generations().unwrap();
        let cache = CacheService::from_url(None);
        let policies = rustok_modules::ModuleEffectivePolicyCache::new();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        policies.insert(tenant_a, cached_policy_fixture()).unwrap();
        policies.insert(tenant_b, cached_policy_fixture()).unwrap();

        let listener = TenantCacheGenerationListener::new(cache.clone(), Some(policies.clone()));
        let base = cache_backend_generation_snapshot(TENANT_CACHE_BACKEND_PREFIX)
            .unwrap()
            .generation;
        listener
            .tracker
            .seed(TENANT_CACHE_GENERATION_CHANNEL, base)
            .unwrap();

        // Two durable rotations were not delivered through the subscriber.
        cache
            .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
            .await
            .unwrap();
        let second = cache
            .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
            .await
            .unwrap();
        assert_eq!(policies.len(), 2, "missed Pub/Sub does not itself clear policy entries");

        listener.recover_shared_generation().await.unwrap();
        assert!(
            policies.is_empty(),
            "durable reconciliation must invalidate all tenants, not merely a received key"
        );
        assert_eq!(
            listener.tracker.last_generation(TENANT_CACHE_GENERATION_CHANNEL),
            Some(second.generation)
        );

        // A healthy, unchanged periodic observation must not evict fresh
        // entries on every reconciliation interval.
        policies.insert(tenant_a, cached_policy_fixture()).unwrap();
        listener.recover_shared_generation().await.unwrap();
        assert_eq!(policies.len(), 1);

        // A later gap delivered for tenant A must evict unrelated tenant B
        // before publishing the recovered generation checkpoint.
        policies.insert(tenant_b, cached_policy_fixture()).unwrap();
        cache
            .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
            .await
            .unwrap();
        let fourth = cache
            .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
            .await
            .unwrap();
        let message = VersionedCacheInvalidation::new(
            TENANT_CACHE_GENERATION_CHANNEL,
            tenant_a.to_string(),
            fourth.generation,
            1_000,
        )
        .unwrap()
        .to_message()
        .unwrap();
        listener.handle_message(message).await.unwrap();
        assert!(policies.is_empty(), "gap recovery cannot retain tenant B's stale policy");
        assert_eq!(
            listener.tracker.last_generation(TENANT_CACHE_GENERATION_CHANNEL),
            Some(fourth.generation)
        );
    }

    #[tokio::test]
    async fn invalid_policy_key_clears_all_before_acknowledging_in_order_event() {
        let _guard = generation_test_lock().lock().await;
        bind_tenant_backend_generations().unwrap();
        let cache = CacheService::from_url(None);
        let policies = rustok_modules::ModuleEffectivePolicyCache::new();
        let tenant = Uuid::new_v4();
        policies.insert(tenant, cached_policy_fixture()).unwrap();
        let listener = TenantCacheGenerationListener::new(cache.clone(), Some(policies.clone()));
        let base = cache_backend_generation_snapshot(TENANT_CACHE_BACKEND_PREFIX)
            .unwrap()
            .generation;
        listener
            .tracker
            .seed(TENANT_CACHE_GENERATION_CHANNEL, base)
            .unwrap();
        let next = cache
            .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
            .await
            .unwrap();
        let message = VersionedCacheInvalidation::new(
            TENANT_CACHE_GENERATION_CHANNEL,
            "malformed-tenant-id",
            next.generation,
            1_000,
        )
        .unwrap()
        .to_message()
        .unwrap();
        listener.handle_message(message).await.unwrap();
        assert!(
            policies.is_empty(),
            "invalid bounded key cannot silently skip tenant policy invalidation"
        );
        assert_eq!(
            listener.tracker.last_generation(TENANT_CACHE_GENERATION_CHANNEL),
            Some(next.generation)
        );

        // The canonical manual global invalidation key is a valid wildcard,
        // not a malformed tenant identifier. It must still clear every policy.
        policies.insert(tenant, cached_policy_fixture()).unwrap();
        let wildcard_generation = cache
            .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
            .await
            .unwrap();
        let wildcard_message = VersionedCacheInvalidation::new(
            TENANT_CACHE_GENERATION_CHANNEL,
            "*",
            wildcard_generation.generation,
            1_000,
        )
        .unwrap()
        .to_message()
        .unwrap();
        listener.handle_message(wildcard_message).await.unwrap();
        assert!(policies.is_empty(), "manual global invalidation clears policies");

        // Nil UUIDs are not tenant identifiers. They must conservatively
        // clear, rather than selectively evicting an impossible tenant.
        policies.insert(tenant, cached_policy_fixture()).unwrap();
        let nil_generation = cache
            .bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)
            .await
            .unwrap();
        let nil_message = VersionedCacheInvalidation::new(
            TENANT_CACHE_GENERATION_CHANNEL,
            Uuid::nil().to_string(),
            nil_generation.generation,
            1_000,
        )
        .unwrap()
        .to_message()
        .unwrap();
        listener.handle_message(nil_message).await.unwrap();
        assert!(policies.is_empty(), "nil tenant key cannot skip invalidation");
    }

    #[test]
    fn cloned_generation_listeners_share_one_transition_lock() {
        let listener = TenantCacheGenerationListener::new(CacheService::from_url(None), None);
        let copy = listener.clone();
        let guard = listener
            .operation_gate
            .try_lock()
            .expect("first transition lock is free");
        assert!(
            copy.operation_gate.try_lock().is_err(),
            "local, Redis and reconciliation owners must share one gate"
        );
        drop(guard);
        assert!(copy.operation_gate.try_lock().is_ok());
    }

    #[tokio::test]
    async fn redis_listener_is_not_running_until_all_required_tasks_exist() {
        let task = tokio::spawn(std::future::pending::<()>());
        let handle = TenantCacheGenerationListenerHandle {
            state: TenantCacheGenerationListenerState::new(true),
            required_task_count: 3,
            tasks: Arc::new(std::sync::Mutex::new(Some(vec![task]))),
        };
        assert!(!handle.is_running());

        for task in handle.take_tasks() {
            task.abort();
            let _ = task.await;
        }
    }

    #[tokio::test]
    async fn configured_redis_without_client_tracks_only_the_started_local_task() {
        let task = tokio::spawn(std::future::pending::<()>());
        let handle = TenantCacheGenerationListenerHandle {
            state: TenantCacheGenerationListenerState::new(true),
            required_task_count: 1,
            tasks: Arc::new(std::sync::Mutex::new(Some(vec![task]))),
        };
        assert!(handle.is_running());

        for task in handle.take_tasks() {
            task.abort();
            let _ = task.await;
        }
    }

    #[tokio::test]
    async fn listener_handle_reports_finished_background_tasks() {
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = finished_rx.await;
        });
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, crate::common::settings::RustokSettings::default());
        let state = TenantCacheGenerationListenerState::new(false);
        state.mark_local_healthy().await;
        let handle = TenantCacheGenerationListenerHandle {
            state,
            required_task_count: 1,
            tasks: Arc::new(std::sync::Mutex::new(Some(vec![task]))),
        };
        ctx.shared_insert(handle.clone());

        assert!(handle.is_running());
        finished_tx.send(()).expect("signal task completion");
        tokio::time::timeout(Duration::from_secs(1), async {
            while handle.is_running() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("listener handle must observe the finished task");
        let snapshot = tenant_cache_generation_listener_snapshot(&ctx).await;
        assert_eq!(
            snapshot.status,
            crate::services::tenant_cache_generation_status::TenantCacheGenerationListenerStatus::Degraded
        );
        assert!(!snapshot.local_ready);
    }

    #[tokio::test]
    async fn shutdown_aborts_and_joins_tenant_generation_listener_tasks() {
        struct NotifyDrop(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for NotifyDrop {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, crate::common::settings::RustokSettings::default());
        let mut stop = crate::services::app_lifecycle::StopHandle::ensure(&ctx).subscribe();
        let (stopped_tx, stopped_rx) = tokio::sync::oneshot::channel();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _notify_drop = NotifyDrop(Some(stopped_tx));
            let _ = started_tx.send(());
            let _ = stop.changed().await;
        });
        started_rx.await.expect("listener task must start");
        ctx.shared_insert(TenantCacheGenerationListenerHandle {
            state: TenantCacheGenerationListenerState::new(false),
            required_task_count: 1,
            tasks: Arc::new(std::sync::Mutex::new(Some(vec![task]))),
        });

        crate::services::app_lifecycle::shutdown_runtime_workers(&ctx).await;

        stopped_rx
            .await
            .expect("shutdown must abort and join listener tasks");
        assert!(!ctx.shared_contains::<TenantCacheGenerationListenerHandle>());
    }

    #[tokio::test]
    async fn missing_listener_handle_reports_disabled_snapshot() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, crate::common::settings::RustokSettings::default());
        let snapshot = tenant_cache_generation_listener_snapshot(&ctx).await;
        assert_eq!(
            snapshot.status,
            crate::services::tenant_cache_generation_status::TenantCacheGenerationListenerStatus::Disabled
        );
    }

    #[tokio::test]
    async fn invalid_redis_configuration_does_not_seed_trusted_tenant_generation() {
        let _guard = generation_test_lock().lock().await;
        bind_tenant_backend_generations().unwrap();
        let cache = CacheService::from_url(Some("://invalid-redis-url"));
        let listener = TenantCacheGenerationListener::new(cache, None);
        assert!(listener.recover_shared_generation().await.is_err());
    }
}
