use loco_rs::app::AppContext;
use rustok_core::events::{DispatcherConfig, EventDispatcher};
use rustok_search::SearchIngestionHandler;

pub fn spawn_search_dispatcher(ctx: &AppContext) {
    let bus = crate::services::event_bus::event_bus_from_context(ctx);
    let db = ctx.db.clone();

    let config = DispatcherConfig {
        retry_count: 3,
        retry_delay_ms: 500,
        ..Default::default()
    };
    let mut dispatcher = EventDispatcher::with_config(bus, config);
    dispatcher.register(SearchIngestionHandler::new(db));

    let running = dispatcher.start();

    tokio::spawn(async move {
        if let Err(error) = running.join().await {
            tracing::error!("Search dispatcher task panicked: {:?}", error);
        }
    });
}
