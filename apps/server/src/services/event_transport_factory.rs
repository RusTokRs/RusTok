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
use rustok_outbox::{
    OutboxRelay, OutboxRelayPort, OutboxRelayRunOnceRequest, OutboxTransport, RelayConfig,
};
use tokio::task::JoinHandle;

use crate::common::settings::EventDeliveryProfile;
use crate::services::event_delivery_settings_service::EventDeliverySettingsService;
use crate::services::rbac_cache_invalidation::start_rbac_cache_invalidation_listener;
use crate::services::server_runtime_context::ServerRuntimeContext;
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

pub async fn build_event_runtime(ctx: &ServerRuntimeContext) -> Result<EventRuntime> {
    let settings = ctx.settings();
    let delivery_profile = EventDeliverySettingsService::desired_profile(ctx)
        .await
        .map_err(|error| Error::BadRequest(error.to_string()))?;
    let channel_capacity = settings.events.channel_capacity;
    let cache = ctx.shared_get::<CacheService>().ok_or_else(|| {
        Error::BadRequest("CacheService must be initialized before the event runtime".to_string())
    })?;

    // Subscribe before any transport can publish a generation. This also restores shared
    // generations before middleware and authorization paths construct or read their caches.
    crate::services::cache_redis_status_monitor::start_cache_redis_status_monitor(
        ctx,
        cache.clone(),
    )
    .await;
    start_tenant_cache_generation_listener(ctx, cache.clone()).await?;
    start_rbac_cache_invalidation_listener(ctx, cache.clone()).await?;

    let runtime = match delivery_profile {
        EventDeliveryProfile::Outbox | EventDeliveryProfile::OutboxIggy => {
            // Keep the application-facing transport concrete so TransactionalEventBus can
            // downcast to OutboxTransport and write into the caller's database transaction.
            let outbox_transport = Arc::new(OutboxTransport::new(ctx.db_clone()));
            let (relay_target, listener_bus, iggy_mode) = match delivery_profile {
                EventDeliveryProfile::Outbox => {
                    let transport = MemoryTransport::with_capacity(channel_capacity);
                    let listener_bus = transport.event_bus();
                    (
                        Arc::new(transport) as Arc<dyn EventTransport>,
                        listener_bus,
                        None,
                    )
                }
                EventDeliveryProfile::OutboxIggy => {
                    let iggy_config =
                        crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::resolved_config(ctx)
                            .await
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
                    // process on the same ports and split lifecycle ownership.
                    ctx.shared_insert(Arc::clone(&iggy_transport));
                    let transport: Arc<dyn EventTransport> = iggy_transport;
                    let (transport, listener_bus) =
                        transport_with_local_delivery(transport, channel_capacity);
                    (transport, listener_bus, Some(iggy_config.mode))
                }
            };
            let artifact_projector = ModuleControlPlane::new(ctx.db_clone())
                .artifact_event_projector(ArtifactEventDeliveryConfig::default())
                .map_err(|error| {
                    Error::BadRequest(format!(
                        "Failed to initialize durable artifact event projection: {error}"
                    ))
                })?;
            let relay_target: Arc<dyn EventTransport> = Arc::new(
                ArtifactEventProjectionTransport::new(artifact_projector, relay_target),
            );
            // The relay target performs generation rotation synchronously. OutboxRelay therefore
            // cannot mark a tenant mutation dispatched until cache invalidation has been published
            // and the exact canonical local listener is ready when Redis is not configured.
            let relay_target = tenant_generation_transport(ctx, &cache, relay_target);
            let relay_policy = &settings.events.relay_retry_policy;
            let max_attempts = if settings.events.dlq.enabled {
                settings.events.dlq.max_attempts
            } else {
                relay_policy.max_attempts
            };
            let relay_config = RelayRuntimeConfig {
                interval: Duration::from_millis(settings.events.relay_interval_ms),
                relay: OutboxRelay::new(ctx.db_clone(), relay_target).with_config(RelayConfig {
                    batch_size: settings.events.relay_batch_size,
                    max_attempts,
                    backoff_base: Duration::from_millis(relay_policy.base_backoff_ms),
                    backoff_max: Duration::from_millis(relay_policy.max_backoff_ms),
                    max_concurrency: settings.events.relay_max_concurrency,
                    claim_ttl: Duration::from_millis(settings.events.relay_claim_ttl_ms),
                    ..RelayConfig::default()
                }),
            };

            let outbox_retention =
                resolve_outbox_retention(ctx, &settings.events.outbox_retention)?;

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

    // Module listeners are started immediately after this function returns, while the historical
    // bootstrap stored EventRuntime only afterwards. Seed the shared runtime here so listener
    // startup always resolves the exact delivery bus paired with the configured transport.
    ctx.shared_insert(Arc::new(runtime.clone()));
    Ok(runtime)
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
    if settings.interval_seconds == 0 {
        return Err(Error::BadRequest(
            "events.outbox_retention.interval_seconds must be greater than zero".to_string(),
        ));
    }

    let retention = rustok_outbox::OutboxRetentionConfig {
        retention: chrono::Duration::days(i64::try_from(settings.retention_days).map_err(
            |_| Error::BadRequest("outbox retention window is out of range".to_string()),
        )?),
        batch_size: settings.batch_size,
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
            inner_tasks.spawn(async move {
                loop {
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
                            message = %error.message,
                            "Relay processing error"
                        );
                    }
                    tokio::time::sleep(interval).await;
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
                        Some(Err(error)) => tracing::error!(
                            "Outbox relay worker failed: {:?}. Restarting in 5s.",
                            error
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
                    tracing::info!("Outbox relay supervisor received shutdown signal, exiting");
                    inner_tasks.abort_all();
                    while inner_tasks.join_next().await.is_some() {}
                    return;
                }
            }
        }
    })
}

fn outbox_relay_worker_context() -> PortContext {
    PortContext::new(
        "platform",
        PortActor::service("rustok-server.outbox-relay-worker"),
        "und",
        format!("outbox-relay-worker:{}", uuid::Uuid::new_v4()),
    )
    .with_idempotency_key(format!("outbox-relay-tick:{}", uuid::Uuid::new_v4()))
    .with_deadline(Duration::from_secs(30))
}

fn transport_with_local_delivery(
    primary: Arc<dyn EventTransport>,
    channel_capacity: usize,
) -> (Arc<dyn EventTransport>, EventBus) {
    let local = MemoryTransport::with_capacity(channel_capacity);
    let listener_bus = local.event_bus();
    let transport = LocalDeliveryFanoutTransport { primary, local };
    (Arc::new(transport), listener_bus)
}

#[derive(Clone)]
struct LocalDeliveryFanoutTransport {
    primary: Arc<dyn EventTransport>,
    local: MemoryTransport,
}

#[async_trait]
impl EventTransport for LocalDeliveryFanoutTransport {
    async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
        // The primary delivery is irreversible. Once it succeeds, returning a local fan-out error
        // would make the outbox relay publish the same remote event again. Record the local failure
        // separately and let durable/idempotent consumers recover through their transport path.
        self.primary.publish(envelope.clone()).await?;
        let event_id = envelope.id;
        let event_type = envelope.event.event_type();
        if let Err(error) = self.local.publish(envelope).await {
            EVENT_LOCAL_DELIVERY_FAILURE_TOTAL.fetch_add(1, Ordering::Relaxed);
            rustok_telemetry::metrics::record_event_error(event_type, "local_delivery");
            tracing::error!(
                event_id = %event_id,
                event_type,
                error = %error,
                "Remote event was accepted but local module delivery failed"
            );
        }
        Ok(())
    }

    async fn publish_contract(&self, envelope: ContractEventEnvelope) -> rustok_core::Result<()> {
        let payload = envelope
            .payload()
            .map_err(|error| rustok_core::Error::Validation(error.to_string()))?;
        let deliver_locally = matches!(payload, ContractEventPayload::Root(_))
            || self.local.event_bus().has_contract_handler(&envelope)?;
        if payload.requires_module_delivery() && !deliver_locally {
            return Err(rustok_core::Error::External(
                "Required sealed module delivery is unavailable; retain the original outbox identity".to_owned(),
            ));
        }
        // The configured Iggy connector must accept the unchanged frame first. A required
        // local projection failure retains this outbox identity for retry, even if that
        // republishes the same frame remotely. Remote consumers must be idempotent.
        self.primary.publish_contract(envelope.clone()).await?;
        if deliver_locally {
            let event_id = envelope.id();
            let event_type = envelope.event_type().to_owned();
            if let Err(error) = self.local.publish_contract(envelope).await {
                EVENT_LOCAL_DELIVERY_FAILURE_TOTAL.fetch_add(1, Ordering::Relaxed);
                rustok_telemetry::metrics::record_event_error(
                    &event_type,
                    "local_contract_delivery",
                );
                tracing::error!(
                    %event_id, %event_type, code = "events.contract_local_delivery_failed",
                    "Sealed local delivery failed; retain the same outbox identity for retry"
                );
                return Err(error);
            }
        }
        Ok(())
    }

    async fn acknowledge(&self, event_id: uuid::Uuid) -> rustok_core::Result<()> {
        self.primary.acknowledge(event_id).await
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
    use rustok_events::{DomainEvent, EventEnvelope};
    use uuid::Uuid;

    #[tokio::test]
    async fn fanout_transport_delivers_only_after_primary_accepts_event() {
        let primary = MemoryTransport::with_capacity(8);
        let mut primary_receiver = primary.subscribe();
        let (transport, listener_bus) = transport_with_local_delivery(Arc::new(primary), 8);
        let mut listener = listener_bus.subscribe();
        let envelope = EventEnvelope::new(
            Uuid::from_u128(1),
            None,
            DomainEvent::TenantUpdated {
                tenant_id: Uuid::from_u128(1),
            },
        );

        transport.publish(envelope.clone()).await.unwrap();
        assert_eq!(primary_receiver.recv().await.unwrap().id, envelope.id);
        assert_eq!(listener.recv().await.unwrap().id, envelope.id);
    }

    #[tokio::test]
    async fn fanout_transport_does_not_deliver_locally_when_primary_rejects() {
        let primary = MemoryTransport::with_capacity(8);
        let (transport, listener_bus) = transport_with_local_delivery(Arc::new(primary), 8);
        let mut listener = listener_bus.subscribe();
        let envelope = EventEnvelope::new(
            Uuid::from_u128(2),
            None,
            DomainEvent::TenantUpdated {
                tenant_id: Uuid::from_u128(2),
            },
        );

        assert!(transport.publish(envelope).await.is_err());
        assert!(
            tokio::time::timeout(Duration::from_millis(10), listener.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn accepted_remote_delivery_is_not_retried_when_local_bus_has_no_receivers() {
        let primary = MemoryTransport::with_capacity(8);
        let mut primary_receiver = primary.subscribe();
        let (transport, _listener_bus) = transport_with_local_delivery(Arc::new(primary), 8);
        let before = event_local_delivery_metrics_snapshot().failure_total;
        let envelope = EventEnvelope::new(
            Uuid::from_u128(3),
            None,
            DomainEvent::TenantUpdated {
                tenant_id: Uuid::from_u128(3),
            },
        );

        transport.publish(envelope.clone()).await.unwrap();
        assert_eq!(primary_receiver.recv().await.unwrap().id, envelope.id);
        assert!(event_local_delivery_metrics_snapshot().failure_total > before);
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
