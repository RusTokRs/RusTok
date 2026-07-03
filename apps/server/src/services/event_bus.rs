use std::sync::Arc;

use rustok_core::events::{BackpressureConfig, BackpressureController, EventTransport};
use rustok_core::{EventBus, EventConsumerRuntime};
use rustok_outbox::TransactionalEventBus;
use tokio::task::JoinHandle;

use crate::common::settings::RustokSettings;
use crate::services::server_runtime_context::ServerRuntimeContext;

#[derive(Clone)]
pub struct SharedEventBus(pub Arc<EventBus>);

pub struct EventForwarderHandle {
    _handle: JoinHandle<()>,
}

pub fn event_bus_from_context(ctx: &ServerRuntimeContext) -> EventBus {
    if let Some(shared) = ctx.shared_get::<SharedEventBus>() {
        return (*shared.0).clone();
    }

    let bus = Arc::new(build_event_bus(ctx, Some(ctx.settings())));

    if let Some(transport) = ctx.shared_get::<Arc<dyn EventTransport>>() {
        let mut receiver = bus.subscribe();
        let consumer_runtime = EventConsumerRuntime::new("server_event_forwarder");
        let handle = tokio::spawn(async move {
            consumer_runtime.restarted("startup");
            loop {
                match receiver.recv().await {
                    Ok(envelope) => {
                        if let Err(error) = transport.publish(envelope).await {
                            tracing::error!("Failed to publish domain event to transport: {error}");
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        consumer_runtime.lagged(skipped);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        consumer_runtime.closed();
                        break;
                    }
                }
            }
        });
        ctx.shared_insert(EventForwarderHandle { _handle: handle });
    } else {
        tracing::warn!(
            "Event transport is not initialized; event bus will operate in local in-memory mode"
        );
    }

    ctx.shared_insert(SharedEventBus(bus.clone()));
    (*bus).clone()
}

pub fn transactional_event_bus_from_context(ctx: &ServerRuntimeContext) -> TransactionalEventBus {
    let transport = ctx
        .shared_get::<Arc<dyn EventTransport>>()
        .expect("Event transport must be initialized before creating TransactionalEventBus");
    TransactionalEventBus::new(transport)
}

fn build_event_bus(ctx: &ServerRuntimeContext, settings: Option<&RustokSettings>) -> EventBus {
    let Some(runtime) =
        ctx.shared_get::<Arc<crate::services::event_transport_factory::EventRuntime>>()
    else {
        return EventBus::default();
    };

    let Some(settings) = settings else {
        tracing::warn!(
            "Rustok settings unavailable while creating EventBus; backpressure disabled"
        );
        return EventBus::with_capacity(runtime.channel_capacity);
    };

    if settings.events.backpressure.enabled {
        let config = &settings.events.backpressure;
        return EventBus::with_backpressure(
            runtime.channel_capacity,
            BackpressureController::new(BackpressureConfig::new(
                config.max_queue_depth,
                config.warning_threshold,
                config.critical_threshold,
            )),
        );
    }

    EventBus::with_capacity(runtime.channel_capacity)
}
