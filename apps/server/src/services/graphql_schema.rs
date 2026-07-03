use std::sync::Arc;

use crate::graphql::rbac_runtime::rbac_graphql_role_writer_from_context;
use crate::graphql::search_rate_limit::search_graphql_rate_limiter_from_context;
use crate::graphql::{build_schema, AppSchema, SharedGraphqlSchema};
use crate::services::app_runtime::module_runtime_extensions_from_ctx;
use crate::services::build_event_hub::build_event_hub_from_context;
use crate::services::event_bus::{event_bus_from_context, transactional_event_bus_from_context};
use crate::services::field_definition_cache::field_definition_cache_from_context;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub fn init_graphql_schema(ctx: &ServerRuntimeContext) -> Arc<AppSchema> {
    if let Some(shared) = ctx.shared_get::<SharedGraphqlSchema>() {
        return shared.0.clone();
    }

    let event_bus = event_bus_from_context(ctx);
    let schema = Arc::new(build_schema(
        ctx.db_clone(),
        event_bus.clone(),
        transactional_event_bus_from_context(ctx),
        build_event_hub_from_context(ctx),
        field_definition_cache_from_context(ctx, event_bus),
        module_runtime_extensions_from_ctx(ctx),
        rbac_graphql_role_writer_from_context(ctx),
        search_graphql_rate_limiter_from_context(ctx),
        #[cfg(feature = "mod-media")]
        storage_from_ctx(ctx),
    ));

    ctx.shared_insert(SharedGraphqlSchema(schema.clone()));

    schema
}

#[cfg(feature = "mod-media")]
fn storage_from_ctx(ctx: &ServerRuntimeContext) -> rustok_storage::StorageService {
    if let Some(storage) = ctx.shared_get::<rustok_storage::StorageService>() {
        return storage;
    }

    let fallback = rustok_storage::StorageService::new(rustok_storage::local::LocalStorage::new(
        std::env::temp_dir().join("rustok-media-fallback"),
        "/media",
    ));
    ctx.shared_insert(fallback.clone());
    fallback
}
