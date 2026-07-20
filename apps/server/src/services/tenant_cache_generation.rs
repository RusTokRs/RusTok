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
use tokio::sync::broadcast;

use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::tenant_cache_generation_status::{
    TenantCacheGenerationListenerSnapshot, TenantCacheGenerationListenerState,
};

/// Durable sequence namespace. Both physical tenant cache backends alias this generation state.
pub const TENANT_CACHE_BACKEND_PREFIX: &str = "rustok:tenant:v2";
/// Physical prefixes used by `middleware::tenant::TenantCacheInfrastructure`.
pub const TENANT_CACHE_DATA_BACKEND_PREFIX: &str = "tenant-cache:v2:data";
pub const TENANT_CACHE_NEGATIVE_BACKEND_PREFIX: &str = "tenant-cache:v2:negative";
pub const TENANT_CACHE_GENERATION_CHANNEL: &str = "tenant.cache.generation.v1";
const LISTENER_RESTART_DELAY: Duration = Duration::from_secs(1);
const GENERATION_RECONCILE_INTERVAL: Duration = Duration::from_secs(30);
const PREFLIGHT_GENERATION: u64 = 1;

fn bind_tenant_backend_generations() -> Result<()> {
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

fn observe_tenant_backend_generation(generation: u64) -> Result<()> {
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
        tenant_id: uuid::Uuid,
        generation: u64,
        emitted_at_unix_ms: u64,
    ) -> Result<DurableCacheInvalidationRecord> {
        DurableCacheInvalidationRecord::new(
            envelope.id,
            Some(tenant_id),
            TENANT_CACHE_GENERATION_CHANNEL,
            tenant_id.to_string(),
            generation,
            emitted_at_unix_ms,
            envelope.event_type.clone(),
            envelope.trace_id.clone(),
        )
        .map_err(|error| Error::Cache(error.to_string()))
    }

    async fn publish_generation_if_needed(&self, envelope: &EventEnvelope) -> Result<()> {
        let Some(tenant_id) = tenant_cache_event_tenant_id(&envelope.event) else {
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

        let _ = self.successful_rotations.observe(envelope.id);
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

    async fn acknowledge(&self, event_id: uuid::Uuid) -> Result<()> {
        self.inner.acknowledge(event_id).await
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        self.inner.reliability_level()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn tenant_cache_event_tenant_id(event: &DomainEvent) -> Option<uuid::Uuid> {
    match event {
        DomainEvent::TenantCreated { tenant_id }
        | DomainEvent::TenantUpdated { tenant_id }
        | DomainEvent::TenantModuleToggled { tenant_id, .. }
        | DomainEvent::LocaleEnabled { tenant_id, .. }
        | DomainEvent::LocaleDisabled { tenant_id, .. } => Some(*tenant_id),
        _ => None,
    }
}

#[derive(Clone)]
struct TenantCacheGenerationListener {
    cache: CacheService,
    tracker: BoundedCacheInvalidationGapTracker,
}

impl TenantCacheGenerationListener {
    fn new(cache: CacheService) -> Self {
        Self {
            cache,
            tracker: BoundedCacheInvalidationGapTracker::default(),
        }
    }

    async fn recover_shared_generation(&self) -> Result<u64> {
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
        let started_at = Instant::now();
        let event = VersionedCacheInvalidation::from_message(&message)
            .map_err(|error| Error::Cache(error.to_string()))?;
        if event.channel != TENANT_CACHE_GENERATION_CHANNEL {
            return Err(Error::Validation(format!(
                "unexpected tenant cache invalidation channel {}",
                event.channel
            )));
        }

        match self.tracker.observe(&event) {
            CacheInvalidationObservation::InOrder { generation } => {
                observe_tenant_backend_generation(generation)?;
                self.acknowledge_applied_generation(generation)?;
            }
            CacheInvalidationObservation::Duplicate { .. }
            | CacheInvalidationObservation::Stale { .. } => {}
            CacheInvalidationObservation::UnverifiedFirst { .. }
            | CacheInvalidationObservation::Gap { .. } => {
                let recovered = self.recover_shared_generation().await?;
                if recovered < event.generation {
                    return Err(Error::Cache(format!(
                        "shared tenant cache generation {recovered} trails received {}",
                        event.generation
                    )));
                }
            }
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
}

#[derive(Clone, Default)]
struct TenantCacheGenerationListenerStartLock(Arc<tokio::sync::Mutex<()>>);

pub async fn tenant_cache_generation_listener_snapshot(
    ctx: &ServerRuntimeContext,
) -> TenantCacheGenerationListenerSnapshot {
    match ctx.shared_get::<TenantCacheGenerationListenerHandle>() {
        Some(handle) => handle.state.snapshot().await,
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
    let _ = ctx.shared_insert_if_absent(TenantCacheGenerationListenerStartLock::default());
    let start_lock = ctx
        .shared_get::<TenantCacheGenerationListenerStartLock>()
        .ok_or_else(|| Error::Cache("tenant listener start lock is unavailable".to_string()))?;
    let _start_guard = start_lock.0.lock().await;
    if ctx
        .shared_get::<TenantCacheGenerationListenerHandle>()
        .is_some()
    {
        return Ok(());
    }

    let redis_required = cache.redis_configuration_present();
    let state = TenantCacheGenerationListenerState::new(redis_required);
    let listener = TenantCacheGenerationListener::new(cache.clone());
    match listener.recover_shared_generation().await {
        Ok(_) if !redis_required => state.mark_local_healthy().await,
        Ok(_) => state.mark_reconciliation_healthy().await,
        Err(error) if redis_required => {
            state.mark_reconciliation_degraded(error.to_string()).await;
            tracing::warn!(%error, "Tenant cache generation startup recovery failed; isolated boot namespace remains active");
            rustok_telemetry::metrics::record_event_error(
                "tenant.cache.generation",
                "startup_recovery",
            );
        }
        Err(error) => {
            state.mark_local_degraded(error.to_string()).await;
            tracing::warn!(%error, "Local tenant cache generation startup recovery failed");
            rustok_telemetry::metrics::record_event_error(
                "tenant.cache.generation",
                "startup_recovery",
            );
        }
    }

    let mut local = cache
        .invalidations()
        .subscribe_local_channel(TENANT_CACHE_GENERATION_CHANNEL);
    let local_listener = listener.clone();
    let local_state = Arc::clone(&state);
    tokio::spawn(async move {
        let runtime = EventConsumerRuntime::new("tenant_cache_generation_local");
        runtime.restarted("startup");
        loop {
            match local.recv().await {
                Ok(message) => match local_listener.handle_message(message).await {
                    Ok(()) => {
                        local_state.mark_local_healthy().await;
                    }
                    Err(error) => {
                        local_state.mark_local_degraded(error.to_string()).await;
                        tracing::error!(%error, "Local tenant cache generation apply failed");
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
                        Err(error) => {
                            local_state.mark_local_degraded(error.to_string()).await;
                            tracing::error!(%error, "Tenant cache generation recovery after local lag failed");
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
    });

    if cache.redis_client_initialized() {
        let redis_listener = listener.clone();
        let invalidations = cache.invalidations();
        let redis_state = Arc::clone(&state);
        tokio::spawn(async move {
            let runtime = EventConsumerRuntime::new("tenant_cache_generation_redis");
            let mut reason = "startup";
            loop {
                runtime.restarted(reason);
                redis_state.mark_subscriber_starting();
                let ready_listener = redis_listener.clone();
                let ready_state = Arc::clone(&redis_state);
                let handler_listener = redis_listener.clone();
                let handler_state = Arc::clone(&redis_state);
                let result = invalidations
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
                                    Err(error) => {
                                        ready_state
                                            .mark_subscriber_degraded(error.to_string())
                                            .await;
                                        tracing::warn!(%error, "Tenant cache generation post-subscribe recovery failed");
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
                                    Err(error) => {
                                        handler_state
                                            .mark_subscriber_degraded(error.to_string())
                                            .await;
                                        tracing::error!(%error, "Redis tenant cache generation apply failed");
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
                    .mark_subscriber_degraded(format!(
                        "Redis tenant generation subscriber stopped: {result:?}"
                    ))
                    .await;
                tracing::warn!(?result, "Tenant cache generation Redis subscriber stopped");
                reason = "reconnect";
                tokio::time::sleep(LISTENER_RESTART_DELAY).await;
            }
        });

        let reconcile_listener = listener.clone();
        let reconcile_state = Arc::clone(&state);
        tokio::spawn(async move {
            let start = tokio::time::Instant::now() + GENERATION_RECONCILE_INTERVAL;
            let mut interval = tokio::time::interval_at(start, GENERATION_RECONCILE_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                match reconcile_listener.recover_shared_generation().await {
                    Ok(_) => reconcile_state.mark_reconciliation_healthy().await,
                    Err(error) => {
                        reconcile_state
                            .mark_reconciliation_degraded(error.to_string())
                            .await;
                        tracing::warn!(%error, "Periodic tenant cache generation reconciliation failed");
                        rustok_telemetry::metrics::record_event_error(
                            "tenant.cache.generation",
                            "periodic_reconciliation",
                        );
                    }
                }
            }
        });
    }

    ctx.shared_insert(TenantCacheGenerationListenerHandle { state });
    Ok(())
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
        let tenant_id = tenant_cache_event_tenant_id(&event).unwrap();
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
        ] {
            assert_eq!(tenant_cache_event_tenant_id(&event), Some(tenant_id));
        }
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

        let listener = TenantCacheGenerationListener::new(CacheService::from_url(None));
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
        let listener = TenantCacheGenerationListener::new(cache.clone());
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
        let listener = TenantCacheGenerationListener::new(cache);
        assert!(listener.recover_shared_generation().await.is_err());
    }
}
