use rustok_auth::{
    AuthConfig, AuthLifecycleRuntime, AuthUserBackfillRuntime, OAuthAdminRuntime,
    UserAdminMutationRuntime,
};
use rustok_core::events::{DispatcherConfig, EventDispatcher};
use rustok_core::{EventBus, ModuleEventListenerContext, ModuleRegistry, ModuleRuntimeExtensions};
use rustok_index::IndexerRuntimeConfig;
use rustok_mcp::McpManagementRuntime;
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::common::settings::RustokSettings;
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::event_transport_factory::EventRuntime;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub fn spawn_module_event_dispatcher(
    ctx: &ServerRuntimeContext,
    registry: &ModuleRegistry,
    extensions: Arc<ModuleRuntimeExtensions>,
) {
    let extensions = enrich_runtime_extensions_after_event_start(ctx, extensions);
    let bus = ctx
        .shared_get::<Arc<EventRuntime>>()
        .expect("EventRuntime must be initialized before module event listeners")
        .listener_bus
        .clone();
    let db = ctx.db_clone();
    let dispatcher = build_module_event_dispatcher(registry, bus, db, extensions.as_ref());

    #[cfg(feature = "mod-commerce")]
    spawn_paid_order_label_worker_if_enabled(ctx);
    #[cfg(feature = "mod-commerce")]
    spawn_marketplace_financial_worker_if_enabled(ctx);
    #[cfg(feature = "mod-payment")]
    spawn_payment_provider_event_worker_if_enabled(ctx);

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

fn enrich_runtime_extensions_after_event_start(
    ctx: &ServerRuntimeContext,
    extensions: Arc<ModuleRuntimeExtensions>,
) -> Arc<ModuleRuntimeExtensions> {
    let mut enriched = extensions.as_ref().clone();

    #[cfg(feature = "mod-commerce")]
    {
        let financial_runtime = ctx
            .shared_get::<rustok_commerce::MarketplaceFinancialRuntime>()
            .unwrap_or_else(|| {
                let runtime =
                    rustok_commerce::MarketplaceFinancialRuntime::in_process(ctx.db_clone());
                ctx.shared_insert(runtime.clone());
                runtime
            });
        let event_bus = transactional_event_bus_from_context(ctx);
        ctx.shared_insert(event_bus.clone());
        enriched.insert(financial_runtime);
        enriched.insert(event_bus);
    }

    let enriched = Arc::new(enriched);
    ctx.shared_insert(enriched.clone());
    enriched
}

#[cfg(feature = "mod-commerce")]
fn spawn_paid_order_label_worker_if_enabled(ctx: &ServerRuntimeContext) {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<
            crate::services::paid_order_label_worker::PaidOrderCreateLabelWorkerHandle,
        >()
    {
        return;
    }

    ensure_stop_handle(ctx);
    let stop_rx = ctx
        .shared_get::<crate::services::app_lifecycle::StopHandle>()
        .expect("StopHandle must exist before paid-order label worker startup")
        .subscribe();
    ctx.shared_insert(
        crate::services::paid_order_label_worker::spawn_paid_order_create_label_worker(
            ctx.clone(),
            stop_rx,
        ),
    );
}

#[cfg(feature = "mod-commerce")]
fn spawn_marketplace_financial_worker_if_enabled(ctx: &ServerRuntimeContext) {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<
            crate::services::marketplace_financial_worker::MarketplaceFinancialWorkerHandle,
        >()
    {
        return;
    }

    ensure_stop_handle(ctx);
    let stop_rx = ctx
        .shared_get::<crate::services::app_lifecycle::StopHandle>()
        .expect("StopHandle must exist before marketplace financial worker startup")
        .subscribe();
    ctx.shared_insert(
        crate::services::marketplace_financial_worker::spawn_marketplace_financial_worker(
            ctx.clone(),
            stop_rx,
        ),
    );
}

#[cfg(feature = "mod-payment")]
fn spawn_payment_provider_event_worker_if_enabled(ctx: &ServerRuntimeContext) {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<
            crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle,
        >()
    {
        return;
    }

    ensure_stop_handle(ctx);
    let stop_rx = ctx
        .shared_get::<crate::services::app_lifecycle::StopHandle>()
        .expect("StopHandle must exist before payment provider event worker startup")
        .subscribe();
    ctx.shared_insert(
        crate::services::payment_provider_event_worker::spawn_payment_provider_event_worker(
            ctx.clone(),
            stop_rx,
        ),
    );
}

#[cfg(any(feature = "mod-commerce", feature = "mod-payment"))]
fn ensure_stop_handle(ctx: &ServerRuntimeContext) {
    if !ctx.shared_contains::<crate::services::app_lifecycle::StopHandle>() {
        let (stop_handle, _stop_rx) = crate::services::app_lifecycle::StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
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

pub fn build_shared_runtime_extensions_with_host_providers(
    registry: &ModuleRegistry,
    settings: &RustokSettings,
    runtime_ctx: ServerRuntimeContext,
    auth_config: AuthConfig,
) -> Arc<ModuleRuntimeExtensions> {
    let base = build_shared_runtime_extensions(registry, settings);
    let mut extensions = base.as_ref().clone();
    let db = runtime_ctx.db_clone();

    #[cfg(all(feature = "mod-seo", feature = "mod-media"))]
    if let Some(storage) = runtime_ctx.shared_get::<rustok_storage::StorageService>() {
        let provider: Arc<dyn rustok_media::MediaAssetReadPort> =
            Arc::new(rustok_media::MediaService::new(db.clone(), storage));
        extensions.insert(rustok_seo::SeoMediaAssetReadProvider::new(provider));
    }

    #[cfg(feature = "mod-fulfillment")]
    {
        let fulfillment_registry = runtime_ctx
            .shared_get::<rustok_fulfillment::providers::FulfillmentProviderRegistry>()
            .unwrap_or_else(|| {
                let registry = rustok_fulfillment::providers::FulfillmentProviderRegistry::with_manual_provider();
                runtime_ctx.shared_insert(registry.clone());
                registry
            });
        extensions.insert(fulfillment_registry);
    }

    #[cfg(feature = "mod-commerce")]
    {
        let financial_runtime = runtime_ctx
            .shared_get::<rustok_commerce::MarketplaceFinancialRuntime>()
            .unwrap_or_else(|| {
                let runtime =
                    rustok_commerce::MarketplaceFinancialRuntime::in_process(db.clone());
                runtime_ctx.shared_insert(runtime.clone());
                runtime
            });
        extensions.insert(financial_runtime);
    }

    let auth_admin_provider = Arc::new(
        crate::services::auth_admin_mutation_provider::ServerAuthAdminMutationProvider::new(
            db.clone(),
        ),
    );
    let oauth_admin_provider = Arc::new(
        crate::services::oauth_admin_guard::GuardedOAuthAdminProvider::new(
            db.clone(),
            auth_admin_provider.clone(),
        ),
    );
    extensions.insert(OAuthAdminRuntime::new(oauth_admin_provider));
    let user_admin_provider = Arc::new(
        crate::services::user_admin_guard::GuardedUserAdminMutationProvider::new(
            auth_admin_provider,
        ),
    );
    extensions.insert(UserAdminMutationRuntime::new(user_admin_provider));
    let auth_lifecycle_provider = Arc::new(
        crate::services::auth_lifecycle_provider::ServerAuthLifecycleProvider::new(
            runtime_ctx,
            auth_config,
        ),
    );
    extensions.insert(AuthLifecycleRuntime::new(auth_lifecycle_provider.clone()));
    extensions.insert(AuthUserBackfillRuntime::new(auth_lifecycle_provider));
    let mcp_management_provider = Arc::new(
        crate::services::mcp_management_mutation_provider::ServerMcpManagementMutationProvider::new(
            db.clone(),
        ),
    );
    let mcp_management_provider = Arc::new(
        crate::services::mcp_management_guard::GuardedMcpManagementProvider::new(
            db.clone(),
            mcp_management_provider,
        ),
    );
    extensions.insert(McpManagementRuntime::new(mcp_management_provider));

    #[cfg(feature = "mod-notifications")]
    {
        let host = extensions.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db));
        rustok_notifications::api::materialize_notification_source_registry(
            &mut extensions,
            &host,
        )
        .expect("notification source provider factories must materialize uniquely");
    }

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
    use super::{
        build_module_event_dispatcher, build_shared_runtime_extensions,
        build_shared_runtime_extensions_with_host_providers,
    };
    use crate::common::settings::RustokSettings;
    use rustok_auth::AuthConfig;
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

    #[tokio::test]
    async fn host_runtime_extensions_register_admin_mutation_providers() {
        let registry = ModuleRegistry::new();
        let settings = RustokSettings::default();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let runtime_ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(
            db,
            settings.clone(),
        );

        let extensions = build_shared_runtime_extensions_with_host_providers(
            &registry,
            &settings,
            runtime_ctx.clone(),
            AuthConfig::new("test-secret-key-for-unit-tests-only-32bytes!".to_string()),
        );

        assert!(extensions.contains::<rustok_auth::AuthLifecycleRuntime>());
        assert!(extensions.contains::<rustok_auth::AuthUserBackfillRuntime>());
        assert!(extensions.contains::<rustok_auth::OAuthAdminRuntime>());
        assert!(extensions.contains::<rustok_auth::UserAdminMutationRuntime>());
        assert!(extensions.contains::<rustok_mcp::McpManagementRuntime>());
        #[cfg(feature = "mod-notifications")]
        assert!(
            rustok_notifications::api::notification_source_registry_from_extensions(
                extensions.as_ref()
            )
            .is_some()
        );
        #[cfg(feature = "mod-fulfillment")]
        {
            assert!(
                extensions.contains::<rustok_fulfillment::providers::FulfillmentProviderRegistry>()
            );
            assert!(
                runtime_ctx
                    .shared_get::<rustok_fulfillment::providers::FulfillmentProviderRegistry>()
                    .is_some()
            );
        }
        #[cfg(feature = "mod-commerce")]
        assert!(extensions.contains::<rustok_commerce::MarketplaceFinancialRuntime>());
    }
}
