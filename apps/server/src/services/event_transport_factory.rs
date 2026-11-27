use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::error::{Error, Result};
use async_trait::async_trait;
use rustok_api::{PortActor, PortContext};
use rustok_cache::CacheService;
use rustok_core::events::{
    EventBus, EventEnvelope, EventTransport, MemoryTransport, ReliabilityLevel,
};
use rustok_events::{ContractEventEnvelope, ContractEventPayload};
use rustok_iggy::IggyTransport;
use rustok_modules::{
    ArtifactEventDeliveryConfig, ArtifactEventProjectionTransport, ModuleControlPlane,
};
use rustok_outbox::{OutboxRelay, OutboxRelayPort, OutboxRelayRunOnceRequest, RelayConfig};
use tokio::task::JoinHandle;

use crate::common::settings::EventDeliveryProfile;
use crate::services::event_bus::shared_event_transport;
use crate::services::event_delivery_settings_service::EventDeliverySettingsService;
use crate::services::rbac_cache_invalidation::start_rbac_cache_invalidation_listener;
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::stop_handle::{StopHandle, await_worker_startup_or_stop};
use crate::services::tenant_cache_generation::{
    TenantCacheGenerationTransport, start_tenant_cache_generation_listener,
};
use crate::services::tenant_generation_delivery_gate::TenantGenerationDeliveryGate;

static OUTBOX_RELAY_SUPERVISOR_RESTART_TOTAL: AtomicU64 = AtomicU64::new(0);
static EVENT_LOCAL_DELIVERY_FAILURE_TOTAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct EventRuntime {
    pub delivery_profile: EventDeliveryProfile,
    pub iggy_mode: Option<rustok_iggy::config::IggyMode>,
    pub transport: Arc<dyn EventTransport>,
    /// Events accepted by the configured transport are delivered to module listeners on this bus.
    /// This is deliberately separate from the outbound publisher bus to avoid relay-to-outbox loops.
    pub listener_bus: EventBus,
    pub relay_config: Option<RelayRuntimeConfig>,
    /// Present when delivered-event retention is enabled for this deployment.
    pub outbox_retention: Option<OutboxRetentionRuntimeConfig>,
    pub channel_capacity: usize,
    pub relay_fallback_active: bool,
}

#[derive(Clone)]
pub struct RelayRuntimeConfig {
    pub interval: Duration,
    pub relay: OutboxRelay,
}

/// Resolved retention configuration: absent when retention is disabled, so the
/// readiness layer can distinguish "worker not required" from "worker stopped".
pub type OutboxRetentionRuntimeConfig =
    crate::services::outbox_retention_worker::OutboxRetentionRuntimeConfig;

/// Upper bound of the configurable retention window.
///
/// A delivered-event window longer than ten years is not a retention policy but
/// unbounded table growth, so the bound is rejected at boot instead of silently
/// keeping every delivered row forever.
const MAX_OUTBOX_RETENTION_DAYS: u64 = 3_650;
const MAX_OUTBOX_RETENTION_BATCH_SIZE: u64 = 100_000;
const MAX_OUTBOX_RETENTION_INTERVAL_SECONDS: u64 = 86_400;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OutboxRelaySupervisorMetricsSnapshot {
    pub restart_total: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EventLocalDeliveryMetricsSnapshot {
    pub failure_total: u64,
}

pub fn outbox_relay_supervisor_metrics_snapshot() -> OutboxRelaySupervisorMetricsSnapshot {
    OutboxRelaySupervisorMetricsSnapshot {
        restart_total: OUTBOX_RELAY_SUPERVISOR_RESTART_TOTAL.load(Ordering::Relaxed),
    }
}

pub fn event_local_delivery_metrics_snapshot() -> EventLocalDeliveryMetricsSnapshot {
    EventLocalDeliveryMetricsSnapshot {
        failure_total: EVENT_LOCAL_DELIVERY_FAILURE_TOTAL.load(Ordering::Relaxed),
    }
}

/// Serialize initialization of the complete delivery graph per host.
#[derive(Clone, Default)]
pub(crate) struct EventRuntimeInitializationLock(Arc<tokio::sync::Mutex<()>>);

impl EventRuntimeInitializationLock {
    pub(crate) fn ensure(ctx: &ServerRuntimeContext) -> Self {
        ctx.shared_get_or_insert_with(Self::default)
    }

    pub(crate) async fn lock(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.0.lock().await
    }
}

fn event_runtime_shutdown_error() -> Error {
    Error::Message("server shutdown started during event runtime initialization".to_string())
}

pub async fn build_event_runtime(ctx: &ServerRuntimeContext) -> Result<EventRuntime> {
    let stop = StopHandle::ensure(ctx);
    let initialization = EventRuntimeInitializationLock::ensure(ctx);
    let _guard = await_worker_startup_or_stop(&stop, initialization.lock())
        .await
        .ok_or_else(event_runtime_shutdown_error)?;

    // Desired policy applies only after a controlled restart. Do not replace
    // a live relay, its listener bus or its connector during a bootstrap retry.
    if let Some(runtime) = ctx.shared_get::<Arc<EventRuntime>>() {
        return Ok((*runtime).clone());
    }

    let settings = ctx.settings();
    let delivery_profile =
        await_worker_startup_or_stop(&stop, EventDeliverySettingsService::desired_profile(ctx))
            .await
            .ok_or_else(event_runtime_shutdown_error)?
            .map_err(|error| Error::BadRequest(error.to_string()))?;
    let channel_capacity = settings.events.channel_capacity;
    let cache = ctx.shared_get::<CacheService>().ok_or_else(|| {
        Error::BadRequest("CacheService must be initialized before the event runtime".to_string())
    })?;

    // Build and validate the complete delivery runtime before starting any
    // cache/listener workers. In particular, an unavailable configured Iggy
    // deployment or an invalid artifact projector must not leave detached
    // listeners running after bootstrap has failed.
    let mut iggy_transport_handle: Option<Arc<IggyTransport>> = None;
    let runtime = match delivery_profile {
        EventDeliveryProfile::Outbox | EventDeliveryProfile::OutboxIggy => {
            // Validate local DB-backed components before connecting/starting the external
            // Iggy transport. A bad retention policy or artifact projector must not leave
            // a broker process or connector alive after bootstrap fails.
            let outbox_retention =
                resolve_outbox_retention(ctx, &settings.events.outbox_retention)?;
            let relay_policy = &settings.events.relay_retry_policy;
            let max_attempts = if settings.events.dlq.enabled {
                settings.events.dlq.max_attempts
            } else {
                relay_policy.max_attempts
            };
            let relay_policy_config = RelayConfig {
                    batch_size: settings.events.relay_batch_size,
                    max_attempts,
                    backoff_base: Duration::from_millis(relay_policy.base_backoff_ms),
                    backoff_max: Duration::from_millis(relay_policy.max_backoff_ms),
                    max_concurrency: settings.events.relay_max_concurrency,
                    claim_ttl: Duration::from_millis(settings.events.relay_claim_ttl_ms),
                    // A dispatch must finish well inside its claim lease:
                    // half the lease, capped at the relay default of 30s.
                    publish_timeout: Duration::from_millis(
                        (settings.events.relay_claim_ttl_ms / 2).clamp(1, 30_000),
                    ),
                    ..RelayConfig::default()
                };
            relay_policy_config.verify_execution_admission(&ctx.db_clone()).await
                .map_err(|error| Error::BadRequest(error.to_string()))?;
            let artifact_projector = ModuleControlPlane::new(ctx.db_clone())
                .artifact_event_projector(ArtifactEventDeliveryConfig::default())
                .map_err(|error| {
                    Error::BadRequest(format!(
                        "Failed to initialize durable artifact event projection: {error}"
                    ))
                })?;
            // Early publishers may already have initialized the durable outbox.
            let outbox_transport = shared_event_transport(ctx);
            if !outbox_transport.supports_transactional_writes()
                || outbox_transport.reliability_level() != ReliabilityLevel::Outbox
            {
                return Err(Error::BadRequest(
                    "registered event transport is not a transactional outbox".to_string(),
                ));
            }
            let (relay_target, listener_bus, iggy_mode, delivery_route) = match delivery_profile {
                EventDeliveryProfile::Outbox => {
                    let local = MemoryTransport::with_capacity(channel_capacity);
                    let listener_bus = local.event_bus();
                    (
                        Arc::new(LocalEventDeliveryTransport { local }) as Arc<dyn EventTransport>,
                        listener_bus,
                        None,
                        "server-local-v1".to_owned(),
                    )
                }
                EventDeliveryProfile::OutboxIggy => {
                    let iggy_config = await_worker_startup_or_stop(
                        &stop,
                        crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::resolved_config(ctx),
                    )
                    .await
                    .ok_or_else(event_runtime_shutdown_error)?
                    .map_err(|error| Error::BadRequest(error.to_string()))?;
                    let iggy_transport = Arc::new(
                        IggyTransport::new(iggy_config.clone()).await.map_err(|error| {
                            Error::BadRequest(format!(
                                "outbox_iggy requires a configured and reachable Iggy deployment: {error}"
                            ))
                        })?,
                    );
                    // Durable inbound consumers must reuse the exact configured connector.
                    // Creating another bundled transport would start a second native broker
                    // process on the same ports and split lifecycle ownership. Register it only
                    // after the rest of event-runtime construction succeeds.
                    iggy_transport_handle = Some(Arc::clone(&iggy_transport));
                    let transport: Arc<dyn EventTransport> = iggy_transport;
                    let route = format!("server-iggy-fanout-v1:{}", iggy_config.outbox_route_fingerprint());
                    let checkpoint_enabled = iggy_config.mode == rustok_iggy::config::IggyMode::External;
                    let (transport, listener_bus) =
                        transport_with_local_delivery_policy(transport, channel_capacity, checkpoint_enabled);
                    (transport, listener_bus, Some(iggy_config.mode), route)
                }
            };
            let relay_target: Arc<dyn EventTransport> = Arc::new(
                ArtifactEventProjectionTransport::new(artifact_projector, relay_target),
            );
            // The relay target performs generation rotation synchronously. OutboxRelay therefore
            // cannot mark a tenant mutation dispatched until cache invalidation has been published
            // and the canonical tenant generation listener is ready for the configured transport.
            let relay_target = tenant_generation_transport(ctx, &cache, relay_target);
            let relay_config = RelayRuntimeConfig {
                interval: Duration::from_millis(settings.events.relay_interval_ms),
                relay: OutboxRelay::new(ctx.db_clone(), relay_target).with_config(relay_policy_config).with_delivery_route(delivery_route),
            };

            EventRuntime {
                delivery_profile,
                iggy_mode,
                transport: outbox_transport,
                listener_bus,
                relay_config: Some(relay_config),
                outbox_retention,
                channel_capacity,
                relay_fallback_active: false,
            }
        }
    };

    // All fallible transport/projector/configuration work has completed. Start the
    // generation listeners before returning the runtime, so no relay can publish into
    // an unobserved cache-generation channel.
    if let Err(error) = start_tenant_cache_generation_listener(ctx, cache.clone()).await {
        shutdown_unpublished_iggy(iggy_transport_handle.as_ref()).await;
        return Err(error.into());
    }
    if let Err(error) = start_rbac_cache_invalidation_listener(ctx, cache.clone()).await {
        let tasks =
            crate::services::tenant_cache_generation::take_listener_tasks_for_shutdown(ctx).await;
        for task in &tasks {
            task.abort();
        }
        for task in tasks {
            if let Err(join_err) = task.await {
                if !join_err.is_cancelled() {
                    tracing::warn!(
                        failure_code = "tenant_cache_generation_listener_join_failed",
                        "Aborted tenant cache generation task failed unexpectedly"
                    );
                }
            }
        }
        shutdown_unpublished_iggy(iggy_transport_handle.as_ref()).await;
        return Err(error);
    }
    crate::services::cache_redis_status_monitor::start_cache_redis_status_monitor(
        ctx,
        cache.clone(),
    )
    .await;

    if stop.is_stopping() {
        shutdown_unpublished_iggy(iggy_transport_handle.as_ref()).await;
        return Err(event_runtime_shutdown_error());
    }
    if let Some(iggy_transport) = iggy_transport_handle {
        ctx.shared_insert(iggy_transport);
    }
    // Module listeners and relay startup resolve this exact published graph.
    ctx.shared_insert(Arc::new(runtime.clone()));
    Ok(runtime)
}

async fn shutdown_unpublished_iggy(transport: Option<&Arc<IggyTransport>>) {
    if let Some(transport) = transport {
        if transport.shutdown().await.is_err() {
            tracing::warn!(
                error_code = "event_runtime_unpublished_iggy_shutdown_failed",
                "Failed to shut down unpublished Iggy transport"
            );
        }
    }
}

/// Turns the configured retention policy into the worker's runtime shape.
///
/// Returns `None` when retention is disabled: the outbox then keeps every
/// delivered row and readiness does not report a missing retention worker.
fn resolve_outbox_retention(
    ctx: &ServerRuntimeContext,
    settings: &crate::common::settings::OutboxRetentionSettings,
) -> Result<Option<OutboxRetentionRuntimeConfig>> {
    if !settings.enabled {
        return Ok(None);
    }

    if settings.retention_days == 0 || settings.retention_days > MAX_OUTBOX_RETENTION_DAYS {
        return Err(Error::BadRequest(format!(
            "events.outbox_retention.retention_days must be between 1 and {MAX_OUTBOX_RETENTION_DAYS}"
        )));
    }
    if settings.batch_size == 0 || settings.batch_size > MAX_OUTBOX_RETENTION_BATCH_SIZE {
        return Err(Error::BadRequest(format!(
            "events.outbox_retention.batch_size must be between 1 and {MAX_OUTBOX_RETENTION_BATCH_SIZE}"
        )));
    }
    if settings.interval_seconds == 0
        || settings.interval_seconds > MAX_OUTBOX_RETENTION_INTERVAL_SECONDS
    {
        return Err(Error::BadRequest(format!(
            "events.outbox_retention.interval_seconds must be between 1 and {MAX_OUTBOX_RETENTION_INTERVAL_SECONDS}"
        )));
    }

    if settings.failed_retention_days > MAX_OUTBOX_RETENTION_DAYS {
        return Err(Error::BadRequest(format!(
            "events.outbox_retention.failed_retention_days must be between 0 (keep forever) and {MAX_OUTBOX_RETENTION_DAYS}"
        )));
    }
    if settings.receipt_retention_days > MAX_OUTBOX_RETENTION_DAYS {
        return Err(Error::BadRequest(format!(
            "events.outbox_retention.receipt_retention_days must be between 0 (keep forever) and {MAX_OUTBOX_RETENTION_DAYS}"
        )));
    }

    let retention = rustok_outbox::OutboxRetentionConfig {
        retention: chrono::Duration::days(i64::try_from(settings.retention_days).map_err(
            |_| Error::BadRequest("outbox retention window is out of range".to_string()),
        )?),
        batch_size: settings.batch_size,
        failed_retention: (settings.failed_retention_days > 0)
            .then(|| chrono::Duration::days(settings.failed_retention_days as i64)),
        receipt_retention: (settings.receipt_retention_days > 0)
            .then(|| chrono::Duration::days(settings.receipt_retention_days as i64)),
    };
    retention.validate().map_err(|error| {
        Error::BadRequest(format!(
            "outbox retention configuration is invalid: {error}"
        ))
    })?;

    Ok(Some(OutboxRetentionRuntimeConfig {
        interval: Duration::from_secs(settings.interval_seconds),
        retention: rustok_outbox::OutboxRetention::new(ctx.db_clone()).with_config(retention),
    }))
}

fn tenant_generation_transport(
    ctx: &ServerRuntimeContext,
    cache: &CacheService,
    downstream: Arc<dyn EventTransport>,
) -> Arc<dyn EventTransport> {
    let gated: Arc<dyn EventTransport> = Arc::new(TenantGenerationDeliveryGate::new(
        downstream,
        ctx.clone(),
        cache.clone(),
    ));
    Arc::new(TenantCacheGenerationTransport::new(gated, cache.clone()))
}

pub fn spawn_outbox_relay_worker(
    config: RelayRuntimeConfig,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if *stop_rx.borrow() {
                tracing::info!("Outbox relay supervisor received shutdown signal, exiting");
                return;
            }

            let relay = config.relay.clone();
            let interval = config.interval;
            tracing::info!(
                worker = "outbox_relay",
                interval_ms = interval.as_millis() as u64,
                "Outbox relay worker loop starting"
            );

            let mut inner_tasks = tokio::task::JoinSet::new();
            // The inner poller owns the current durable relay pass. Shutdown
            // must stop admission of the next batch, not cancel this one
            // between transport publication and its database acknowledgement.
            let mut inner_stop_rx = stop_rx.clone();
            inner_tasks.spawn(async move {
                loop {
                    if *inner_stop_rx.borrow() {
                        break;
                    }
                    if let Err(error) = OutboxRelayPort::process_pending_once(
                        &relay,
                        outbox_relay_worker_context(),
                        OutboxRelayRunOnceRequest {
                            max_batch_hint: None,
                        },
                    )
                    .await
                    {
                        tracing::error!(
                            kind = ?error.kind,
                            code = %error.code,
                            retryable = error.retryable,
                            "Relay processing error"
                        );
                    }
                    if *inner_stop_rx.borrow() {
                        break;
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(interval) => {}
                        changed = inner_stop_rx.changed() => {
                            if changed.is_err() || *inner_stop_rx.borrow() {
                                break;
                            }
                        }
                    }
                }
            });

            tokio::select! {
                result = inner_tasks.join_next() => {
                    if *stop_rx.borrow() {
                        tracing::info!("Outbox relay supervisor received shutdown signal, exiting");
                        return;
                    }
                    OUTBOX_RELAY_SUPERVISOR_RESTART_TOTAL.fetch_add(1, Ordering::Relaxed);
                    match result {
                        Some(Err(_error)) => tracing::error!(
                            failure_code = "outbox_relay_worker_join_failed",
                            "Outbox relay worker failed; restarting in 5s."
                        ),
                        Some(Ok(())) => tracing::error!(
                            "Outbox relay worker exited unexpectedly. Restarting in 5s."
                        ),
                        None => tracing::error!(
                            "Outbox relay worker set became empty unexpectedly. Restarting in 5s."
                        ),
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                        _ = stop_rx.changed() => {
                            tracing::info!(
                                "Outbox relay supervisor received shutdown signal during restart delay, exiting"
                            );
                            return;
                        }
                    }
                }
                _ = stop_rx.changed() => {
                    tracing::info!(
                        "Outbox relay supervisor received shutdown signal; draining current relay pass"
                    );
                    // Unlike abort_all(), a cooperative drain lets the owner
                    // commit its outcome after transport publication. The
                    // host's bounded shutdown still enforces the outer deadline.
                    while let Some(result) = inner_tasks.join_next().await {
                        if let Err(error) = result {
                            tracing::error!(
                                ?error,
                                failure_code = "outbox_relay_shutdown_join_failed",
                                "Outbox relay failed while draining its current pass"
                            );
                        }
                    }
                    return;
                }
            }
        }
    })
}

#[cfg(test)]
mod outbox_relay_shutdown_tests {
    use super::*;
    use sea_orm::{Database, EntityTrait, Set};
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use std::sync::Mutex;
    use tokio::sync::oneshot;
    use uuid::Uuid;

    struct BlockingDelivery {
        started: Mutex<Option<oneshot::Sender<()>>>,
        complete: Mutex<Option<oneshot::Receiver<()>>>,
    }

    #[async_trait::async_trait]
    impl EventTransport for BlockingDelivery {
        async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
            let started = self.started.lock().expect("started lock").take();
            if let Some(started) = started {
                started.send(()).expect("notify dispatch start");
            }
            let complete = self
                .complete
                .lock()
                .expect("complete lock")
                .take()
                .expect("one dispatched event");
            complete.await.expect("permit external delivery completion");
            Ok(())
        }

        fn reliability_level(&self) -> ReliabilityLevel {
            ReliabilityLevel::InMemory
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[tokio::test]
    async fn graceful_stop_finishes_outbox_publication_and_acknowledgement() {
        use rustok_outbox::{SysEventsMigration, entity};
        let db = Database::connect("sqlite::memory:").await.expect("sqlite");
        SysEventsMigration
            .up(&SchemaManager::new(&db))
            .await
            .expect("outbox schema");

        let tenant_id = Uuid::new_v4();
        let event = EventEnvelope::new(
            tenant_id,
            None,
            rustok_events::DomainEvent::TenantUpdated { tenant_id },
        );
        entity::Entity::insert(entity::ActiveModel {
            id: Set(event.id),
            tenant_id: Set(Some(tenant_id)),
            event_type: Set(event.event_type.clone()),
            schema_version: Set(i16::try_from(event.schema_version).expect("schema version fits")),
            payload: Set(serde_json::to_value(&event).expect("serialize event")),
            status: Set(entity::SysEventStatus::Pending),
            retry_count: Set(0),
            next_attempt_at: Set(None),
            last_error: Set(None),
            claimed_by: Set(None),
            claimed_at: Set(None),
            created_at: Set(chrono::Utc::now()),
            dispatched_at: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("queue one event");

        let (started_tx, started_rx) = oneshot::channel();
        let (complete_tx, complete_rx) = oneshot::channel();
        let transport = Arc::new(BlockingDelivery {
            started: Mutex::new(Some(started_tx)),
            complete: Mutex::new(Some(complete_rx)),
        });
        let relay = OutboxRelay::new(db.clone(), transport).with_config(RelayConfig {
            worker_id: "graceful-outbox-relay".to_string(),
            ..RelayConfig::default()
        });
        let (stop, stop_rx) = crate::services::stop_handle::StopHandle::new();
        let supervisor = spawn_outbox_relay_worker(
            RelayRuntimeConfig {
                interval: Duration::from_millis(5),
                relay,
            },
            stop_rx,
        );

        tokio::time::timeout(Duration::from_secs(2), started_rx)
            .await
            .expect("outbox dispatch began")
            .expect("publication started");
        stop.stop().await;
        assert!(
            !supervisor.is_finished(),
            "stop must not cancel the current publish before acknowledgement"
        );
        complete_tx.send(()).expect("permit publish completion");
        tokio::time::timeout(Duration::from_secs(2), supervisor)
            .await
            .expect("cooperative relay drain")
            .expect("relay supervisor joined");

        let acknowledged = entity::Entity::find_by_id(event.id)
            .one(&db)
            .await
            .expect("read outbox event")
            .expect("queued event retained");
        assert_eq!(acknowledged.status, entity::SysEventStatus::Dispatched);
    }
}

fn outbox_relay_worker_context() -> PortContext {
    PortContext::new(
        "platform",
        PortActor::service("rustok-server.outbox-relay-worker"),
        "und",
        format!("outbox-relay-worker:{}", uuid::Uuid::new_v4()),
    )
    .with_idempotency_key(format!("outbox-relay-tick:{}", uuid::Uuid::new_v4()))
    // One relay pass may dispatch a full batch; the deadline bounds the
    // whole pass, not a single event, so it must cover batch latency.
    .with_deadline(Duration::from_secs(600))
}

#[cfg(test)]
fn transport_with_local_delivery(
    primary: Arc<dyn EventTransport>,
    channel_capacity: usize,
) -> (Arc<dyn EventTransport>, EventBus) {
    transport_with_local_delivery_policy(primary, channel_capacity, false)
}

fn transport_with_local_delivery_policy(
    primary: Arc<dyn EventTransport>,
    channel_capacity: usize,
    checkpoint_enabled: bool,
) -> (Arc<dyn EventTransport>, EventBus) {
    let local = MemoryTransport::with_capacity(channel_capacity);
    let listener_bus = local.event_bus();
    let transport = LocalDeliveryFanoutTransport { primary, local, checkpoint_enabled };
    (Arc::new(transport), listener_bus)
}

#[derive(Clone)]
struct LocalEventDeliveryTransport {
    local: MemoryTransport,
}

#[async_trait]
impl EventTransport for LocalEventDeliveryTransport {
    async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
        let event_id = envelope.id;
        let event_type = envelope.event.event_type();
        if let Err(error) = self
            .local
            .event_bus()
            .publish_root_envelope_and_wait(envelope)
            .await
        {
            EVENT_LOCAL_DELIVERY_FAILURE_TOTAL.fetch_add(1, Ordering::Relaxed);
            rustok_telemetry::metrics::record_event_error(event_type, "local_delivery");
            tracing::error!(
                event_id = %event_id,
                event_type,
                failure_code = "local_root_event_delivery_failed",
                "Local root-event delivery failed"
            );
            return Err(error);
        }
        Ok(())
    }

    async fn publish_contract(&self, envelope: ContractEventEnvelope) -> rustok_core::Result<()> {
        let event_id = envelope.id();
        let event_type = envelope.event_type().to_owned();
        if let Err(error) = self.local.publish_contract(envelope).await {
            EVENT_LOCAL_DELIVERY_FAILURE_TOTAL.fetch_add(1, Ordering::Relaxed);
            rustok_telemetry::metrics::record_event_error(&event_type, "local_contract_delivery");
            tracing::error!(
                %event_id,
                %event_type,
                failure_code = "local_contract_delivery_failed",
                "Local contract delivery failed"
            );
            return Err(error);
        }
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::InMemory
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone)]
struct LocalDeliveryFanoutTransport {
    primary: Arc<dyn EventTransport>,
    local: MemoryTransport,
    checkpoint_enabled: bool,
}

#[async_trait]
impl EventTransport for LocalDeliveryFanoutTransport {
    async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
        // Refuse before remote publication if the local listener registry is unavailable.
        // A committed external-primary checkpoint survives local failure.
        // Without that evidence the original event identity can redeliver.
        let local_bus = self.local.event_bus();
        local_bus.ensure_local_listeners_running()?;
        rustok_outbox::publish_primary_with_checkpoint(envelope.id, true, self.checkpoint_enabled, self.primary.publish(envelope.clone())).await?;
        let event_id = envelope.id;
        let event_type = envelope.event.event_type();
        if let Err(error) = local_bus.publish_root_envelope_and_wait(envelope).await {
            EVENT_LOCAL_DELIVERY_FAILURE_TOTAL.fetch_add(1, Ordering::Relaxed);
            rustok_telemetry::metrics::record_event_error(event_type, "local_delivery");
            tracing::error!(
                event_id = %event_id,
                event_type,
                failure_code = "remote_accepted_local_root_delivery_failed",
                "Remote event was accepted but acknowledged local module delivery failed"
            );
            return Err(error);
        }
        Ok(())
    }

    async fn publish_contract(&self, envelope: ContractEventEnvelope) -> rustok_core::Result<()> {
        let payload = envelope
            .payload()
            .map_err(|error| rustok_core::Error::Validation(error.to_string()))?;
        let local_bus = self.local.event_bus();
        // Validate the explicit registry before the irreversible primary publish,
        // including for root envelopes and remote-only sealed families.
        let has_local_handler = local_bus.has_contract_handler(&envelope)?;
        let deliver_locally = matches!(payload, ContractEventPayload::Root(_)) || has_local_handler;
        if payload.requires_module_delivery() && !deliver_locally {
            return Err(rustok_core::Error::External(
                "Required sealed module delivery is unavailable; retain the original outbox identity".to_owned(),
            ));
        }
        // Pin the local delivery obligation and retain confirmed primary
        // acceptance before attempting the process-local projection.
        rustok_outbox::publish_primary_with_checkpoint(envelope.id(), deliver_locally, self.checkpoint_enabled, self.primary.publish_contract(envelope.clone())).await?;
        if deliver_locally {
            let event_id = envelope.id();
            let event_type = envelope.event_type().to_owned();
            if let Err(error) = local_bus.publish_contract_envelope(envelope).await {
                EVENT_LOCAL_DELIVERY_FAILURE_TOTAL.fetch_add(1, Ordering::Relaxed);
                rustok_telemetry::metrics::record_event_error(
                    &event_type,
                    "local_contract_delivery",
                );
                tracing::error!(
                    %event_id, %event_type, code = "events.contract_local_delivery_failed",
                    "Local contract delivery failed; retain the same outbox identity for retry"
                );
                return Err(error);
            }
        }
        Ok(())
    }

    async fn acknowledge(&self, event_id: uuid::Uuid) -> rustok_core::Result<()> {
        self.primary.acknowledge(event_id).await
    }

    // Transactional appends go to the primary only; local fanout is a
    // publish-time concern.
    fn supports_transactional_writes(&self) -> bool {
        self.primary.supports_transactional_writes()
    }

    async fn write_root_in_transaction(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        envelope: EventEnvelope,
    ) -> rustok_core::Result<()> {
        self.primary
            .write_root_in_transaction(transaction, envelope)
            .await
    }

    async fn write_contract_in_transaction(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        envelope: ContractEventEnvelope,
    ) -> rustok_core::Result<()> {
        self.primary
            .write_contract_in_transaction(transaction, envelope)
            .await
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        self.primary.reliability_level()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::event_bus::assert_root_delivery;
    use async_trait::async_trait;
    use rustok_core::events::{EventDispatcher, EventHandler, HandlerResult, RunningDispatcher};
    use rustok_events::{DomainEvent, EventEnvelope};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use uuid::Uuid;

    struct RootListener {
        handled: Arc<Mutex<Vec<EventEnvelope>>>,
        fail: Arc<AtomicBool>,
    }

    #[async_trait]
    impl EventHandler for RootListener {
        fn name(&self) -> &'static str {
            "root_local_delivery_regression"
        }

        fn handles(&self, event: &DomainEvent) -> bool {
            matches!(event, DomainEvent::TenantUpdated { .. })
        }

        async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
            self.handled.lock().unwrap().push(envelope.clone());
            if self.fail.load(Ordering::SeqCst) {
                return Err(rustok_core::Error::External(
                    "root projection unavailable".to_owned(),
                ));
            }
            Ok(())
        }
    }

    fn listen_root(
        bus: EventBus,
        handled: Arc<Mutex<Vec<EventEnvelope>>>,
        fail: Arc<AtomicBool>,
    ) -> RunningDispatcher {
        let mut dispatcher = EventDispatcher::new(bus);
        dispatcher.register(RootListener { handled, fail });
        dispatcher.start_with_contract_delivery().unwrap()
    }

    #[tokio::test]
    async fn existing_event_runtime_is_reused_without_rereading_desired_policy() {
        // A disconnected DB proves that reuse does not query desired settings.
        let ctx = ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            crate::common::settings::RustokSettings::default(),
        );
        let transport = shared_event_transport(&ctx);
        let published = Arc::new(EventRuntime {
            delivery_profile: EventDeliveryProfile::Outbox,
            iggy_mode: None,
            transport: transport.clone(),
            listener_bus: MemoryTransport::with_capacity(8).event_bus(),
            relay_config: None,
            outbox_retention: None,
            channel_capacity: 8,
            relay_fallback_active: false,
        });
        ctx.shared_insert(published.clone());

        let (first, second) = tokio::join!(build_event_runtime(&ctx), build_event_runtime(&ctx));
        for result in [first, second] {
            let runtime = result.expect("published event runtime must be reused");
            assert!(Arc::ptr_eq(&runtime.transport, &transport));
            assert_eq!(runtime.delivery_profile, EventDeliveryProfile::Outbox);
        }
        assert!(Arc::ptr_eq(
            &ctx.shared_get::<Arc<EventRuntime>>().unwrap(),
            &published,
        ));
        StopHandle::ensure(&ctx).stop().await;
        assert!(build_event_runtime(&ctx).await.is_err());
    }

    #[tokio::test]
    async fn stopped_event_runtime_initialization_does_not_wait_for_busy_owner() {
        let ctx = ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            crate::common::settings::RustokSettings::default(),
        );
        let initialization = EventRuntimeInitializationLock::ensure(&ctx);
        let _guard = initialization.lock().await;
        StopHandle::ensure(&ctx).stop().await;
        let result = tokio::time::timeout(Duration::from_secs(1), build_event_runtime(&ctx))
            .await
            .expect("stopped initializer must not wait for initialization lock");
        assert!(result.is_err());
        assert!(!ctx.shared_contains::<Arc<EventRuntime>>());
    }

    #[tokio::test]
    async fn failed_event_runtime_validation_does_not_leave_cache_workers_running() {
        let db = rustok_test_utils::setup_test_db().await;
        let mut settings = crate::common::settings::RustokSettings::default();
        settings.events.delivery_profile = crate::common::settings::EventDeliveryProfile::Outbox;
        settings.events.outbox_retention.enabled = true;
        settings.events.outbox_retention.retention_days = 0;
        let ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(db, settings);
        crate::services::cache_runtime::ensure_cache_service(&ctx);

        let error = match build_event_runtime(&ctx).await {
            Err(error) => error,
            Ok(_) => panic!("invalid retention settings must fail event runtime bootstrap"),
        };
        assert!(error.to_string().contains("retention_days"));
        assert!(
            !ctx.shared_contains::<crate::services::tenant_cache_generation::TenantCacheGenerationListenerHandle>(),
            "tenant listener must not start before runtime validation succeeds"
        );
        assert!(
            !ctx.shared_contains::<crate::services::rbac_cache_invalidation::RbacCacheInvalidationListenerHandle>(),
            "RBAC listener must not start before runtime validation succeeds"
        );
        assert!(
            !ctx.shared_contains::<crate::services::cache_redis_status_monitor::CacheRedisStatusMonitorHandle>(),
            "Redis status monitor must not start before runtime validation succeeds"
        );
    }

    #[test]
    fn outbox_retention_rejects_intervals_that_can_disable_pruning() {
        let ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            crate::common::settings::RustokSettings::default(),
        );
        let settings = crate::common::settings::OutboxRetentionSettings {
            enabled: true,
            retention_days: 30,
            batch_size: 100,
            interval_seconds: MAX_OUTBOX_RETENTION_INTERVAL_SECONDS + 1,
            ..Default::default()
        };
        let error = match resolve_outbox_retention(&ctx, &settings) {
            Err(error) => error,
            Ok(_) => panic!("retention polling must remain bounded"),
        };
        assert!(
            error
                .to_string()
                .contains("interval_seconds must be between")
        );
    }

    #[tokio::test]
    async fn single_node_outbox_target_counts_local_root_projection_failures() {
        let local = MemoryTransport::with_capacity(8);
        let listener_bus = local.event_bus();
        let target = LocalEventDeliveryTransport { local };
        let handled = Arc::new(Mutex::new(vec![]));
        let running = listen_root(
            listener_bus,
            handled.clone(),
            Arc::new(AtomicBool::new(true)),
        );
        let envelope = EventEnvelope::new(
            Uuid::from_u128(11),
            None,
            DomainEvent::TenantUpdated {
                tenant_id: Uuid::from_u128(11),
            },
        );
        let failures_before = event_local_delivery_metrics_snapshot().failure_total;

        assert!(target.publish(envelope).await.is_err());
        assert_eq!(handled.lock().unwrap().len(), 1);
        assert!(
            event_local_delivery_metrics_snapshot().failure_total > failures_before,
            "single-node local delivery failures must be observable"
        );
        running.stop().await.expect("root dispatcher stop");
    }

    #[tokio::test]
    async fn single_node_outbox_target_waits_for_root_projection_completion() {
        let local = MemoryTransport::with_capacity(8);
        let listener_bus = local.event_bus();
        let target = LocalEventDeliveryTransport { local };
        let handled = Arc::new(Mutex::new(vec![]));
        let running = listen_root(
            listener_bus,
            handled.clone(),
            Arc::new(AtomicBool::new(false)),
        );
        let envelope = EventEnvelope::new(
            Uuid::from_u128(10),
            None,
            DomainEvent::TenantUpdated {
                tenant_id: Uuid::from_u128(10),
            },
        );

        target.publish(envelope.clone()).await.unwrap();
        assert_root_delivery(&handled.lock().unwrap(), &[&envelope]);
        running.stop().await.expect("root dispatcher stop");
    }

    #[tokio::test]
    async fn fanout_transport_waits_for_local_root_handler_after_primary_acceptance() {
        let primary = MemoryTransport::with_capacity(8);
        let mut primary_receiver = primary.subscribe();
        let (transport, listener_bus) = transport_with_local_delivery(Arc::new(primary), 8);
        let handled = Arc::new(Mutex::new(vec![]));
        let running = listen_root(
            listener_bus,
            handled.clone(),
            Arc::new(AtomicBool::new(false)),
        );
        let envelope = EventEnvelope::new(
            Uuid::from_u128(1),
            None,
            DomainEvent::TenantUpdated {
                tenant_id: Uuid::from_u128(1),
            },
        );

        transport.publish(envelope.clone()).await.unwrap();
        assert_eq!(primary_receiver.recv().await.unwrap().id, envelope.id);
        assert_root_delivery(&handled.lock().unwrap(), &[&envelope]);
        running.stop().await.expect("root dispatcher stop");
    }

    #[tokio::test]
    async fn fanout_transport_does_not_deliver_locally_when_primary_rejects() {
        let primary = MemoryTransport::with_capacity(8);
        let (transport, listener_bus) = transport_with_local_delivery(Arc::new(primary), 8);
        let handled = Arc::new(Mutex::new(vec![]));
        let running = listen_root(
            listener_bus,
            handled.clone(),
            Arc::new(AtomicBool::new(false)),
        );
        let envelope = EventEnvelope::new(
            Uuid::from_u128(2),
            None,
            DomainEvent::TenantUpdated {
                tenant_id: Uuid::from_u128(2),
            },
        );

        assert!(transport.publish(envelope).await.is_err());
        assert!(handled.lock().unwrap().is_empty());
        running.stop().await.expect("root dispatcher stop");
    }

    #[tokio::test]
    async fn missing_local_dispatcher_is_rejected_before_remote_publication() {
        let primary = MemoryTransport::with_capacity(8);
        let mut primary_receiver = primary.subscribe();
        let (transport, listener_bus) = transport_with_local_delivery(Arc::new(primary), 8);
        let envelope = EventEnvelope::new(
            Uuid::from_u128(3),
            None,
            DomainEvent::TenantUpdated {
                tenant_id: Uuid::from_u128(3),
            },
        );

        assert!(
            transport.publish(envelope.clone()).await.is_err(),
            "a missing local dispatcher must refuse before irreversible publication"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(10), primary_receiver.recv())
                .await
                .is_err(),
            "the primary must not receive an event without a bound local registry"
        );

        let running = EventDispatcher::new(listener_bus)
            .start_with_contract_delivery()
            .unwrap();
        transport.publish(envelope.clone()).await.unwrap();
        assert_eq!(primary_receiver.recv().await.unwrap().id, envelope.id);
        running.stop().await.expect("empty root dispatcher stop");
    }

    #[tokio::test]
    async fn durable_fanout_retries_local_projection_without_republishing_confirmed_primary() {
        use rustok_outbox::{OutboxTransport, SysEventsMigration, entity};
        use sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter, Set};
        use sea_orm_migration::{MigrationTrait, SchemaManager};
        let db = Database::connect("sqlite::memory:").await.unwrap();
        SysEventsMigration.up(&SchemaManager::new(&db)).await.unwrap();
        let primary = MemoryTransport::with_capacity(8);
        let mut primary_receiver = primary.subscribe();
        let primary: Arc<dyn EventTransport> = Arc::new(primary);
        let (transport, listener_bus) = transport_with_local_delivery_policy(primary.clone(), 8, true);
        let handled = Arc::new(Mutex::new(vec![]));
        let fail = Arc::new(AtomicBool::new(true));
        let running = listen_root(listener_bus, handled.clone(), fail);
        let tenant_id = Uuid::new_v4();
        let envelope = EventEnvelope::new(tenant_id, None, DomainEvent::TenantUpdated { tenant_id });
        OutboxTransport::new(db.clone()).publish(envelope.clone()).await.unwrap();
        let relay = OutboxRelay::new(db.clone(), transport)
            .with_delivery_route("server-external-test-v1");
        assert_eq!(relay.process_pending_once(None).await.unwrap(), 1);
        assert_eq!(primary_receiver.try_recv().unwrap().id, envelope.id);
        let pending = entity::Entity::find_by_id(envelope.id).one(&db).await.unwrap().unwrap();
        assert_eq!(pending.status, entity::SysEventStatus::Pending);
        assert_eq!(pending.retry_count, 1);
        running.stop().await.unwrap();
        drop(relay);
        entity::Entity::update_many().filter(entity::Column::Id.eq(envelope.id))
            .set(entity::ActiveModel { next_attempt_at: Set(None), ..Default::default() })
            .exec(&db).await.unwrap();

        let (transport, listener_bus) = transport_with_local_delivery_policy(primary, 8, true);
        let running = listen_root(listener_bus, handled.clone(), Arc::new(AtomicBool::new(false)));
        let restarted = OutboxRelay::new(db.clone(), transport)
            .with_delivery_route("server-external-test-v1");
        assert_eq!(restarted.process_pending_once(None).await.unwrap(), 1);
        let delivered = entity::Entity::find_by_id(envelope.id).one(&db).await.unwrap().unwrap();
        assert_eq!(delivered.status, entity::SysEventStatus::Dispatched);
        assert!(matches!(primary_receiver.try_recv(), Err(tokio::sync::broadcast::error::TryRecvError::Empty)));
        assert_root_delivery(&handled.lock().unwrap(), &[&envelope, &envelope]);
        running.stop().await.unwrap();
    }

    #[tokio::test]
    async fn local_root_projection_failure_is_returned_to_the_outbox_relay() {
        let primary = MemoryTransport::with_capacity(8);
        let mut primary_receiver = primary.subscribe();
        let (transport, listener_bus) = transport_with_local_delivery(Arc::new(primary), 8);
        let handled = Arc::new(Mutex::new(vec![]));
        let fail = Arc::new(AtomicBool::new(true));
        let running = listen_root(listener_bus, handled.clone(), fail.clone());
        let envelope = EventEnvelope::new(
            Uuid::from_u128(4),
            None,
            DomainEvent::TenantUpdated {
                tenant_id: Uuid::from_u128(4),
            },
        );

        assert!(transport.publish(envelope.clone()).await.is_err());
        assert_eq!(primary_receiver.recv().await.unwrap().id, envelope.id);
        assert_eq!(handled.lock().unwrap().len(), 1);

        fail.store(false, Ordering::SeqCst);
        transport.publish(envelope.clone()).await.unwrap();
        assert_eq!(primary_receiver.recv().await.unwrap().id, envelope.id);
        assert_root_delivery(&handled.lock().unwrap(), &[&envelope, &envelope]);
        running.stop().await.expect("root dispatcher stop");
    }
}

#[cfg(test)]
mod contract_delivery_tests {
    use super::*;
    use rustok_core::events::{EventDispatcher, EventHandler, HandlerResult, RunningDispatcher};
    use rustok_events::{
        DomainEvent, InventoryReservationEvent, InventorySourceEvent, ProductIndexRefreshEvent,
    };
    use std::sync::{Mutex, atomic::AtomicBool};
    use uuid::Uuid;

    /// A transport contract double, not evidence of a running Iggy broker/cursor.
    struct Primary {
        accepted: Arc<Mutex<Vec<ContractEventEnvelope>>>,
        fail: Arc<AtomicBool>,
    }

    #[async_trait]
    impl EventTransport for Primary {
        async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
            Err(rustok_core::Error::Validation(
                "no root fallback in this sealed regression".to_owned(),
            ))
        }
        async fn publish_contract(
            &self,
            envelope: ContractEventEnvelope,
        ) -> rustok_core::Result<()> {
            if self.fail.load(Ordering::SeqCst) {
                return Err(rustok_core::Error::External(
                    "primary unavailable".to_owned(),
                ));
            }
            self.accepted.lock().unwrap().push(envelope);
            Ok(())
        }
        fn reliability_level(&self) -> ReliabilityLevel {
            ReliabilityLevel::Streaming
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    struct Listener {
        handled: Arc<Mutex<Vec<ContractEventEnvelope>>>,
        fail: Arc<AtomicBool>,
    }

    #[async_trait]
    impl EventHandler for Listener {
        fn name(&self) -> &'static str {
            "sealed_local_delivery_regression"
        }
        fn handles(&self, _event: &DomainEvent) -> bool {
            false
        }
        async fn handle(&self, _envelope: &EventEnvelope) -> HandlerResult {
            Err(rustok_core::Error::Validation(
                "sealed facts are not root events".to_owned(),
            ))
        }
        fn handles_contract(&self, payload: &ContractEventPayload) -> bool {
            matches!(payload, ContractEventPayload::InventoryReservation(_))
        }
        async fn handle_contract(&self, envelope: &ContractEventEnvelope) -> HandlerResult {
            self.handled.lock().unwrap().push(envelope.clone());
            if self.fail.load(Ordering::SeqCst) {
                return Err(rustok_core::Error::External(
                    "projection unavailable".to_owned(),
                ));
            }
            Ok(())
        }
    }

    fn envelope() -> ContractEventEnvelope {
        ContractEventEnvelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            InventoryReservationEvent::Changed {
                product_id: Uuid::new_v4(),
                variant_id: Uuid::new_v4(),
            },
        )
        .unwrap()
    }

    fn listen(
        bus: EventBus,
        handled: Arc<Mutex<Vec<ContractEventEnvelope>>>,
        fail: Arc<AtomicBool>,
    ) -> RunningDispatcher {
        let mut dispatcher = EventDispatcher::new(bus);
        dispatcher.register(Listener { handled, fail });
        dispatcher.start_with_contract_delivery().unwrap()
    }

    #[tokio::test]
    async fn outbox_memory_target_delivers_the_exact_sealed_envelope_to_its_listener_bus() {
        let target = MemoryTransport::with_capacity(8);
        let handled = Arc::new(Mutex::new(vec![]));
        let running = listen(
            target.event_bus(),
            handled.clone(),
            Arc::new(AtomicBool::new(false)),
        );
        let envelope = envelope();
        target.publish_contract(envelope.clone()).await.unwrap();
        assert_eq!(*handled.lock().unwrap(), vec![envelope]);
        running.stop().await.expect("dispatcher stop");
    }

    #[tokio::test]
    async fn primary_refusal_precedes_local_sealed_delivery() {
        let accepted = Arc::new(Mutex::new(vec![]));
        let handled = Arc::new(Mutex::new(vec![]));
        let (target, bus) = transport_with_local_delivery(
            Arc::new(Primary {
                accepted: accepted.clone(),
                fail: Arc::new(AtomicBool::new(true)),
            }),
            8,
        );
        let running = listen(bus, handled.clone(), Arc::new(AtomicBool::new(false)));
        assert!(target.publish_contract(envelope()).await.is_err());
        assert!(accepted.lock().unwrap().is_empty());
        assert!(handled.lock().unwrap().is_empty());
        running.stop().await.expect("dispatcher stop");
    }

    #[tokio::test]
    async fn local_failure_retains_original_frame_for_at_least_once_primary_and_local_retry() {
        let accepted = Arc::new(Mutex::new(vec![]));
        let handled = Arc::new(Mutex::new(vec![]));
        let fail_local = Arc::new(AtomicBool::new(true));
        let (target, bus) = transport_with_local_delivery(
            Arc::new(Primary {
                accepted: accepted.clone(),
                fail: Arc::new(AtomicBool::new(false)),
            }),
            8,
        );
        let mut root = bus.subscribe();
        let running = listen(bus, handled.clone(), fail_local.clone());
        let envelope = envelope();
        assert!(target.publish_contract(envelope.clone()).await.is_err());
        fail_local.store(false, Ordering::SeqCst);
        target.publish_contract(envelope.clone()).await.unwrap();
        assert_eq!(
            *accepted.lock().unwrap(),
            vec![envelope.clone(), envelope.clone()]
        );
        assert_eq!(*handled.lock().unwrap(), vec![envelope.clone(), envelope]);
        assert!(root.try_recv().is_err());
        running.stop().await.expect("dispatcher stop");
    }

    #[tokio::test]
    async fn unregistered_or_stopped_listener_registry_cannot_silently_skip_inventory() {
        let accepted = Arc::new(Mutex::new(vec![]));
        let (target, bus) = transport_with_local_delivery(
            Arc::new(Primary {
                accepted: accepted.clone(),
                fail: Arc::new(AtomicBool::new(false)),
            }),
            8,
        );
        assert!(target.publish_contract(envelope()).await.is_err());
        let running = listen(
            bus,
            Arc::new(Mutex::new(vec![])),
            Arc::new(AtomicBool::new(false)),
        );
        running.abort();
        assert!(target.publish_contract(envelope()).await.is_err());
        assert!(accepted.lock().unwrap().is_empty());
    }

    struct SourceListener(Listener);
    #[async_trait]
    impl EventHandler for SourceListener {
        fn name(&self) -> &'static str {
            "inventory_source_delivery_regression"
        }
        fn handles(&self, _event: &DomainEvent) -> bool {
            false
        }
        async fn handle(&self, _envelope: &EventEnvelope) -> HandlerResult {
            Err(rustok_core::Error::Validation(
                "source facts have no root fallback".into(),
            ))
        }
        fn handles_contract(&self, payload: &ContractEventPayload) -> bool {
            matches!(payload, ContractEventPayload::InventorySource(_))
        }
        async fn handle_contract(&self, envelope: &ContractEventEnvelope) -> HandlerResult {
            self.0.handle_contract(envelope).await
        }
    }
    fn source_envelope(activation: bool) -> ContractEventEnvelope {
        let baseline_id = Uuid::new_v4();
        let event = if activation {
            InventorySourceEvent::BaselineActivated { baseline_id }
        } else {
            InventorySourceEvent::Changed {
                baseline_id,
                variant_id: Uuid::new_v4(),
                source_version: 2,
            }
        };
        let id = if activation {
            baseline_id
        } else {
            Uuid::new_v4()
        };
        ContractEventEnvelope::new_with_envelope_id(id, Uuid::new_v4(), Some(Uuid::new_v4()), event)
            .unwrap()
    }
    fn listen_source(
        bus: EventBus,
        handled: Arc<Mutex<Vec<ContractEventEnvelope>>>,
        fail: Arc<AtomicBool>,
    ) -> RunningDispatcher {
        let mut dispatcher = EventDispatcher::new(bus);
        dispatcher.register(SourceListener(Listener { handled, fail }));
        dispatcher.start_with_contract_delivery().unwrap()
    }

    #[tokio::test]
    async fn active_unmatched_registry_refuses_required_inventory_families_before_primary_acceptance()
     {
        let accepted = Arc::new(Mutex::new(vec![]));
        let (target, bus) = transport_with_local_delivery(
            Arc::new(Primary {
                accepted: accepted.clone(),
                fail: Arc::new(AtomicBool::new(false)),
            }),
            8,
        );
        let running = EventDispatcher::new(bus)
            .start_with_contract_delivery()
            .unwrap();
        for frame in [envelope(), source_envelope(false), source_envelope(true)] {
            assert!(target.publish_contract(frame).await.is_err());
        }
        assert!(accepted.lock().unwrap().is_empty());
        running.stop().await.expect("dispatcher stop");
    }

    #[tokio::test]
    async fn source_family_cannot_be_acknowledged_by_a_running_reservation_only_listener() {
        let accepted = Arc::new(Mutex::new(vec![]));
        let handled = Arc::new(Mutex::new(vec![]));
        let (target, bus) = transport_with_local_delivery(
            Arc::new(Primary {
                accepted: accepted.clone(),
                fail: Arc::new(AtomicBool::new(false)),
            }),
            8,
        );
        let running = listen(bus, handled.clone(), Arc::new(AtomicBool::new(false)));
        assert!(
            target
                .publish_contract(source_envelope(false))
                .await
                .is_err()
        );
        assert!(
            target
                .publish_contract(source_envelope(true))
                .await
                .is_err()
        );
        assert!(accepted.lock().unwrap().is_empty());
        assert!(handled.lock().unwrap().is_empty());
        running.stop().await.expect("dispatcher stop");
    }

    #[tokio::test]
    async fn source_memory_profile_awaits_exact_registered_facts_and_refuses_after_shutdown() {
        let target = MemoryTransport::with_capacity(8);
        let handled = Arc::new(Mutex::new(vec![]));
        let running = listen_source(
            target.event_bus(),
            handled.clone(),
            Arc::new(AtomicBool::new(false)),
        );
        let frames = [source_envelope(false), source_envelope(true)];
        for frame in &frames {
            target.publish_contract(frame.clone()).await.unwrap();
        }
        assert_eq!(*handled.lock().unwrap(), frames.to_vec());
        running.stop().await.expect("dispatcher stop");
        assert!(target.publish_contract(frames[0].clone()).await.is_err());
    }

    #[tokio::test]
    async fn source_fanout_listener_error_retains_the_same_frame_for_at_least_once_retry() {
        let accepted = Arc::new(Mutex::new(vec![]));
        let handled = Arc::new(Mutex::new(vec![]));
        let failing = Arc::new(AtomicBool::new(true));
        let (target, bus) = transport_with_local_delivery(
            Arc::new(Primary {
                accepted: accepted.clone(),
                fail: Arc::new(AtomicBool::new(false)),
            }),
            8,
        );
        let running = listen_source(bus, handled.clone(), failing.clone());
        let frame = source_envelope(false);
        assert!(target.publish_contract(frame.clone()).await.is_err());
        failing.store(false, Ordering::SeqCst);
        target.publish_contract(frame.clone()).await.unwrap();
        assert_eq!(
            *accepted.lock().unwrap(),
            vec![frame.clone(), frame.clone()]
        );
        assert_eq!(*handled.lock().unwrap(), vec![frame.clone(), frame]);
        running.stop().await.expect("dispatcher stop");
    }

    #[tokio::test]
    async fn remote_only_family_uses_primary_with_an_explicitly_empty_listener_registry() {
        let accepted = Arc::new(Mutex::new(vec![]));
        let (target, bus) = transport_with_local_delivery(
            Arc::new(Primary {
                accepted: accepted.clone(),
                fail: Arc::new(AtomicBool::new(false)),
            }),
            8,
        );
        let running = EventDispatcher::new(bus)
            .start_with_contract_delivery()
            .unwrap();
        let envelope = ContractEventEnvelope::new(
            Uuid::new_v4(),
            None,
            ProductIndexRefreshEvent::VariantRefreshRequested {
                product_id: Uuid::new_v4(),
                variant_id: Uuid::new_v4(),
                source_version: 1,
            },
        )
        .unwrap();

        target.publish_contract(envelope.clone()).await.unwrap();

        assert_eq!(*accepted.lock().unwrap(), vec![envelope]);
        running
            .stop()
            .await
            .expect("empty listener dispatcher stop");
    }

    #[tokio::test]
    async fn remote_only_family_uses_primary_without_being_reinterpreted_by_inventory_listener() {
        let accepted = Arc::new(Mutex::new(vec![]));
        let handled = Arc::new(Mutex::new(vec![]));
        let (target, bus) = transport_with_local_delivery(
            Arc::new(Primary {
                accepted: accepted.clone(),
                fail: Arc::new(AtomicBool::new(false)),
            }),
            8,
        );
        let running = listen(bus, handled.clone(), Arc::new(AtomicBool::new(false)));
        // This is only a transport-double frame, not a Product owner publication or ledger proof.
        let envelope = ContractEventEnvelope::new(
            Uuid::new_v4(),
            None,
            ProductIndexRefreshEvent::VariantRefreshRequested {
                product_id: Uuid::new_v4(),
                variant_id: Uuid::new_v4(),
                source_version: 1,
            },
        )
        .unwrap();
        target.publish_contract(envelope.clone()).await.unwrap();
        assert_eq!(*accepted.lock().unwrap(), vec![envelope]);
        assert!(handled.lock().unwrap().is_empty());
        running.stop().await.expect("dispatcher stop");
    }
}
