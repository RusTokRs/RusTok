use std::sync::Arc;

use loco_rs::app::AppContext;

use crate::graphql::{build_schema, AppSchema, SharedGraphqlSchema};
use crate::services::app_runtime::module_runtime_extensions_from_ctx;
use crate::services::build_event_hub::build_event_hub_from_context;
use crate::services::event_bus::{event_bus_from_context, transactional_event_bus_from_context};
use crate::services::field_definition_cache::field_definition_cache_from_context;

pub fn init_graphql_schema(ctx: &AppContext) -> Arc<AppSchema> {
    if let Some(shared) = ctx.shared_store.get::<SharedGraphqlSchema>() {
        return shared.0.clone();
    }

    let event_bus = event_bus_from_context(ctx);
    let schema = Arc::new(build_schema(
        ctx.db.clone(),
        event_bus.clone(),
        transactional_event_bus_from_context(ctx),
        build_event_hub_from_context(ctx),
        field_definition_cache_from_context(ctx, event_bus),
        module_runtime_extensions_from_ctx(ctx),
        #[cfg(feature = "mod-media")]
        storage_from_ctx(ctx),
    ));

    ctx.shared_store.insert(SharedGraphqlSchema(schema.clone()));

    schema
}

#[cfg(feature = "mod-media")]
fn storage_from_ctx(ctx: &AppContext) -> rustok_storage::StorageService {
    ctx.shared_store
        .get::<rustok_storage::StorageService>()
        .expect("StorageService not initialised — call init_storage() before init_graphql_schema()")
}
