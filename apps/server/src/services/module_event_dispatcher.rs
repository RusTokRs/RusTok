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
use crate::services::server_runtime_context::{ServerRuntimeContext, ServerRuntimeStartReservation};

#[cfg(feature = "mod-blog")]
#[path = "blog_public_comments_snapshot.rs"]
mod blog_public_comments_snapshot;

/// Whether this worker host requires a live module event dispatcher. Even an
/// empty module listener set must bind an explicit registry so the transport can
/// distinguish "no local handler" from "listener registry not initialized".
#[derive(Clone, Copy)]
pub struct ModuleEventDispatcherRequirement(pub bool);

/// Host-owned lifecycle handle for the module event dispatcher.
///
/// The dispatcher is a background consumer: API/SSR/registry profiles compose
/// the same module registry but must not start local event-loop workers. When
/// started, the handle is stored in the runtime context so host shutdown can
/// request a cooperative drain and join the receive loop instead of leaking it
/// past the server. Abort remains the emergency fallback after the host drain
/// deadline.
// A separate type per worker makes registration atomic without requiring a
// Clone implementation for a JoinHandle or publishing a provisional handle.
#[cfg(feature = "mod-commerce")]
struct PaidOrderLabelWorkerReservation;
#[cfg(feature = "commerce-marketplace-financial")]
struct MarketplaceFinancialWorkerReservation;
#[cfg(feature = "mod-payment")]
struct PaymentProviderEventWorkerReservation;
struct ModuleEventDispatcherReservation;

pub struct ModuleEventDispatcherHandle {
    running: rustok_core::RunningDispatcher,
}

impl ModuleEventDispatcherHandle {
    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self {
            running: EventDispatcher::new(EventBus::new()).start(),
        }
    }

    pub fn is_finished(&self) -> bool {
        self.running.is_finished()
    }

    pub fn request_stop(&self) {
        self.running.request_stop();
    }

    pub fn abort(&self) {
        self.running.abort();
    }

    pub fn abort_handle(&self) -> tokio::task::AbortHandle {
        self.running.abort_handle()
    }

    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self.running.join().await
    }

    pub(crate) fn into_task(self) -> tokio::task::JoinHandle<()> {
        self.running.into_join_handle()
    }
}

fn module_event_dispatcher_enabled(settings: &RustokSettings) -> bool {
    settings.runtime.runs_background_workers()
}

/// A published handle alone is not readiness: the listener requirement
/// must also have been registered, and the dispatcher loop must still run.
fn existing_module_dispatcher_status(ctx: &ServerRuntimeContext) -> Option<Result<()>> {
    let running = ctx.shared_map::<ModuleEventDispatcherHandle, _>(|handle| !handle.is_finished())?;
    let required = ctx
        .shared_get::<ModuleEventDispatcherRequirement>()
        .is_some_and(|requirement| requirement.0);
    Some(if running && required {
        Ok(())
    } else {
        Err(Error::Message(
            "module event dispatcher exists but is not ready or has terminated".to_string(),
        ))
    })
}

/// Distinguish a dispatcher already ready in the runtime from one still
/// being built by another caller. An in-flight reservation is *not* readiness:
/// the owner can still fail before it registers its handle and listener flag.
fn claim_module_dispatcher_start(
    ctx: &ServerRuntimeContext,
) -> Result<Option<ServerRuntimeStartReservation<'_, ModuleEventDispatcherReservation>>> {
    if crate::services::app_lifecycle::StopHandle::ensure(ctx).is_stopping() {
        return Err(Error::Message(
            "server shutdown started before module dispatcher initialization".to_string(),
        ));
    }
    if let Some(existing) = existing_module_dispatcher_status(ctx) {
        existing?;
        return Ok(None);
    }

    let Some(claim) =
        ServerRuntimeStartReservation::try_claim(ctx, ModuleEventDispatcherReservation)
    else {
        // The first starter may have published its handle between our
        // preflight and the failed atomic claim. Its ready listener flag must
        // also exist before another caller may report successful startup.
        if let Some(existing) = existing_module_dispatcher_status(ctx) {
            existing?;
            return Ok(None);
        }
        return Err(Error::Message(
            "module event dispatcher startup is already in progress; retry".to_string(),
        ));
    };
    Ok(Some(claim))
}

pub fn spawn_module_event_dispatcher(
    ctx: &ServerRuntimeContext,
    registry: &ModuleRegistry,
    extensions: Arc<ModuleRuntimeExtensions>,
) -> Result<()> {
    tracing::debug!("Spawning module event dispatcher");
    // Preflight the required event runtime before starting any module-owned
    // worker. Previously an absent EventRuntime returned an error only after
    // payment/commerce/media recovery tasks had already been spawned.
    let event_bus = if module_event_dispatcher_enabled(ctx.settings()) {
        Some(
            ctx.shared_get::<Arc<EventRuntime>>()
                .ok_or_else(|| {
                    Error::Message(
                        "EventRuntime must be initialized before module event listeners"
                            .to_owned(),
                    )
                })?
                .listener_bus
                .clone(),
        )
    } else {
        None
    };

    // Claim ownership before enriching the runtime or registering listeners.
    // The guard releases the claim on errors, unwinds and abandoned startups.
    // A concurrent *in-progress* attempt must never be reported as success.
    let reservation = if event_bus.is_some() {
        match claim_module_dispatcher_start(ctx)? {
            Some(reservation) => Some(reservation),
            None => {
                tracing::debug!("Module event dispatcher is already registered");
                return Ok(());
            }
        }
    } else {
        None
    };

    let extensions = enrich_runtime_extensions_after_event_start(ctx, extensions);

    let Some(event_bus) = event_bus else {
        tracing::info!(
            host_mode = ?ctx.settings().runtime.host_mode,
            "Module event dispatcher disabled for the current runtime profile"
        );
        return Ok(());
    };
    let db = ctx.db_clone();
    let dispatcher = build_module_event_dispatcher(registry, event_bus, db, extensions.as_ref());

    let handler_count = dispatcher.handler_count();
    if handler_count == 0 {
        tracing::info!(
            "No module-owned event listeners registered; binding an explicit empty registry for transport routing"
        );
    }

    if crate::services::app_lifecycle::StopHandle::ensure(ctx).is_stopping() {
        return Err(Error::Message(
            "server shutdown started during module dispatcher initialization".to_string(),
        ));
    }
    // Publication is the commit boundary for local startup ownership.
    // On any earlier failure, the RAII guard automatically permits retry.
    start_module_event_dispatcher(ctx, dispatcher, handler_count)?;
    if let Some(reservation) = reservation {
        reservation.commit();
    }

    // Required dispatcher startup has succeeded. Optional recovery workers may
    // now begin their own loops; a failed dispatcher start leaves none behind.
    #[cfg(feature = "mod-product")]
    spawn_product_media_release_worker_if_enabled(ctx);
    #[cfg(feature = "mod-commerce")]
    spawn_paid_order_label_worker_if_enabled(ctx);
    #[cfg(feature = "commerce-marketplace-financial")]
    spawn_marketplace_financial_worker_if_enabled(ctx);
    #[cfg(feature = "mod-payment")]
    spawn_payment_provider_event_worker_if_enabled(ctx);
    Ok(())
}

fn start_module_event_dispatcher(
    ctx: &ServerRuntimeContext,
    dispatcher: EventDispatcher,
    handler_count: usize,
) -> Result<()> {
    // The listener registry is part of the transport contract even when it is
    // intentionally empty: remote-only sealed events may proceed, while
    // required local families still fail closed when no handler is registered.
    let running = dispatcher.start_with_contract_delivery()?;
    // Publish the required-listener flag only after the dispatcher actually
    // starts; a failed start is not a running/healthy listener registry.
    ctx.shared_insert(ModuleEventDispatcherHandle { running });
    ctx.shared_insert(ModuleEventDispatcherRequirement(true));
    tracing::info!(handler_count, "Module event dispatcher initialized");
    Ok(())
}

fn enrich_runtime_extensions_after_event_start(
    ctx: &ServerRuntimeContext,
    extensions: Arc<ModuleRuntimeExtensions>,
) -> Arc<ModuleRuntimeExtensions> {
    #[cfg(any(feature = "mod-pages", feature = "commerce-marketplace-financial"))]
    let mut enriched = extensions.as_ref().clone();
    #[cfg(not(any(feature = "mod-pages", feature = "commerce-marketplace-financial")))]
    let enriched = extensions.as_ref().clone();

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
    let stop = crate::services::app_lifecycle::StopHandle::ensure(ctx);
    if !ctx.settings().runtime.runs_background_workers()
        || stop.is_stopping()
        || ctx.shared_contains::<
            crate::services::paid_order_label_worker::PaidOrderCreateLabelWorkerHandle,
        >()
    {
        return;
    }
    let Some(reservation) =
        ServerRuntimeStartReservation::try_claim(ctx, PaidOrderLabelWorkerReservation)
    else {
        return;
    };
    if stop.is_stopping() {
        return;
    }

    if let Some(handle) =
        crate::services::paid_order_label_worker::spawn_paid_order_create_label_worker(
            ctx.clone(),
            stop.subscribe(),
        )
    {
        ctx.shared_insert(handle);
        reservation.commit();
    }
    // A missing registry or an unwinding/cancelled start releases the claim
    // through Drop; a later composition may retry.
}

#[cfg(feature = "commerce-marketplace-financial")]
fn spawn_marketplace_financial_worker_if_enabled(ctx: &ServerRuntimeContext) {
    let stop = crate::services::app_lifecycle::StopHandle::ensure(ctx);
    if !ctx.settings().runtime.runs_background_workers()
        || stop.is_stopping()
        || ctx.shared_contains::<
            crate::services::marketplace_financial_worker::MarketplaceFinancialWorkerHandle,
        >()
    {
        return;
    }
    let Some(reservation) =
        ServerRuntimeStartReservation::try_claim(ctx, MarketplaceFinancialWorkerReservation)
    else {
        return;
    };
    if stop.is_stopping() {
        return;
    }

    if let Some(handle) =
        crate::services::marketplace_financial_worker::spawn_marketplace_financial_worker(
            ctx.clone(),
            stop.subscribe(),
        )
    {
        ctx.shared_insert(handle);
        reservation.commit();
    }
}

#[cfg(feature = "mod-payment")]
fn spawn_payment_provider_event_worker_if_enabled(ctx: &ServerRuntimeContext) {
    let stop = crate::services::app_lifecycle::StopHandle::ensure(ctx);
    if !ctx.settings().runtime.runs_background_workers()
        || stop.is_stopping()
        || ctx.shared_contains::<
            crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle,
        >()
    {
        return;
    }
    let Some(reservation) =
        ServerRuntimeStartReservation::try_claim(ctx, PaymentProviderEventWorkerReservation)
    else {
        return;
    };
    if stop.is_stopping() {
        return;
    }

    ctx.shared_insert(
        crate::services::payment_provider_event_worker::spawn_payment_provider_event_worker(
            ctx.clone(),
            stop.subscribe(),
        ),
    );
    reservation.commit();
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
    extensions.insert(
        crate::services::worker_service_context::materialize_worker_service_context(
            &runtime_ctx,
            &auth_config,
        )?,
    );

    // Static/native tenant settings are owned by rustok-modules. Consumers receive
    // only the owner port through the neutral runtime extension registry.
    let static_settings_reader = std::sync::Arc::new(
        rustok_modules::DatabaseStaticModuleSettingsReader::new(db.clone()),
    );
    extensions.insert(rustok_api::SharedStaticModuleSettingsReader(
        static_settings_reader.clone(),
    ));
    extensions.insert(rustok_api::SharedStaticModuleSettingsTransactionReader(
        static_settings_reader,
    ));

    // Canonical route resolution is owned by content orchestration, which
    // dispatches to Blog and Forum owner tables. SEO receives the port here.
    #[cfg(all(feature = "mod-content", feature = "mod-blog", feature = "mod-forum"))]
    extensions.insert(rustok_content::SharedCanonicalRouteResolver(Arc::new(
        rustok_content_orchestration::OwnerCanonicalRouteResolver::new(db.clone()),
    )));

    #[cfg(all(feature = "mod-seo", feature = "mod-media"))]
    {
        let selected = runtime_ctx
            .shared_get::<rustok_media::MediaAssetPorts>()
            .map(|ports| ports.read())
            .or_else(|| runtime_ctx.shared_get::<Arc<dyn rustok_media::MediaAssetReadPort>>());
        let provider = selected.or_else(|| {
            runtime_ctx
                .shared_get::<rustok_storage::StorageRuntime>()
                .map(|storage| {
                    let provider: Arc<dyn rustok_media::MediaAssetReadPort> =
                        Arc::new(rustok_media::MediaService::new(db.clone(), storage));
                    provider
                })
        });
        if let Some(provider) = provider {
            extensions.insert(rustok_seo::SeoMediaAssetReadProvider::new(provider));
        }
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
        let owner_registry = extensions
            .get::<rustok_taxonomy::TaxonomyModuleTermTranslationOwnerRegistry>()
            .cloned()
            .unwrap_or_default();
        let provider = rustok_taxonomy::TaxonomyTranslationTargetProvider::with_owner_registry(
            Arc::new(rustok_taxonomy::TaxonomyService::new(db.clone())),
            owner_registry,
        );
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
        let service = Arc::new(rustok_product::CatalogService::new(
            db.clone(),
            event_bus.clone(),
        ));
        let schema_service = Arc::new(rustok_product::ProductCatalogSchemaService::new(
            db.clone(),
            event_bus,
        ));
        let provider = rustok_product::ProductTranslationTargetProvider::new(service.clone());
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Product translation target provider registration failed: {error}"
                ))
            })?;
        let provider =
            rustok_product::ProductVariantTranslationTargetProvider::new(service.clone());
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Product Variant translation target provider registration failed: {error}"
                ))
            })?;
        let provider = rustok_product::ProductImageTranslationTargetProvider::new(service);
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Product Image translation target provider registration failed: {error}"
                ))
            })?;
        let provider =
            rustok_product::ProductCatalogSchemaService::attribute_translation_target_provider(
                schema_service.clone(),
            );
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Product Attribute translation target provider registration failed: {error}"
                ))
            })?;
        let provider =
            rustok_product::ProductCatalogSchemaService::attribute_schema_translation_target_provider(
                schema_service.clone(),
            );
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Product Attribute Schema translation target provider registration failed: {error}"
                ))
            })?;
        let provider =
            rustok_product::ProductCatalogSchemaService::category_form_translation_target_provider(
                schema_service.clone(),
            );
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Product Category Form translation target provider registration failed: {error}"
                ))
            })?;
        let provider =
            rustok_product::ProductCatalogSchemaService::attribute_value_translation_target_provider(
                schema_service.clone(),
            );
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Product Attribute Value translation target provider registration failed: {error}"
                ))
            })?;
        let provider =
            rustok_product::ProductCatalogSchemaService::category_seo_translation_target_provider(
                schema_service.clone(),
            );
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Product Category SEO translation target provider registration failed: {error}"
                ))
            })?;
        let provider = rustok_product::ProductCatalogSchemaService::variant_attribute_value_translation_target_provider(
            schema_service,
        );
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Product Variant Attribute Value translation target provider registration failed: {error}"
                ))
            })?;
    }

    #[cfg(feature = "mod-commerce")]
    {
        let event_bus = rustok_outbox::TransactionalEventBus::new(Arc::new(
            rustok_outbox::OutboxTransport::new(db.clone()),
        ));
        let service = Arc::new(
            rustok_commerce::services::collection_translation::CollectionTranslationService::new(
                db.clone(),
                event_bus,
            ),
        );
        let provider = rustok_commerce::CommerceCollectionTranslationTargetProvider::new(service);
        rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)
            .map_err(|error| {
                Error::Message(format!(
                    "Commerce Collection translation target provider registration failed: {error}"
                ))
            })?;
    }

    #[cfg(feature = "mod-region")]
    rustok_region::register_region_translation_target_provider(&mut extensions, db.clone())
        .map_err(|error| {
            Error::Message(format!(
                "Region translation target provider registration failed: {error}"
            ))
        })?;

    #[cfg(feature = "mod-inventory")]
    rustok_inventory::register_stock_location_translation_target_provider(
        &mut extensions,
        db.clone(),
    )
    .map_err(|error| {
        Error::Message(format!(
            "Inventory Stock Location translation target provider registration failed: {error}"
        ))
    })?;

    #[cfg(feature = "mod-pricing")]
    rustok_pricing::register_price_list_translation_target_provider(&mut extensions, db.clone())
        .map_err(|error| {
            Error::Message(format!(
                "Pricing translation target provider registration failed: {error}"
            ))
        })?;

    #[cfg(feature = "mod-translation")]
    {
        let provider =
            crate::static_settings_translation_target::StaticSettingsTranslationTargetProvider::new(
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
        rustok_fulfillment::register_shipping_option_translation_target_provider(
            &mut extensions,
            db.clone(),
        )
        .map_err(|error| {
            Error::Message(format!(
                "Fulfillment Shipping Option translation target provider registration failed: {error}"
            ))
        })?;

        let fulfillment_registry = runtime_ctx
            .shared_get::<rustok_fulfillment::providers::FulfillmentProviderRegistry>()
            .unwrap_or_else(|| {
                let registry = rustok_fulfillment::providers::FulfillmentProviderRegistry::with_manual_provider();
                runtime_ctx.shared_insert(registry.clone());
                registry
            });
        extensions.insert(fulfillment_registry);
    }

    #[cfg(feature = "mod-marketplace_seller")]
    rustok_marketplace_seller::register_marketplace_seller_translation_target_provider(
        &mut extensions,
        db.clone(),
    )
    .map_err(|error| {
        Error::Message(format!(
            "Marketplace Seller presentation translation target provider registration failed: {error}"
        ))
    })?;

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
            auth_config.clone(),
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
            runtime_ctx.clone(),
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
            runtime_ctx.clone(),
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
        rustok_reactions::api::materialize_reaction_subject_registry(&mut extensions, &host)
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
    tracing::debug!(
        handlers_count = handlers.len(),
        "Building module event dispatcher"
    );
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

#[cfg(feature = "mod-product")]
fn spawn_product_media_release_worker_if_enabled(ctx: &ServerRuntimeContext) {
    use crate::services::product_media_release_worker::spawn_product_media_release_worker;
    // Reserve before spawning: concurrent composition must not create two workers.
    struct ProductMediaReleaseWorkerReservation;
    let stop = crate::services::app_lifecycle::StopHandle::ensure(ctx);
    if !ctx.settings().runtime.runs_background_workers()
        || stop.is_stopping()
        || ctx.shared_contains::<
            crate::services::product_media_release_worker::ProductMediaReleaseWorkerHandle,
        >()
        || !ctx.shared_insert_if_absent(ProductMediaReleaseWorkerReservation)
    {
        return;
    }
    ctx.shared_insert(spawn_product_media_release_worker(ctx.clone(), stop.subscribe()));
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
        let extensions = build_shared_runtime_extensions(&registry, &settings)
            .expect("runtime extensions should initialize");

        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let dispatcher =
            build_module_event_dispatcher(&registry, EventBus::default(), db, extensions.as_ref());

        let expected = if cfg!(feature = "mod-workflow") { 2 } else { 1 };
        assert_eq!(dispatcher.handler_count(), expected);
    }

    #[tokio::test]
    async fn host_runtime_extensions_register_admin_mutation_providers() {
        let registry = ModuleRegistry::new();
        #[cfg(feature = "mod-reactions")]
        let registry = registry.register(rustok_reactions::ReactionsModule);
        #[cfg(feature = "mod-moderation")]
        let registry = registry.register(rustok_moderation::ModerationModule);
        let settings = RustokSettings::default();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let runtime_ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(
            db,
            settings.clone(),
        );
        #[cfg(feature = "mod-media")]
        let _media_storage_directory = {
            let directory = tempfile::tempdir().expect("temporary Media storage should initialize");
            let storage =
                rustok_storage::StorageRuntime::local(&rustok_storage::LocalStorageConfig {
                    base_dir: directory.path().display().to_string(),
                    base_url: "/media".to_string(),
                    fsync: false,
                })
                .expect("local Media storage runtime should initialize");
            runtime_ctx.shared_insert(storage);
            directory
        };

        let extensions = build_shared_runtime_extensions_with_host_providers(
            &registry,
            &settings,
            runtime_ctx.clone(),
            AuthConfig::new("test-secret-key-for-unit-tests-only-32bytes!".to_string()),
        )
        .expect("host runtime extensions should initialize");

        assert!(extensions.contains::<rustok_auth::AuthLifecycleRuntime>());
        assert!(extensions.contains::<rustok_auth::AuthUserBackfillRuntime>());
        assert!(extensions.contains::<rustok_auth::OAuthAdminRuntime>());
        assert!(extensions.contains::<rustok_auth::UserAdminMutationRuntime>());
        assert!(extensions.contains::<rustok_mcp::McpManagementRuntime>());
        #[cfg(feature = "mod-media")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "media"
                        && descriptor.resource_kind.as_str() == "asset"
                }))
        );
        #[cfg(feature = "mod-taxonomy")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "taxonomy"
                        && descriptor.resource_kind.as_str() == "term"
                }))
        );
        #[cfg(feature = "mod-blog")]
        assert!(
            !rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "blog"
                        && descriptor.resource_kind.as_str() == "category"
                }))
        );
        #[cfg(feature = "mod-navigation")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "navigation"
                        && descriptor.resource_kind.as_str() == "menu"
                }))
        );
        #[cfg(feature = "mod-pages")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "pages"
                        && descriptor.resource_kind.as_str() == "page_metadata"
                }))
        );
        #[cfg(feature = "mod-product")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "product"
                        && descriptor.resource_kind.as_str() == "product"
                }))
        );
        #[cfg(feature = "mod-product")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "product"
                        && descriptor.resource_kind.as_str() == "variant"
                }))
        );
        #[cfg(feature = "mod-product")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "product"
                        && descriptor.resource_kind.as_str() == "image"
                }))
        );
        #[cfg(feature = "mod-product")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "product"
                        && descriptor.resource_kind.as_str() == "attribute"
                }))
        );
        #[cfg(feature = "mod-product")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "product"
                        && descriptor.resource_kind.as_str() == "attribute_schema"
                }))
        );
        #[cfg(feature = "mod-product")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "product"
                        && descriptor.resource_kind.as_str() == "category_form"
                }))
        );
        #[cfg(feature = "mod-product")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "product"
                        && descriptor.resource_kind.as_str() == "attribute_value"
                }))
        );
        #[cfg(feature = "mod-product")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "product"
                        && descriptor.resource_kind.as_str() == "variant_attribute_value"
                }))
        );
        #[cfg(feature = "mod-product")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "product"
                        && descriptor.resource_kind.as_str() == "category_seo"
                }))
        );
        #[cfg(feature = "mod-commerce")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "commerce"
                        && descriptor.resource_kind.as_str() == "collection_copy"
                }))
        );
        #[cfg(feature = "mod-inventory")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "inventory"
                        && descriptor.resource_kind.as_str() == "stock_location_copy"
                }))
        );
        #[cfg(feature = "mod-pricing")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "pricing"
                        && descriptor.resource_kind.as_str() == "price_list_copy"
                }))
        );
        #[cfg(feature = "mod-translation")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "modules"
                        && descriptor.resource_kind.as_str() == "static_settings"
                }))
        );
        #[cfg(feature = "mod-marketplace_seller")]
        assert!(
            rustok_translation_targets::translation_target_registry(extensions.as_ref())
                .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                    descriptor.owner_slug.as_str() == "marketplace_seller"
                        && descriptor.resource_kind.as_str() == "seller_presentation"
                }))
        );
        #[cfg(feature = "mod-forum")]
        assert!(extensions.contains::<rustok_forum::SharedForumNotificationRecipientContextPort>());
        #[cfg(feature = "mod-forum")]
        assert!(extensions.contains::<rustok_forum::SharedForumAudienceFactsPort>());
        #[cfg(feature = "mod-forum")]
        assert!(extensions.contains::<
            crate::services::forum_posting_policy_facts::SharedForumPostingPolicyFactsComposer,
        >());
        #[cfg(feature = "mod-reactions")]
        assert!(
            rustok_reactions::api::reaction_subject_registry_from_extensions(extensions.as_ref())
                .is_some()
        );
        #[cfg(feature = "mod-moderation")]
        assert!(
            rustok_moderation::moderation_subject_adapter_registry_from_extensions(
                extensions.as_ref()
            )
            .is_some()
        );
        #[cfg(feature = "mod-notifications")]
        assert!(
            rustok_notifications::api::notification_source_registry_from_extensions(
                extensions.as_ref()
            )
            .is_some()
        );
        #[cfg(all(feature = "mod-notifications", feature = "mod-profiles"))]
        assert!(
            extensions
                .get::<rustok_notifications::NotificationRecipientPolicyRuntime>()
                .is_some_and(|runtime| !runtime.candidate_worker_ready())
        );
        #[cfg(feature = "mod-fulfillment")]
        {
            assert!(
                rustok_translation_targets::translation_target_registry(extensions.as_ref())
                    .is_some_and(|registry| registry.descriptors().iter().any(|descriptor| {
                        descriptor.owner_slug.as_str() == "fulfillment"
                            && descriptor.resource_kind.as_str() == "shipping_option_copy"
                    }))
            );
            assert!(
                extensions.contains::<rustok_fulfillment::providers::FulfillmentProviderRegistry>()
            );
            assert!(
                runtime_ctx
                    .shared_get::<rustok_fulfillment::providers::FulfillmentProviderRegistry>()
                    .is_some()
            );
        }
        #[cfg(feature = "commerce-marketplace-financial")]
        assert!(extensions.contains::<rustok_commerce::MarketplaceFinancialRuntime>());
        #[cfg(all(feature = "commerce-marketplace-financial", feature = "mod-payment"))]
        assert!(extensions.contains::<rustok_payment::PaymentProviderEventObservers>());
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::common::settings::RuntimeHostMode;

    #[cfg(feature = "mod-payment")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_payment_recovery_starts_only_one_runtime_worker() {
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::Worker;
        let ctx = ServerRuntimeContext::new(sea_orm::DatabaseConnection::default(), settings);
        let arrivals = Arc::new(tokio::sync::Barrier::new(16));
        let mut calls = Vec::new();
        for _ in 0..16 {
            let ctx = ctx.clone();
            let arrivals = Arc::clone(&arrivals);
            calls.push(tokio::spawn(async move {
                arrivals.wait().await;
                spawn_payment_provider_event_worker_if_enabled(&ctx);
            }));
        }
        for call in calls {
            call.await.expect("worker composition should not panic");
        }
        let first_instance = ctx
            .shared_map::<
                crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle,
                _,
            >(|worker| worker.instance_id())
            .expect("one payment recovery worker must be registered");
        assert!(ctx.shared_contains::<PaymentProviderEventWorkerReservation>());
        spawn_payment_provider_event_worker_if_enabled(&ctx);
        assert_eq!(
            ctx.shared_map::<
                crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle,
                _,
            >(|worker| worker.instance_id()),
            Some(first_instance),
            "repeated startup cannot overwrite the owner handle"
        );

        crate::services::app_lifecycle::StopHandle::ensure(&ctx)
            .stop()
            .await;
        if let Some(worker) = ctx.shared_take::<
            crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle,
        >() {
            tokio::time::timeout(std::time::Duration::from_secs(3), worker.join())
                .await
                .expect("payment recovery should stop cooperatively")
                .expect("payment recovery join should succeed");
        }
    }

    #[cfg(feature = "mod-commerce")]
    #[test]
    fn unavailable_fulfillment_registry_does_not_poison_worker_admission() {
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::Worker;
        let ctx = ServerRuntimeContext::new(sea_orm::DatabaseConnection::default(), settings);
        spawn_paid_order_label_worker_if_enabled(&ctx);
        assert!(!ctx.shared_contains::<PaidOrderLabelWorkerReservation>());
        assert!(!ctx.shared_contains::<
            crate::services::paid_order_label_worker::PaidOrderCreateLabelWorkerHandle,
        >());
    }

    #[cfg(feature = "commerce-marketplace-financial")]
    #[test]
    fn unavailable_financial_runtime_does_not_poison_worker_admission() {
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::Worker;
        let ctx = ServerRuntimeContext::new(sea_orm::DatabaseConnection::default(), settings);
        spawn_marketplace_financial_worker_if_enabled(&ctx);
        assert!(!ctx.shared_contains::<MarketplaceFinancialWorkerReservation>());
        assert!(!ctx.shared_contains::<
            crate::services::marketplace_financial_worker::MarketplaceFinancialWorkerHandle,
        >());
    }

    #[cfg(feature = "mod-payment")]
    #[tokio::test]
    async fn stopped_host_does_not_register_payment_recovery_worker() {
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::Worker;
        let ctx = ServerRuntimeContext::new(sea_orm::DatabaseConnection::default(), settings);
        crate::services::app_lifecycle::StopHandle::ensure(&ctx)
            .stop()
            .await;
        spawn_payment_provider_event_worker_if_enabled(&ctx);
        assert!(!ctx.shared_contains::<PaymentProviderEventWorkerReservation>());
        assert!(!ctx.shared_contains::<
            crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle,
        >());
    }

    #[test]
    fn concurrent_dispatcher_start_does_not_report_success_before_publication() {
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::Worker;
        let ctx = ServerRuntimeContext::new(sea_orm::DatabaseConnection::default(), settings);

        let first = claim_module_dispatcher_start(&ctx)
            .expect("first starter may reserve")
            .expect("first starter owns the in-flight claim");
        assert!(ctx.shared_contains::<ModuleEventDispatcherReservation>());
        assert!(!ctx.shared_contains::<ModuleEventDispatcherHandle>());

        let contender = claim_module_dispatcher_start(&ctx);
        assert!(
            matches!(contender, Err(Error::Message(message)) if message.contains("in progress")),
            "the loser cannot report success before dispatcher publication"
        );
        drop(first);
        assert!(!ctx.shared_contains::<ModuleEventDispatcherReservation>());
        let retry = claim_module_dispatcher_start(&ctx)
            .expect("failed or cancelled bootstrap permits a retry")
            .expect("the retry takes the released claim");
        drop(retry);
    }

    #[test]
    fn dispatcher_start_claim_is_released_on_unwind() {
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::Worker;
        let ctx = ServerRuntimeContext::new(sea_orm::DatabaseConnection::default(), settings);
        let cancelled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _reservation = claim_module_dispatcher_start(&ctx)
                .expect("claim should work")
                .expect("first owner");
            panic!("simulate panic during synchronous extension construction");
        }));
        assert!(cancelled.is_err());
        assert!(!ctx.shared_contains::<ModuleEventDispatcherReservation>());
        let recovered = claim_module_dispatcher_start(&ctx)
            .expect("retry after unwind")
            .expect("claim was released by guard drop");
        drop(recovered);
    }

    #[tokio::test]
    async fn dispatcher_start_requires_live_handle_and_published_listener_flag() {
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::Worker;
        let ctx = ServerRuntimeContext::new(sea_orm::DatabaseConnection::default(), settings);
        let reservation = claim_module_dispatcher_start(&ctx)
            .expect("first admission")
            .expect("first owner");
        ctx.shared_insert(ModuleEventDispatcherHandle::for_test());

        assert!(
            matches!(claim_module_dispatcher_start(&ctx), Err(Error::Message(message)) if message.contains("not ready")),
            "publishing a handle before the required listener flag is not success"
        );
        ctx.shared_insert(ModuleEventDispatcherRequirement(true));
        assert!(
            matches!(claim_module_dispatcher_start(&ctx), Ok(None)),
            "a live dispatcher and listener flag are idempotently ready"
        );
        reservation.commit();

        let handle = ctx
            .shared_take::<ModuleEventDispatcherHandle>()
            .expect("mock dispatcher is present");
        handle.request_stop();
        handle.join().await.expect("mock dispatcher shuts down");
        assert!(
            ctx.shared_get::<ModuleEventDispatcherRequirement>()
                .is_some_and(|value| value.0),
            "the flag remains registered even after worker shutdown"
        );
        assert!(
            matches!(claim_module_dispatcher_start(&ctx), Err(Error::Message(message)) if message.contains("in progress")),
            "a committed claim without a worker cannot be treated as readiness"
        );
    }

    #[test]
    fn module_event_dispatcher_runs_only_in_background_worker_profiles() {
        for mode in [RuntimeHostMode::Full, RuntimeHostMode::Worker] {
            let mut settings = RustokSettings::default();
            settings.runtime.host_mode = mode;
            assert!(module_event_dispatcher_enabled(&settings));
        }
        for mode in [
            RuntimeHostMode::Api,
            RuntimeHostMode::AdminSsr,
            RuntimeHostMode::StorefrontSsr,
            RuntimeHostMode::RegistryOnly,
        ] {
            let mut settings = RustokSettings::default();
            settings.runtime.host_mode = mode;
            assert!(!module_event_dispatcher_enabled(&settings));
        }
    }

    #[test]
    fn missing_event_runtime_fails_before_any_module_worker_is_spawned() {
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = RuntimeHostMode::Worker;
        let registry = ModuleRegistry::new();
        let extensions = build_shared_runtime_extensions(&registry, &settings)
            .expect("empty module registry can build extensions");
        let ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            settings,
        );

        let error = spawn_module_event_dispatcher(&ctx, &registry, extensions)
            .expect_err("worker host requires EventRuntime before starting any child");
        assert!(matches!(
            error,
            crate::error::Error::Message(ref message)
                if message.contains("EventRuntime must be initialized")
        ));
        assert!(!ctx.shared_contains::<ModuleEventDispatcherHandle>());
        #[cfg(feature = "mod-payment")]
        assert!(!ctx.shared_contains::<
            crate::services::payment_provider_event_worker::PaymentProviderEventWorkerHandle,
        >());
        #[cfg(feature = "mod-product")]
        assert!(!ctx.shared_contains::<
            crate::services::product_media_release_worker::ProductMediaReleaseWorkerHandle,
        >());
        #[cfg(feature = "mod-commerce")]
        assert!(!ctx.shared_contains::<
            crate::services::paid_order_label_worker::PaidOrderCreateLabelWorkerHandle,
        >());
        #[cfg(feature = "commerce-marketplace-financial")]
        assert!(!ctx.shared_contains::<
            crate::services::marketplace_financial_worker::MarketplaceFinancialWorkerHandle,
        >());
    }

    #[tokio::test]
    async fn empty_listener_set_binds_an_explicit_registry_for_transport_routing() {
        use rustok_core::events::{EventTransport, MemoryTransport};
        use rustok_events::{
            ContractEventEnvelope, DomainEvent, EventEnvelope, ProductIndexRefreshEvent,
        };
        use uuid::Uuid;

        let memory = MemoryTransport::with_capacity(8);
        let bus = memory.event_bus();
        let ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            RustokSettings::default(),
        );

        start_module_event_dispatcher(&ctx, EventDispatcher::new(bus.clone()), 0)
            .expect("empty listener registry should start");

        assert_eq!(
            ctx.shared_get::<ModuleEventDispatcherRequirement>()
                .map(|requirement| requirement.0),
            Some(true),
            "the explicit empty registry is still required by the worker transport"
        );
        assert_eq!(
            ctx.shared_map::<ModuleEventDispatcherHandle, _>(|handle| !handle.is_finished()),
            Some(true)
        );

        let remote_only = ContractEventEnvelope::new(
            Uuid::new_v4(),
            None,
            ProductIndexRefreshEvent::VariantRefreshRequested {
                product_id: Uuid::new_v4(),
                variant_id: Uuid::new_v4(),
                source_version: 1,
            },
        )
        .expect("remote-only contract envelope");
        assert!(
            !bus.has_contract_handler(&remote_only)
                .expect("an empty registry is initialized, not unavailable")
        );

        let tenant_id = Uuid::new_v4();
        let root_event = EventEnvelope::new(
            tenant_id,
            None,
            DomainEvent::TenantUpdated { tenant_id },
        );
        memory
            .publish(root_event)
            .await
            .expect("the empty dispatcher must keep the root-event receiver bound");

        let handle = ctx
            .shared_take::<ModuleEventDispatcherHandle>()
            .expect("dispatcher handle should be owned by the runtime context");
        handle.request_stop();
        handle.join().await.expect("empty dispatcher should drain");
    }

    #[tokio::test]
    async fn dispatcher_handle_aborts_and_joins_the_receive_loop() {
        let bus = EventBus::new();
        let running = EventDispatcher::new(bus).start();
        let handle = ModuleEventDispatcherHandle { running };
        assert!(!handle.is_finished());
        handle.abort();
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), handle.join()).await;
        assert!(matches!(result, Ok(Err(error)) if error.is_cancelled()));
    }

    #[tokio::test]
    async fn dispatcher_handle_requests_cooperative_stop_and_joins() {
        let bus = EventBus::new();
        let running = EventDispatcher::new(bus).start();
        let handle = ModuleEventDispatcherHandle { running };
        handle.request_stop();
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), handle.join()).await;
        assert!(matches!(result, Ok(Ok(()))));
    }
}
