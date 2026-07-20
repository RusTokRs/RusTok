use std::sync::Arc;

use crate::graphql::rbac_runtime::rbac_graphql_role_writer_from_context;
use crate::graphql::search_rate_limit::search_graphql_rate_limiter_from_context;
use crate::graphql::{build_schema, AppSchema, GraphqlSchemaDependencies, SharedGraphqlSchema};
use crate::services::app_runtime::module_runtime_extensions_from_ctx;
use crate::services::build_event_hub::build_event_hub_from_context;
use crate::services::commerce_provider_runtime::attach_commerce_provider_registries;
use crate::services::event_bus::{event_bus_from_context, transactional_event_bus_from_context};
use crate::services::field_definition_cache::field_definition_cache_from_context;
#[cfg(feature = "mod-seo")]
use crate::services::seo_redirect_cache_reconciliation::start_seo_redirect_cache_reconciliation;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub fn init_graphql_schema(ctx: &ServerRuntimeContext) -> Arc<AppSchema> {
    #[cfg(feature = "mod-seo")]
    start_seo_redirect_cache_reconciliation(ctx);

    if let Some(shared) = ctx.shared_get::<SharedGraphqlSchema>() {
        return shared.0.clone();
    }

    let event_bus = event_bus_from_context(ctx);
    let transactional_event_bus = transactional_event_bus_from_context(ctx);
    let registry = ctx
        .shared_get::<rustok_core::ModuleRegistry>()
        .expect("ModuleRegistry not initialized; bootstrap_app_runtime must run first");
    let host_runtime = rustok_api::HostRuntimeContext::new(ctx.db_clone())
        .with_shared_value(transactional_event_bus.clone())
        .with_shared_value(registry);
    let host_runtime = module_runtime_extensions_from_ctx(ctx).apply_to_host_runtime(host_runtime);
    let host_runtime = attach_commerce_provider_registries(host_runtime, ctx);
    #[cfg(feature = "mod-media")]
    let host_runtime = if let Some(storage) = ctx.shared_get::<rustok_storage::StorageService>() {
        host_runtime.with_shared_value(storage)
    } else {
        host_runtime
    };
    #[cfg(feature = "mod-alloy")]
    let host_runtime = if let Some(alloy_runtime) = ctx.shared_get::<alloy::SharedAlloyRuntime>() {
        let host_runtime = host_runtime.with_shared_value(alloy_runtime);
        host_runtime.with_shared_value(
            crate::services::registry_governance::alloy_release_governance_handle(ctx.db_clone()),
        )
    } else {
        host_runtime
    };
    let graphql_runtime_inputs = rustok_api::graphql::GraphqlRuntimeInputs::new(host_runtime);
    let schema = Arc::new(build_schema(GraphqlSchemaDependencies {
        db: ctx.db_clone(),
        event_bus: event_bus.clone(),
        transactional_event_bus,
        graphql_runtime_inputs,
        build_event_hub: build_event_hub_from_context(ctx),
        field_definition_cache: field_definition_cache_from_context(ctx, event_bus),
        runtime_extensions: module_runtime_extensions_from_ctx(ctx),
        rbac_role_writer: rbac_graphql_role_writer_from_context(ctx),
        search_rate_limiter: search_graphql_rate_limiter_from_context(ctx),
        #[cfg(feature = "mod-alloy")]
        alloy_runtime: alloy_runtime_from_ctx(ctx),
        #[cfg(feature = "mod-alloy")]
        alloy_release_governance: alloy_release_governance_from_ctx(ctx),
        #[cfg(all(
            feature = "mod-content",
            feature = "mod-blog",
            feature = "mod-forum",
            feature = "mod-comments"
        ))]
        content_orchestration: content_orchestration_from_ctx(ctx),
        #[cfg(feature = "mod-media")]
        storage: storage_from_ctx(ctx),
    }));

    ctx.shared_insert(SharedGraphqlSchema(schema.clone()));

    schema
}

#[cfg(feature = "mod-alloy")]
fn alloy_runtime_from_ctx(ctx: &ServerRuntimeContext) -> alloy::SharedAlloyRuntime {
    ctx.shared_get::<alloy::SharedAlloyRuntime>()
        .expect("Alloy runtime not initialized; bootstrap_app_runtime must run first")
}

#[cfg(feature = "mod-alloy")]
fn alloy_release_governance_from_ctx(
    ctx: &ServerRuntimeContext,
) -> alloy::AlloyReleaseGovernanceHandle {
    crate::services::registry_governance::alloy_release_governance_handle(ctx.db_clone())
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn content_orchestration_from_ctx(
    ctx: &ServerRuntimeContext,
) -> rustok_content_orchestration::SharedContentOrchestrationService {
    ctx.shared_get::<rustok_content_orchestration::SharedContentOrchestrationService>()
        .expect("ContentOrchestrationService not initialized; bootstrap_app_runtime must run first")
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
