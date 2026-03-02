use std::sync::Arc;

use loco_rs::app::AppContext;
use rustok_core::events::EventTransport;
use rustok_core::EventBus;
use rustok_outbox::TransactionalEventBus;
use tokio::task::JoinHandle;

use crate::services::event_transport_factory::EventRuntime;

#[derive(Clone)]
pub struct SharedEventBus(pub Arc<EventBus>);

#[derive(Clone)]
pub struct SharedTransactionalEventBus(pub Arc<TransactionalEventBus>);

pub struct EventForwarderHandle {
    _handle: JoinHandle<()>,
}

pub fn event_bus_from_context(ctx: &AppContext) -> EventBus {
    if let Some(shared) = ctx.shared_store.get::<SharedEventBus>() {
        return (*shared.0).clone();
    }

    if let Some(runtime) = ctx.shared_store.get::<Arc<EventRuntime>>() {
        let bus = runtime.event_bus.clone();
        ctx.shared_store
            .insert(SharedEventBus(Arc::new(bus.clone())));
        return bus;
    }

    let bus = Arc::new(EventBus::default());

    if let Some(transport) = ctx.shared_store.get::<Arc<dyn EventTransport>>() {
        let mut receiver = bus.subscribe();
        let handle = tokio::spawn(async move {
            while let Ok(envelope) = receiver.recv().await {
                if let Err(error) = transport.publish(envelope).await {
                    tracing::error!("Failed to publish domain event to transport: {error}");
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

pub fn transactional_event_bus_from_context(ctx: &AppContext) -> TransactionalEventBus {
    if let Some(shared) = ctx.shared_store.get::<SharedTransactionalEventBus>() {
        return (*shared.0).clone();
    }

    let transport = ctx.shared_store.get::<Arc<dyn EventTransport>>().expect(
        "Event transport must be initialized before creating TransactionalEventBus. \
         Check app initialization.",
    );

    let bus = TransactionalEventBus::new(transport.clone());
    let shared = Arc::new(bus.clone());
    ctx.shared_store.insert(SharedTransactionalEventBus(shared));
    bus
}
