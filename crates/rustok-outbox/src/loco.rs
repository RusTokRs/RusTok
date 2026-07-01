use std::sync::Arc;

use loco_rs::app::AppContext;
use rustok_core::events::EventTransport;

use crate::TransactionalEventBus;

#[derive(Clone)]
pub struct SharedTransactionalEventBus(pub Arc<TransactionalEventBus>);

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
