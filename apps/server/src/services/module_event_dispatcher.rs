use rustok_auth::{
    AuthConfig, AuthLifecycleRuntime, AuthUserBackfillRuntime, OAuthAdminRuntime,
    UserAdminMutationRuntime,
};
use rustok_core::events::{DispatcherConfig, EventDispatcher};
use rustok_core::{EventBus, ModuleEventListenerContext, ModuleRegistry, ModuleRuntimeExtensions};
use rustok_mcp::McpManagementRuntime;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::common::settings::RustokSettings;
use crate::error::{Error, Result};
use crate::services::event_transport_factory::EventRuntime;
use crate::services::server_runtime_context::ServerRuntimeContext;

#[cfg(feature = "mod-blog")]
#[path = "blog_public_comments_snapshot.rs"]
mod blog_public_comments_snapshot;

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
    #[cfg(feature = "commerce-marketplace-financial")]
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

    #[cfg(feature = "mod-pages")]
    {
        let cache = crate::services::cache_runtime::ensure_cache_service(ctx);
        let provider =
            Arc::new(crate::services::pages_cache_invalidation::ServerPagesCachePort::new(&cache));
        enriched.insert(rustok_pages::PagesCacheInvalidationRuntime::new(
            provider.clone(),
        ));
        enriched.insert(rustok_pages::PagesCacheReadRuntime::new(provider));
    }

    #[cfg(feature = "commerce-marketplace-financial")]
    {
        let financial_runtime = ctx
            .shared_get::<rustok_commerce::MarketplaceFinancialRuntime>()
            .unwrap_or_else(|| {
                let runtime =
                    rustok_commerce::MarketplaceFinancialRuntime::in_process(ctx.db_clone());
                ctx.shared_insert(runtime.clone());
                runtime
            });
        let event_bus = crate::services::event_bus::transactional_event_bus_from_context(ctx);
        ctx.shared_insert(event_bus.clone());
        enriched.insert(financial_runtime.clone());
        enriched.insert(event_bus);

        #[cfg(feature = "mod-payment")]
        {
            let observers = ctx
                .shared_get::<rustok_payment::PaymentProviderEventObservers>()
                .unwrap_or_else(|| {
                    let observers =
                        financial_runtime.payment_provider_event_observers(ctx.db_clone());
                    ctx.shared_insert(observers.clone());
                    observers
                });
            enriched.insert(observers);
        }
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

#[cfg(feature = "commerce-marketplace-financial")]
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
    _settings: &RustokSettings,
) -> Result<Arc<ModuleRuntimeExtensions>> {
    let extensions = rustok_distribution::build_runtime_extensions(registry).map_err(|error| {
        Error::Message(format!(
            "module runtime extension initialization failed: {error}"
        ))
    })?;
    Ok(Arc::new(extensions))
}

pub fn build_shared_runtime_extensions_with_host_providers(
    registry: &ModuleRegistry,
    settings: &RustokSettings,
    runtime_ctx: ServerRuntimeContext,
    auth_config: AuthConfig,
) -> Result<Arc<ModuleRuntimeExtensions>> {
    let base = build_shared_runtime_extensions(registry, settings)?;
    let mut extensions = Arc::try_unwrap(base).map_err(|_| {
        Error::Message(
            "module runtime extensions must remain uniquely owned during host provider registration"
                .to_string(),
        )
    })?;
    let db = runtime_ctx.db_clone();

    #[cfg(all(feature = "mod-seo", feature = "mod-media"))]
    if let Some(storage) = runtime_ctx.shared_get::<rustok_storage::StorageRuntime>() {
        let provider: Arc<dyn rustok_media::MediaAssetReadPort> =
            Arc::new(rustok_media::MediaService::new(db.clone(), storage));
        extensions.insert(rustok_seo::SeoMediaAssetReadProvider::new(provider));
    }

    #[cfg(feature = "mod-media")]
    if let Some(storage) = runtime_ctx.shared_get::<rustok_storage::StorageRuntime>() {
        let provider = rustok_media::MediaTranslationTargetProvider::new(Arc::new(
            rustok_media::MediaService::new(db.clone(), storage),
        ));
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Media translation target provider registration failed: {error}"
                ))
            })?;
    }

    #[cfg(feature = "mod-taxonomy")]
    {
        let provider = rustok_taxonomy::TaxonomyTranslationTargetProvider::new(Arc::new(
            rustok_taxonomy::TaxonomyService::new(db.clone()),
        ));
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Taxonomy translation target provider registration failed: {error}"
                ))
            })?;
    }

    #[cfg(feature = "mod-blog")]
    blog_public_comments_snapshot::register(&mut extensions, &runtime_ctx);

    #[cfg(feature = "mod-navigation")]
    {
        let provider = rustok_navigation::NavigationMenuTranslationTargetProvider::new(Arc::new(
            rustok_navigation::MenuService::new(db.clone()),
        ));
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Navigation menu translation target provider registration failed: {error}"
                ))
            })?;
    }

    #[cfg(feature = "mod-pages")]
    {
        let event_bus = rustok_outbox::TransactionalEventBus::new(Arc::new(
            rustok_outbox::OutboxTransport::new(db.clone()),
        ));
        let provider = rustok_pages::PagesMetadataTranslationTargetProvider::new(Arc::new(
            rustok_pages::PageService::new(db.clone(), event_bus),
        ));
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Pages metadata translation target provider registration failed: {error}"
                ))
            })?;
    }

    #[cfg(feature = "mod-product")]
    {
        let event_bus = rustok_outbox::TransactionalEventBus::new(Arc::new(
            rustok_outbox::OutboxTransport::new(db.clone()),
        ));
        let service = Arc::new(rustok_product::CatalogService::new(db.clone(), event_bus));
        let provider = rustok_product::ProductTranslationTargetProvider::new(service.clone());
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Product translation target provider registration failed: {error}"
                ))
            })?;
        let provider = rustok_product::ProductVariantTranslationTargetProvider::new(service);
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Product Variant translation target provider registration failed: {error}"
                ))
            })?;
    }

    #[cfg(feature = "mod-translation")]
    {
        let provider = crate::static_settings_translation_target::StaticSettingsTranslationTargetProvider::new(
            db.clone(),
        );
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Static Settings translation target provider registration failed: {error}"
                ))
            })?;
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

    #[cfg(feature = "commerce-marketplace-financial")]
    {
        let financial_runtime = runtime_ctx
            .shared_get::<rustok_commerce::MarketplaceFinancialRuntime>()
            .unwrap_or_else(|| {
                let runtime = rustok_commerce::MarketplaceFinancialRuntime::in_process(db.clone());
                runtime_ctx.shared_insert(runtime.clone());
                runtime
            });
        extensions.insert(financial_runtime.clone());

        #[cfg(feature = "mod-payment")]
        {
            let observers = runtime_ctx
                .shared_get::<rustok_payment::PaymentProviderEventObservers>()
                .unwrap_or_else(|| {
                    let observers = financial_runtime.payment_provider_event_observers(db.clone());
                    runtime_ctx.shared_insert(observers.clone());
                    observers
                });
            extensions.insert(observers);
        }
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

    #[cfg(all(feature = "mod-notifications", feature = "mod-profiles"))]
    {
        let policy = crate::services::notification_recipient_policy::ServerNotificationRecipientPolicy::compose(
            db.clone(),
            &extensions,
        );
        extensions.insert(policy);
    }

    #[cfg(feature = "mod-forum")]
    {
        #[cfg(feature = "mod-groups")]
        let groups = Some(
            crate::services::forum_audience_group_facts::ServerForumAudienceGroupFactsPort::shared(
                db.clone(),
            ),
        );
        #[cfg(not(feature = "mod-groups"))]
        let groups = None;

        let audience_facts =
            crate::services::forum_audience_facts::ServerForumAudienceFactsPort::shared(
                db.clone(),
                groups,
            );
        let posting_policy_facts = crate::services::forum_posting_policy_facts::ServerForumPostingPolicyFactsComposer::shared(
            db.clone(),
            audience_facts.clone(),
        )
        .map_err(|error| {
            Error::Message(format!(
                "Forum posting policy fact composition failed: {}",
                error.code
            ))
        })?;
        extensions.insert(audience_facts);
        extensions.insert(posting_policy_facts);
    }

    #[cfg(feature = "mod-forum")]
    {
        let recipient_context = crate::services::forum_notification_recipient_context::ServerForumNotificationRecipientContextPort::shared(
            db.clone(),
        );
        extensions.insert(recipient_context);
    }

    #[cfg(feature = "mod-reactions")]
    {
        if rustok_reactions::api::reaction_subject_registry_from_extensions(&extensions).is_none() {
            return Err(Error::Message(
                "Reactions feature is selected but ReactionsModule is missing from ModuleRegistry"
                    .to_string(),
            ));
        }
        let host =
            extensions.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db.clone()));
        rustok_reactions::api::materialize_reaction_subject_adapter_registry(&mut extensions, &host)
            .map_err(|error| {
                Error::Message(format!(
                    "reaction subject provider materialization failed: {error}"
                ))
            })?;
    }

    #[cfg(feature = "mod-moderation")]
    {
        if !registry.contains("moderation") {
            return Err(Error::Message(
                "Moderation feature is selected but ModerationModule is missing from ModuleRegistry"
                    .to_string(),
            ));
        }
        let host =
            extensions.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db.clone()));
        rustok_moderation::materialize_moderation_subject_adapter_registry(&mut extensions, &host)
            .map_err(|error| {
                Error::Message(format!(
                    "moderation subject adapter materialization failed: {error}"
                ))
            })?;
    }

    #[cfg(feature = "mod-notifications")]
    {
        let host =
            extensions.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db.clone()));
        rustok_notifications::api::materialize_notification_source_registry(&mut extensions, &host)
            .map_err(|error| {
                Error::Message(format!(
                    "notification source provider materialization failed: {error}"
                ))
            })?;
    }

    Ok(Arc::new(extensions))
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
