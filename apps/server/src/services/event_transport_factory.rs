use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::error::{Error, Result};
use async_trait::async_trait;
use rustok_api::{PortActor, PortContext};
use rustok_cache::CacheService;
use rustok_core::events::{
    EventBus, EventEnvelope, EventTransport, MemoryTransport, ReliabilityLevel,
};
use rustok_iggy::{IggyConfig, IggyTransport};
use rustok_modules::{
    ArtifactEventDeliveryConfig, ArtifactEventProjectionTransport, ModuleControlPlane,
};
use rustok_outbox::{
    OutboxRelay, OutboxRelayPort, OutboxRelayRunOnceRequest, OutboxTransport, RelayConfig,
};
use tokio::task::JoinHandle;

use crate::common::settings::{EventTransportKind, RelayTargetKind, RustokSettings};
use crate::services::rbac_cache_invalidation::start_rbac_cache_invalidation_listener;
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::tenant_cache_generation::{
    start_tenant_cache_generation_listener, TenantCacheGenerationTransport,
};
use crate::services::tenant_generation_delivery_gate::TenantGenerationDeliveryGate;

static OUTBOX_RELAY_SUPERVISOR_RESTART_TOTAL: AtomicU64 = AtomicU64::new(0);
static EVENT_LOCAL_DELIVERY_FAILURE_TOTAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct EventRuntime {
    pub transport: Arc<dyn EventTransport>,
    /// Events accepted by the configured transport are delivered to module listeners on this bus.
    /// This is deliberately separate from the outbound publisher bus to avoid relay-to-outbox loops.
    pub listener_bus: EventBus,
    pub relay_config: Option<RelayRuntimeConfig>,
    pub channel_capacity: usize,
    pub relay_fallback_active: bool,
}

#[derive(Clone)]
pub struct RelayRuntimeConfig {
    pub interval: Duration,
    pub relay: OutboxRelay,
}

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
    let channel_capacity = settings.events.channel_capacity;
    let cache = ctx.shared_get::<CacheService>().ok_or_else(|| {
        Error::BadRequest("CacheService must be initialized before the event runtime".to_string())
    })?;

    // Subscribe before any transport can publish a generation. This also restores shared
    // generations before middleware and authorization paths construct or read their caches.
    start_tenant_cache_generation_listener(ctx, cache.clone())
        .await
        .map_err(|error| Error::Cache(error.to_string()))?;
    start_rbac_cache_invalidation_listener(ctx, cache.clone()).await?;

    let runtime = match settings.events.transport {
        EventTransportKind::Memory => {
            let transport = MemoryTransport::with_capacity(channel_capacity);
            let listener_bus = transport.event_bus();
            let transport = tenant_generation_transport(ctx, &cache, Arc::new(transport));
            EventRuntime {
                transport,
                listener_bus,
                relay_config: None,
                channel_capacity,
                relay_fallback_active: false,
            }
        }
        EventTransportKind::Outbox => {
            // Keep the application-facing transport concrete so TransactionalEventBus can
            // downcast to OutboxTransport and write into the caller's database transaction.
            let outbox_transport = Arc::new(OutboxTransport::new(ctx.db_clone()));
            let (relay_target, listener_bus, relay_fallback_active) =
                resolve_relay_target(settings, channel_capacity).await?;
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

            EventRuntime {
                transport: outbox_transport,
                listener_bus,
                relay_config: Some(relay_config),
                channel_capacity,
                relay_fallback_active,
            }
        }
        EventTransportKind::Iggy => {
            let primary: Arc<dyn EventTransport> = Arc::new(
                IggyTransport::new(resolve_iggy_config(settings))
                    .await
                    .map_err(|error| {
                        Error::BadRequest(format!("Failed to initialize iggy transport: {error}"))
                    })?,
            );
            let primary = tenant_generation_transport(ctx, &cache, primary);
            let (transport, listener_bus) =
                transport_with_local_delivery(primary, channel_capacity);
            EventRuntime {
                transport,
                listener_bus,
                relay_config: None,
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

            let mut inner_handle = tokio::spawn(async move {
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
                result = &mut inner_handle => {
                    if *stop_rx.borrow() {
                        tracing::info!("Outbox relay supervisor received shutdown signal, exiting");
                        return;
                    }
                    OUTBOX_RELAY_SUPERVISOR_RESTART_TOTAL.fetch_add(1, Ordering::Relaxed);
                    if let Err(panic) = result {
                        tracing::error!(
                            "Outbox relay worker panicked: {:?}. Restarting in 5s.",
                            panic
                        );
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                            _ = stop_rx.changed() => {
                                tracing::info!(
                                    "Outbox relay supervisor received shutdown signal during restart delay, exiting"
                                );
                                return;
                            }
                        }
                    } else {
                        tracing::warn!("Outbox relay worker exited unexpectedly. Restarting.");
                    }
                }
                _ = stop_rx.changed() => {
                    tracing::info!("Outbox relay supervisor received shutdown signal, exiting");
                    inner_handle.abort();
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

fn resolve_iggy_config(settings: &RustokSettings) -> IggyConfig {
    settings.events.iggy.clone()
}

async fn resolve_relay_target(
    settings: &RustokSettings,
    channel_capacity: usize,
) -> Result<(Arc<dyn EventTransport>, EventBus, bool)> {
    match settings.events.relay_target {
        RelayTargetKind::Memory => {
            let transport = MemoryTransport::with_capacity(channel_capacity);
            let listener_bus = transport.event_bus();
            Ok((Arc::new(transport), listener_bus, false))
        }
        RelayTargetKind::Iggy => match IggyTransport::new(resolve_iggy_config(settings)).await {
            Ok(transport) => {
                let (target, listener_bus) =
                    transport_with_local_delivery(Arc::new(transport), channel_capacity);
                Ok((target, listener_bus, false))
            }
            Err(error) => {
                if settings.events.allow_relay_target_fallback {
                    tracing::warn!(
                        error = %error,
                        "Failed to initialize relay_target=iggy, fallback to memory due to explicit opt-in"
                    );
                    let transport = MemoryTransport::with_capacity(channel_capacity);
                    let listener_bus = transport.event_bus();
                    Ok((Arc::new(transport), listener_bus, true))
                } else {
                    Err(Error::BadRequest(format!(
                        "Failed to initialize relay_target=iggy and fallback is disabled: {error}"
                    )))
                }
            }
        },
    }
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
