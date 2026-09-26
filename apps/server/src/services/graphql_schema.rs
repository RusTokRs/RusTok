use std::sync::Arc;

#[cfg(feature = "mod-blog")]
use crate::graphql::blog_rate_limit::blog_graphql_rate_limiter_from_context;
use crate::graphql::rbac_runtime::rbac_graphql_role_writer_from_context;
use crate::graphql::search_rate_limit::search_graphql_rate_limiter_from_context;
use crate::graphql::{AppSchema, GraphqlSchemaDependencies, SharedGraphqlSchema, build_schema};
use crate::services::app_lifecycle::StopHandle;
use crate::services::app_runtime::module_runtime_extensions_from_ctx;
use crate::services::build_event_hub::build_event_hub_from_context;
use crate::services::commerce_provider_runtime::attach_commerce_provider_registries;
use crate::services::event_bus::{event_bus_from_context, transactional_event_bus_from_context};
use crate::services::field_definition_cache::field_definition_cache_from_context;
use crate::services::profile_media_public_image_runtime::attach_profile_media_public_image_provider;
#[cfg(feature = "mod-seo")]
use crate::services::seo_redirect_cache_reconciliation::start_seo_redirect_cache_reconciliation;
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::static_module_registry::static_module_registry_reader_from_context;
#[cfg(feature = "mod-profiles")]
use rustok_profiles_api::ProfileSummaryReader;

/// Keeps at least one watch receiver alive for API-only hosts so `StopHandle::stop()` can publish
/// the terminal value even when no background worker has subscribed yet.
#[derive(Clone)]
struct IndexReplayStopKeepalive {
    _receiver: tokio::sync::watch::Receiver<bool>,
}

pub fn init_graphql_schema(ctx: &ServerRuntimeContext) -> Arc<AppSchema> {
    #[cfg(feature = "mod-seo")]
    start_seo_redirect_cache_reconciliation(ctx);

    if let Some(shared) = ctx.shared_get::<SharedGraphqlSchema>() {
        return shared.0.clone();
    }

    // Select the public-image provider before any host snapshot is built. The enriched extension
    // registry is stored back in ServerRuntimeContext, so GraphQL and later server-function
    // composition receive the exact same deployment-selected provider wrapper.
    let runtime_extensions =
        attach_profile_media_public_image_provider(ctx, module_runtime_extensions_from_ctx(ctx));
    let event_bus = event_bus_from_context(ctx);
    let transactional_event_bus = transactional_event_bus_from_context(ctx);
    let stop_handle = stop_handle_from_context(ctx);
    let registry = ctx
        .shared_get::<rustok_core::ModuleRegistry>()
        .unwrap_or_else(|| {
            tracing::warn!(
                "ModuleRegistry not initialized before GraphQL schema build; falling back to build_registry()"
            );
            let reg = crate::modules::build_registry();
            ctx.shared_insert(reg.clone());
            reg
        });
    let static_module_registry_reader =
        static_module_registry_reader_from_context(ctx, registry.clone());
    let host_runtime = rustok_api::HostRuntimeContext::new(ctx.db_clone())
        .with_shared_value(transactional_event_bus.clone())
        .with_shared_value(registry);
    let host_runtime = runtime_extensions.apply_to_host_runtime(host_runtime);
    let host_runtime = attach_commerce_provider_registries(host_runtime, ctx);
    let host_runtime =
        if let Some(catalog) = ctx.shared_get::<rustok_modules::SharedModuleMarketplaceCatalog>() {
            host_runtime.with_shared_value(catalog)
        } else {
            host_runtime
        };
    #[cfg(any(feature = "mod-media", feature = "mod-translation"))]
    let host_runtime = attach_storage_runtime(host_runtime, ctx);
    #[cfg(all(feature = "mod-forum", feature = "mod-media"))]
    let host_runtime = attach_forum_media_asset_read_provider(host_runtime, ctx);
    #[cfg(feature = "mod-alloy")]
    let host_runtime = if let Some(alloy_runtime) = ctx.shared_get::<alloy::SharedAlloyRuntime>() {
        let storage = ctx.shared_get::<rustok_storage::StorageRuntime>();
        let host_runtime = host_runtime.with_shared_value(alloy_runtime);
        let host_runtime = host_runtime.with_shared_value(
            crate::services::registry_governance::alloy_release_governance_handle(ctx.db_clone()),
        );
        if let Some(storage) = storage {
            host_runtime.with_shared_value(
                crate::services::registry_governance::alloy_published_rhai_source_provider_handle(
                    ctx.db_clone(),
                    storage,
                ),
            )
        } else {
            tracing::warn!(
                "Alloy published-release import requires initialized durable storage; omitting Rhai source provider handle"
            );
            host_runtime
        }
    } else {
        host_runtime
    };
    #[cfg(feature = "mod-profiles")]
    let host_runtime = host_runtime.with_shared_value(Arc::new(
        rustok_profiles::ProfilePresentationService::new(ctx.db_clone()),
    ) as Arc<dyn ProfileSummaryReader>);

    let graphql_runtime_inputs = rustok_api::graphql::GraphqlRuntimeInputs::new(host_runtime);
    let schema = Arc::new(build_schema(GraphqlSchemaDependencies {
        db: ctx.db_clone(),
        event_bus: event_bus.clone(),
        transactional_event_bus,
        graphql_runtime_inputs,
        static_module_registry_reader,
        build_event_hub: build_event_hub_from_context(ctx),
        field_definition_cache: field_definition_cache_from_context(ctx, event_bus),
        runtime_extensions,
        stop_handle,
        rbac_role_writer: rbac_graphql_role_writer_from_context(ctx),
        search_rate_limiter: search_graphql_rate_limiter_from_context(ctx),
        #[cfg(feature = "mod-blog")]
        blog_rate_limiter: blog_graphql_rate_limiter_from_context(ctx),
        #[cfg(feature = "mod-alloy")]
        alloy_runtime: alloy_runtime_from_ctx(ctx),
        #[cfg(feature = "mod-alloy")]
        alloy_release_governance: alloy_release_governance_from_ctx(ctx),
        #[cfg(feature = "mod-alloy")]
        alloy_published_rhai_source: alloy_published_rhai_source_from_ctx(ctx),
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

#[cfg(any(feature = "mod-media", feature = "mod-translation"))]
fn attach_storage_runtime(
    host_runtime: rustok_api::HostRuntimeContext,
    ctx: &ServerRuntimeContext,
) -> rustok_api::HostRuntimeContext {
    if let Some(storage) = ctx.shared_get::<rustok_storage::StorageRuntime>() {
        host_runtime.with_shared_value(storage)
    } else {
        host_runtime
    }
}

fn stop_handle_from_context(ctx: &ServerRuntimeContext) -> StopHandle {
    let handle = StopHandle::ensure(ctx);
    ctx.shared_insert_if_absent(IndexReplayStopKeepalive {
        _receiver: handle.subscribe(),
    });
    handle
}

#[cfg(feature = "mod-alloy")]
fn alloy_runtime_from_ctx(ctx: &ServerRuntimeContext) -> alloy::SharedAlloyRuntime {
    if let Some(runtime) = ctx.shared_get::<alloy::SharedAlloyRuntime>() {
        return runtime;
    }
    tracing::warn!(
        "SharedAlloyRuntime not found in ServerRuntimeContext; creating minimal fallback"
    );
    let executors = rustok_sandbox::ExecutorRegistry::new();
    let sandbox = rustok_sandbox::SandboxRuntime::new(
        executors,
        Arc::new(rustok_sandbox::CapabilityBrokerRouter::new()),
    );
    let draft_runtime =
        alloy::AlloyDraftRuntime::new(sandbox, rustok_sandbox::SandboxPolicy::default());
    let runtime =
        alloy::SharedAlloyRuntime(alloy::build_alloy_runtime(ctx.db_clone(), draft_runtime));
    ctx.shared_insert(runtime.clone());
    runtime
}

#[cfg(feature = "mod-alloy")]
fn alloy_release_governance_from_ctx(
    ctx: &ServerRuntimeContext,
) -> alloy::AlloyReleaseGovernanceHandle {
    crate::services::registry_governance::alloy_release_governance_handle(ctx.db_clone())
}

#[cfg(feature = "mod-alloy")]
fn alloy_published_rhai_source_from_ctx(
    ctx: &ServerRuntimeContext,
) -> alloy::AlloyPublishedRhaiSourceProviderHandle {
    let storage = ctx
        .shared_get::<rustok_storage::StorageRuntime>()
        .unwrap_or_else(|| {
            tracing::warn!(
                "Alloy published-release import requires initialized durable storage; falling back to in-memory storage runtime"
            );
            let fallback = rustok_storage::StorageRuntime::in_memory();
            ctx.shared_insert(fallback.clone());
            fallback
        });
    crate::services::registry_governance::alloy_published_rhai_source_provider_handle(
        ctx.db_clone(),
        storage,
    )
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
    if let Some(service) =
        ctx.shared_get::<rustok_content_orchestration::SharedContentOrchestrationService>()
    {
        return service;
    }
    tracing::warn!(
        "ContentOrchestrationService not initialized; building fallback service for GraphQL schema dependencies"
    );
    let service = rustok_content_orchestration::build_content_orchestration_service(
        ctx.db_clone(),
        transactional_event_bus_from_context(ctx),
    );
    ctx.shared_insert(service.clone());
    service
}

#[cfg(all(feature = "mod-forum", feature = "mod-media"))]
fn attach_forum_media_asset_read_provider(
    host_runtime: rustok_api::HostRuntimeContext,
    ctx: &ServerRuntimeContext,
) -> rustok_api::HostRuntimeContext {
    use rustok_media::{MediaAssetReadPort, MediaService};
    use rustok_storage::StorageRuntime;

    if let Some(provider) = host_runtime.shared_get::<Arc<dyn MediaAssetReadPort>>() {
        ctx.shared_insert(provider);
        return host_runtime;
    }

    if let Some(provider) = ctx.shared_get::<Arc<dyn MediaAssetReadPort>>() {
        return host_runtime.with_shared_value(provider);
    }

    let Some(storage) = ctx.shared_get::<StorageRuntime>() else {
        tracing::warn!(
            "Forum attachment reconciliation Media provider is unavailable; GraphQL entrypoint will fail closed"
        );
        return host_runtime;
    };

    let provider: Arc<dyn MediaAssetReadPort> =
        Arc::new(MediaService::new(ctx.db_clone(), storage));
    ctx.shared_insert(provider.clone());

    host_runtime.with_shared_value(provider)
}

#[cfg(all(test, feature = "mod-forum", feature = "mod-media"))]
mod forum_media_provider_composition_tests {
    use super::attach_forum_media_asset_read_provider;
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use rustok_api::HostRuntimeContext;
    use rustok_core::ModuleRuntimeExtensions;
    use rustok_media::{MediaAssetReadPort, MediaService};
    use rustok_storage::{LocalStorageConfig, StorageRuntime};
    use sea_orm::Database;
    use std::sync::Arc;

    #[tokio::test]
    async fn host_published_media_provider_wins_over_embedded_construction() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let storage = StorageRuntime::local(&LocalStorageConfig {
            base_dir: std::env::temp_dir()
                .join(format!(
                    "rustok-forum-media-provider-{}",
                    uuid::Uuid::new_v4()
                ))
                .display()
                .to_string(),
            base_url: String::new(),
            fsync: false,
        })
        .expect("local storage should initialize");
        let remote_like: Arc<dyn MediaAssetReadPort> =
            Arc::new(MediaService::new(db.clone(), storage));
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(remote_like.clone());

        let host = extensions.apply_to_host_runtime(HostRuntimeContext::new(db));
        let resolved = attach_forum_media_asset_read_provider(host, &ctx);
        let selected = resolved
            .shared_get::<Arc<dyn MediaAssetReadPort>>()
            .expect("host-published provider should remain selected");

        assert!(Arc::ptr_eq(&remote_like, &selected));
    }
}

#[cfg(feature = "mod-media")]
fn storage_from_ctx(ctx: &ServerRuntimeContext) -> rustok_storage::StorageRuntime {
    if let Some(storage) = ctx.shared_get::<rustok_storage::StorageRuntime>() {
        return storage;
    }

    let fallback = rustok_storage::StorageRuntime::local(&rustok_storage::LocalStorageConfig {
        base_dir: std::env::temp_dir()
            .join("rustok-media-fallback")
            .to_string_lossy()
            .into_owned(),
        base_url: "/media".to_string(),
        fsync: false,
    })
    .unwrap_or_else(|err| {
        tracing::warn!(
            "Failed to create fallback local storage runtime ({err}); falling back to in-memory store"
        );
        rustok_storage::StorageRuntime::in_memory()
    });
    ctx.shared_insert(fallback.clone());
    fallback
}

#[cfg(all(test, feature = "mod-translation"))]
mod translation_storage_tests {
    use super::attach_storage_runtime;
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use rustok_api::HostRuntimeContext;
    use sea_orm::Database;

    #[tokio::test]
    async fn translation_graphql_host_receives_initialized_storage_runtime() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect Translation GraphQL storage evidence database");
        let ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let storage = rustok_storage::StorageRuntime::local(&rustok_storage::LocalStorageConfig {
            base_dir: std::env::temp_dir()
                .join(format!(
                    "rustok-translation-graphql-storage-{}",
                    uuid::Uuid::new_v4()
                ))
                .to_string_lossy()
                .into_owned(),
            base_url: "/translation-artifacts".to_string(),
            fsync: false,
        })
        .expect("create Translation GraphQL storage runtime");
        ctx.shared_insert(storage);

        let host_runtime = attach_storage_runtime(HostRuntimeContext::new(db), &ctx);

        assert!(
            host_runtime
                .shared_get::<rustok_storage::StorageRuntime>()
                .is_some(),
            "mod-translation GraphQL host must receive the initialized StorageRuntime"
        );
    }
}
