use std::sync::Arc;

use loco_rs::app::AppContext;
use rustok_core::events::{BackpressureConfig, BackpressureController, EventTransport};
use rustok_core::{EventBus, EventConsumerRuntime};
use tokio::task::JoinHandle;

use crate::common::settings::RustokSettings;

pub use rustok_api::loco::SharedTransactionalEventBus;
pub use rustok_api::loco::transactional_event_bus_from_context;

#[derive(Clone)]
pub struct SharedEventBus(pub Arc<EventBus>);

pub struct EventForwarderHandle {
    _handle: JoinHandle<()>,
}

pub fn event_bus_from_context(ctx: &AppContext) -> EventBus {
    if let Some(shared) = ctx.shared_store.get::<SharedEventBus>() {
        return (*shared.0).clone();
    }

    let settings = RustokSettings::from_settings(&ctx.config.settings).ok();
    let bus = Arc::new(build_event_bus(ctx, settings.as_ref()));

    if let Some(transport) = ctx.shared_store.get::<Arc<dyn EventTransport>>() {
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
        ctx.shared_store
            .insert(EventForwarderHandle { _handle: handle });
    } else {
        tracing::warn!(
            "Event transport is not initialized; event bus will operate in local in-memory mode"
        );
    }

    ctx.shared_store.insert(SharedEventBus(bus.clone()));
    (*bus).clone()
}

fn build_event_bus(ctx: &AppContext, settings: Option<&RustokSettings>) -> EventBus {
    let Some(runtime) = ctx
        .shared_store
        .get::<Arc<crate::services::event_transport_factory::EventRuntime>>()
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

