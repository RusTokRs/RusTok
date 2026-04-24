use loco_rs::app::AppContext;
use rustok_core::events::{DispatcherConfig, EventDispatcher};
use rustok_core::{EventBus, ModuleEventListenerContext, ModuleRegistry, ModuleRuntimeExtensions};
use rustok_index::IndexerRuntimeConfig;
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::common::settings::RustokSettings;

pub fn spawn_module_event_dispatcher(
    ctx: &AppContext,
    registry: &ModuleRegistry,
    extensions: Arc<ModuleRuntimeExtensions>,
) {
    let bus = crate::services::event_bus::event_bus_from_context(ctx);
    let db = ctx.db.clone();
    let dispatcher = build_module_event_dispatcher(registry, bus, db, extensions.as_ref());
    let handler_count = dispatcher.handler_count();
    if handler_count == 0 {
        tracing::info!("No module-owned event listeners registered in ModuleRegistry");
        return;
    }

    let running = dispatcher.start();
    tokio::spawn(async move {
        if let Err(error) = running.join().await {
            tracing::error!("Module event dispatcher panicked: {:?}", error);
        }
    });

    tracing::info!(handler_count, "Module event dispatcher initialized");
}

pub fn build_shared_runtime_extensions(
    registry: &ModuleRegistry,
    settings: &RustokSettings,
) -> Arc<ModuleRuntimeExtensions> {
    let mut extensions = registry.build_runtime_extensions();
    let indexer_runtime = IndexerRuntimeConfig::new(
        settings.search.reindex.parallelism,
        settings.search.reindex.entity_budget,
        settings.search.reindex.yield_every,
    );
    metrics::record_index_reindex_runtime_config(
        "content_indexer",
        settings.search.reindex.parallelism,
        settings.search.reindex.entity_budget,
        settings.search.reindex.yield_every,
    );
    metrics::record_index_reindex_runtime_config(
        "product_indexer",
        settings.search.reindex.parallelism,
        settings.search.reindex.entity_budget,
        settings.search.reindex.yield_every,
    );
    metrics::record_index_reindex_runtime_config(
        "flex_indexer",
        settings.search.reindex.parallelism,
        settings.search.reindex.entity_budget,
        settings.search.reindex.yield_every,
    );
    extensions.insert(indexer_runtime);
    Arc::new(extensions)
}

pub fn build_module_event_dispatcher(
    registry: &ModuleRegistry,
    bus: EventBus,
    db: DatabaseConnection,
    extensions: &ModuleRuntimeExtensions,
) -> EventDispatcher {
    let listener_ctx = ModuleEventListenerContext { db, extensions };
    let handlers = registry.build_event_listeners(&listener_ctx);
    let mut dispatcher = EventDispatcher::with_config(
        bus,
        DispatcherConfig {
            retry_count: 3,
            retry_delay_ms: 500,
            ..DispatcherConfig::default()
        },
    );

    for handler in handlers {
        dispatcher.register_boxed(handler);
    }

    dispatcher
}

#[cfg(test)]
mod tests {
    use super::{build_module_event_dispatcher, build_shared_runtime_extensions};
    use crate::common::settings::RustokSettings;
    use rustok_core::{EventBus, ModuleRegistry};
    use rustok_index::IndexModule;
    use rustok_search::SearchModule;
    use sea_orm::Database;

    #[tokio::test]
    async fn build_module_event_dispatcher_collects_registry_owned_handlers() {
        let registry = ModuleRegistry::new()
            .register(IndexModule)
            .register(SearchModule);
        #[cfg(feature = "mod-workflow")]
        let registry = registry.register(rustok_workflow::WorkflowModule);
        let settings = RustokSettings::default();
        let extensions = build_shared_runtime_extensions(&registry, &settings);

        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let dispatcher =
            build_module_event_dispatcher(&registry, EventBus::default(), db, extensions.as_ref());

        let expected = if cfg!(feature = "mod-workflow") { 5 } else { 4 };
        assert_eq!(dispatcher.handler_count(), expected);
    }
}
