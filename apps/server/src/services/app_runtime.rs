use std::sync::Arc;

use crate::error::{Error, Result};
use rustok_core::ModuleRegistry;

use crate::auth::AuthConfig;
use crate::common::settings::{RuntimeHostMode, RustokSettings, SharedRustokSettings};
use crate::graphql::AppSchema;
use crate::middleware;
use crate::middleware::rate_limit::{
    PUBLIC_READINESS_RATE_LIMIT_PREFIXES, PathRateLimitMiddlewareState, PathRateLimitPolicy,
    RateLimitConfig, RateLimiter, SharedApiRateLimiter, SharedAuthRateLimiter,
    SharedOAuthRateLimiter, SharedSearchRateLimiter, cleanup_task,
};
use crate::modules;
use crate::modules::{DeploymentSurfaceContract, ManifestManager};
use crate::services::cache_runtime::ensure_cache_service;
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::event_transport_factory::build_event_runtime;
use crate::services::graphql_schema::init_graphql_schema;
use crate::services::marketplace_catalog::{
    LocalManifestMarketplaceProvider, MarketplaceCatalogProvider, MarketplaceCatalogService,
    SharedMarketplaceCatalogService,
};
use crate::services::marketplace_catalog_cache::HardenedRegistryMarketplaceProvider;
use crate::services::module_event_dispatcher::{
    build_shared_runtime_extensions_with_host_providers, spawn_module_event_dispatcher,
};
use crate::services::oauth_app::sync_manifest_managed_apps_for_all_tenants;
use crate::services::platform_composition::PlatformCompositionService;
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_cache::CacheService;
use rustok_core::ModuleRuntimeExtensions;

pub struct AppRuntimeBootstrap {
    pub deployment_surfaces: DeploymentSurfaceContract,
    pub registry: ModuleRegistry,
    pub graphql_schema: Arc<AppSchema>,
    pub rate_limit_state: PathRateLimitMiddlewareState,
}

fn validate_compiled_surface_contract(
    contract: &DeploymentSurfaceContract,
    compiled_embed_admin: bool,
    compiled_embed_storefront: bool,
) -> Result<()> {
    if contract.embed_admin && !compiled_embed_admin {
        return Err(Error::BadRequest(
            "modules.toml requires embedded admin, but the server was built without feature `embed-admin`".to_string(),
        ));
    }

    if contract.embed_storefront && !compiled_embed_storefront {
        return Err(Error::BadRequest(
            "modules.toml requires embedded storefront, but the server was built without feature `embed-storefront`".to_string(),
        ));
    }

    Ok(())
}

pub async fn bootstrap_app_runtime(
    runtime_ctx: ServerRuntimeContext,
    auth_config: AuthConfig,
) -> Result<AppRuntimeBootstrap> {
    let settings = runtime_ctx.settings().clone();
    let cache_service = ensure_cache_service(&runtime_ctx);

    // Cache parsed settings so per-request middleware avoids repeated JSON deserialization.
    runtime_ctx.shared_insert(SharedRustokSettings(Arc::new(settings.clone())));

    init_marketplace_catalog(&runtime_ctx);

    let manifest = if settings.runtime.is_registry_only() {
        ManifestManager::load().map_err(|error| {
            Error::BadRequest(format!("modules.toml validation failed: {error}"))
        })?
    } else {
        PlatformCompositionService::active_manifest(runtime_ctx.db())
            .await
            .map_err(|error| {
                Error::BadRequest(format!("platform composition validation failed: {error}"))
            })?
    };
    let deployment_surfaces = match settings.runtime.host_mode {
        RuntimeHostMode::RegistryOnly | RuntimeHostMode::Worker | RuntimeHostMode::Api => {
            DeploymentSurfaceContract {
                profile: rustok_build::DeploymentProfile::HeadlessApi,
                embed_admin: false,
                embed_storefront: false,
            }
        }
        RuntimeHostMode::AdminSsr => DeploymentSurfaceContract {
            profile: rustok_build::DeploymentProfile::ServerWithAdmin,
            embed_admin: true,
            embed_storefront: false,
        },
        RuntimeHostMode::StorefrontSsr => DeploymentSurfaceContract {
            profile: rustok_build::DeploymentProfile::ServerWithStorefront,
            embed_admin: false,
            embed_storefront: true,
        },
        RuntimeHostMode::Full => ManifestManager::deployment_surface_contract(&manifest),
    };
    validate_compiled_surface_contract(
        &deployment_surfaces,
        cfg!(feature = "embed-admin"),
        cfg!(feature = "embed-storefront"),
    )?;

    if !settings.runtime.is_registry_only() {
        init_storage(&runtime_ctx).await?;
    }

    let registry = modules::build_registry();
    let runtime_extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime_ctx.clone(),
        auth_config.clone(),
    )?;
    runtime_ctx.shared_insert(runtime_extensions.clone());
    runtime_ctx.shared_insert(registry.clone());
    runtime_ctx.shared_insert(rustok_modules::SharedModuleMarketplaceCatalog(Arc::new(
        crate::services::marketplace_catalog_adapter::ServerMarketplaceCatalog::new(
            runtime_ctx.clone(),
            registry.clone(),
        ),
    )));
    ManifestManager::validate(&manifest)
        .and_then(|_| ManifestManager::validate_with_registry(&manifest, &registry))
        .map_err(|error| Error::BadRequest(format!("modules.toml validation failed: {error}")))?;
    if !settings.runtime.is_registry_only() {
        let event_runtime = build_event_runtime(&runtime_ctx).await?;
        runtime_ctx.shared_insert(event_runtime.transport.clone());
        runtime_ctx.shared_insert(transactional_event_bus_from_context(&runtime_ctx));
        spawn_module_event_dispatcher(&runtime_ctx, &registry, runtime_extensions.clone())?;
        runtime_ctx.shared_insert(Arc::new(event_runtime));
        runtime_ctx.shared_insert(crate::services::build_control::ServerBuildControl::shared(
            runtime_ctx.clone(),
        ));
        runtime_ctx.shared_insert(
            crate::services::mcp_runtime::DbBackedMcpRuntimeBridge::shared(runtime_ctx.db_clone()),
        );
        sync_manifest_managed_apps_for_all_tenants(runtime_ctx.db(), &manifest)
            .await
            .map_err(|error| {
                Error::Message(format!(
                    "Failed to sync manifest-managed OAuth apps: {error}"
                ))
            })?;
        middleware::tenant::init_tenant_cache_infrastructure(&runtime_ctx, &cache_service).await?;
        runtime_ctx.shared_insert(
            rustok_content_orchestration::build_content_orchestration_service(
                runtime_ctx.db_clone(),
                transactional_event_bus_from_context(&runtime_ctx),
            ),
        );
        #[cfg(all(feature = "mod-content", feature = "mod-blog", feature = "mod-forum"))]
        runtime_ctx.shared_insert(rustok_content::SharedCanonicalRouteResolver(
            std::sync::Arc::new(
                rustok_content_orchestration::OwnerCanonicalRouteResolver::new(
                    runtime_ctx.db_clone(),
                ),
            ),
        ));

        #[cfg(feature = "mod-workflow")]
        if workflow_cron_enabled(&settings) {
            init_workflow_runtime(&runtime_ctx);
        } else {
            tracing::info!(
                host_mode = ?settings.runtime.host_mode,
                configured = settings.runtime.background_workers.workflow_cron_enabled,
                "Workflow cron scheduler disabled for the current runtime profile"
            );
        }

        init_alloy_runtime(&runtime_ctx, &manifest).await?;
    }

    if settings.runtime.is_registry_only() {
        use rustok_outbox::OutboxTransport;

        // Registry-only mode does not start the event runtime or relay, but GraphQL
        // schema construction still requires the canonical durable write transport and event bus.
        if runtime_ctx
            .shared_get::<std::sync::Arc<dyn rustok_core::events::EventTransport>>()
            .is_none()
        {
            runtime_ctx.shared_insert(std::sync::Arc::new(OutboxTransport::new(
                runtime_ctx.db_clone(),
            ))
                as std::sync::Arc<dyn rustok_core::events::EventTransport>);
        }

        if runtime_ctx
            .shared_get::<rustok_outbox::TransactionalEventBus>()
            .is_none()
        {
            runtime_ctx.shared_insert(transactional_event_bus_from_context(&runtime_ctx));
        }
    }

    initialize_module_work_runtime(&runtime_ctx, &registry, runtime_extensions.as_ref()).await?;

    // GraphQL module policies consume shared runtime capabilities during schema construction.
    // Initialize rate-limit runtimes first so Blog/Search GraphQL policies receive their
    // configured limiter instead of permanently composing a fail-open None handle.
    let rate_limits =
        init_rate_limit_layers(&runtime_ctx, &settings, &cache_service, Some(auth_config))?;
    let graphql_schema = init_graphql_schema(&runtime_ctx)?;

    Ok(AppRuntimeBootstrap {
        deployment_surfaces,
        registry,
        graphql_schema,
        rate_limit_state: rate_limits.combined_state,
    })
}

async fn initialize_module_work_runtime(
    ctx: &ServerRuntimeContext,
    registry: &ModuleRegistry,
    extensions: &ModuleRuntimeExtensions,
) -> Result<()> {
    use crate::services::app_lifecycle::{ModuleWorkSchedulerInitializationLock, StopHandle};

    // Registration can await, so a check-then-spawn permits competing bootstraps
    // to leave an untracked scheduler running. Cancellation releases this guard.
    let initialization = ModuleWorkSchedulerInitializationLock::ensure(ctx);
    let _guard = initialization.0.lock().await;
    if ctx.shared_contains::<crate::services::app_lifecycle::ModuleWorkSchedulerHandle>() {
        return Ok(());
    }

    let registrations = extensions.get::<rustok_runtime::ModuleWorkRegistrations>();
    // Record the same registration decision used to start the scheduler so
    // readiness distinguishes an empty composition from a missing task.
    ctx.shared_insert(
        crate::services::app_lifecycle::ModuleWorkSchedulerRequirement(
            registrations.is_some_and(|registrations| !registrations.is_empty()),
        ),
    );
    let Some(registrations) = registrations else {
        return Ok(());
    };
    if registrations.is_empty() || !ctx.settings().runtime.runs_background_workers() {
        return Ok(());
    }
    let stop = StopHandle::ensure(ctx);
    if stop.is_stopping() {
        tracing::info!("Shutdown requested; skipping module work scheduler startup");
        return Ok(());
    }

    let host = extensions.apply_to_host_runtime(
        rustok_api::HostRuntimeContext::new(ctx.db_clone())
            .with_shared_value(transactional_event_bus_from_context(ctx))
            .with_shared_value(registry.clone()),
    );
    let host = if let Some(storage) = ctx.shared_get::<rustok_storage::StorageRuntime>() {
        host.with_shared_value(storage)
    } else {
        host
    };
    let artifact_delivery_tenants: rustok_modules::SharedArtifactDeliveryTenantSource = Arc::new(
        crate::services::artifact_delivery_tenants::ServerArtifactDeliveryTenantSource::new(
            ctx.db_clone(),
        ),
    );
    let host = host.with_shared_value(artifact_delivery_tenants);
    let host = if ctx.shared_get::<rustok_storage::StorageRuntime>().is_some() {
        let executor = if let Some(executor) =
            ctx.shared_get::<rustok_modules::SharedArtifactBindingExecutor>()
        {
            executor
        } else {
            crate::services::artifact_runtime::compose_artifact_binding_executor(ctx).await?
        };
        ctx.shared_insert(executor.clone());
        host.with_shared_value(executor)
    } else {
        host
    };
    let host =
        crate::services::commerce_provider_runtime::attach_commerce_provider_registries(host, ctx);
    #[cfg(feature = "mod-alloy")]
    let host = if let Some(alloy_runtime) = ctx.shared_get::<alloy::SharedAlloyRuntime>() {
        host.with_shared_value(alloy_runtime)
    } else {
        host
    };
    let scheduler = rustok_runtime::ModuleWorkScheduler::new();
    registrations
        .register_all(&host, &scheduler)
        .await
        .map_err(|error| Error::Message(format!("module work registration failed: {error}")))?;
    // Registration can overlap shutdown. Do not create a new scheduler after
    // it completes if shutdown has already been requested.
    if stop.is_stopping() {
        tracing::info!("Shutdown requested during module work registration; skipping startup");
        return Ok(());
    }
    let stop = stop.subscribe();
    let handle = tokio::spawn(async move {
        scheduler
            .run_until_stopped(stop, std::time::Duration::from_secs(1))
            .await;
    });
    ctx.shared_insert(crate::services::app_lifecycle::ModuleWorkSchedulerHandle::new(handle));
    Ok(())
}

pub fn module_runtime_extensions_from_ctx(
    ctx: &ServerRuntimeContext,
) -> Arc<ModuleRuntimeExtensions> {
    ctx.shared_get::<Arc<ModuleRuntimeExtensions>>()
        .expect("ModuleRuntimeExtensions not initialized; bootstrap_app_runtime must run first")
}

async fn init_storage(ctx: &ServerRuntimeContext) -> Result<()> {
    use rustok_storage::StorageRuntime;

    let settings = ctx.settings();
    let mut storage = settings.storage.clone();
    let layout = rustok_runtime::resolve_instance_layout_from_environment()
        .map_err(|error| Error::Message(error.to_string()))?;
    storage.bind_local_base_dir(layout.storage());
    let service = StorageRuntime::from_config(&storage)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to initialize storage backend: {error}"))
        })?;
    tracing::info!(driver = ?storage.driver, "Initialized storage backend");
    ctx.shared_insert(service);
    Ok(())
}

fn init_marketplace_catalog(ctx: &ServerRuntimeContext) {
    let mut providers: Vec<Arc<dyn MarketplaceCatalogProvider>> =
        vec![Arc::new(LocalManifestMarketplaceProvider)];
    providers.extend(
        HardenedRegistryMarketplaceProvider::from_env()
            .into_iter()
            .map(|provider| Arc::new(provider) as Arc<dyn MarketplaceCatalogProvider>),
    );
    let marketplace_catalog = Arc::new(MarketplaceCatalogService::new(providers));
    tracing::info!(
        providers = ?marketplace_catalog.provider_keys(),
        "Initialized bounded marketplace catalog provider chain"
    );
    ctx.shared_insert(SharedMarketplaceCatalogService(marketplace_catalog));
}

async fn init_alloy_runtime(
    ctx: &ServerRuntimeContext,
    manifest: &crate::modules::ModulesManifest,
) -> Result<()> {
    #[cfg(not(feature = "mod-alloy"))]
    {
        let _ = (ctx, manifest);
        Ok(())
    }

    #[cfg(feature = "mod-alloy")]
    {
        if !manifest.modules.contains_key("alloy") {
            return Ok(());
        }
        if ctx.shared_get::<alloy::SharedAlloyRuntime>().is_none() {
            if std::env::var("RUSTOK_SANDBOX_WORKER_ENDPOINT").is_ok() {
                let rhai = crate::services::artifact_runtime::sandbox_rhai_executor(ctx).await?;
                let mut executors = rustok_sandbox::ExecutorRegistry::new();
                executors.register_isolated_worker(rhai).map_err(|error| {
                    Error::Message(format!("Alloy isolated Rhai executor failed: {error}"))
                })?;
                let sandbox = rustok_sandbox::SandboxRuntime::new(
                    executors,
                    Arc::new(rustok_sandbox::CapabilityBrokerRouter::new()),
                );
                let draft_runtime = alloy::AlloyDraftRuntime::new(
                    sandbox,
                    rustok_sandbox::SandboxPolicy::default(),
                )
                .with_imported_draft_policy_provider(
                    crate::services::registry_governance::alloy_imported_draft_policy_provider_handle(
                        ctx.db_clone(),
                    ),
                );
                ctx.shared_insert(alloy::SharedAlloyRuntime(alloy::build_alloy_runtime(
                    ctx.db_clone(),
                    draft_runtime,
                )));
            } else {
                tracing::warn!(
                    "RUSTOK_SANDBOX_WORKER_ENDPOINT is not configured; Alloy scripting runtime disabled"
                );
            }
        }
        Ok(())
    }
}

#[cfg(feature = "mod-workflow")]
fn workflow_cron_enabled(settings: &RustokSettings) -> bool {
    settings.runtime.runs_background_workers()
        && settings.runtime.background_workers.workflow_cron_enabled
}

#[cfg(feature = "mod-workflow")]
fn init_workflow_runtime(ctx: &ServerRuntimeContext) {
    if ctx.shared_contains::<crate::services::app_lifecycle::WorkflowCronSchedulerHandle>() {
        return;
    }

    use rustok_workflow::WorkflowCronScheduler;
    let db = ctx.db_clone();

    // Start the cron scheduler
    let scheduler = WorkflowCronScheduler::new(db);
    let mut cron_task = scheduler.start();
    let mut stop_rx = crate::services::app_lifecycle::StopHandle::ensure(ctx).subscribe();

    let join_handle = tokio::spawn(async move {
        tokio::select! {
            res = &mut cron_task => {
                if let Err(error) = res {
                    tracing::error!("Workflow cron scheduler panicked: {:?}", error);
                }
            }
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!("Workflow cron scheduler received shutdown signal, aborting task");
                    cron_task.abort();
                }
            }
        }
    });

    ctx.shared_insert(
        crate::services::app_lifecycle::WorkflowCronSchedulerHandle::new(join_handle),
    );
    tracing::info!("Workflow runtime initialized (cron scheduler)");
}

struct RateLimitLayers {
    combined_state: PathRateLimitMiddlewareState,
}

const AUTH_RATE_LIMIT_PREFIXES: &[&str] = &[
    "/api/auth/login",
    "/api/auth/register",
    "/api/auth/session",
    "/api/auth/reset",
    "/api/auth/verify",
    "/api/auth/invite",
];

fn init_rate_limit_layers(
    ctx: &ServerRuntimeContext,
    settings: &RustokSettings,
    cache_service: &CacheService,
    auth_config: Option<AuthConfig>,
) -> Result<RateLimitLayers> {
    let trusted_auth_dimensions = settings.rate_limit.trusted_auth_dimensions;

    let api_limiter = build_namespaced_rate_limiter(
        ctx,
        settings,
        cache_service,
        "api",
        settings.rate_limit.requests_per_minute,
        settings.rate_limit.burst,
        SharedLimiterNamespace::Api,
    )?;
    let auth_limiter = build_namespaced_rate_limiter(
        ctx,
        settings,
        cache_service,
        "auth",
        settings.rate_limit.auth_requests_per_minute,
        settings.rate_limit.auth_burst,
        SharedLimiterNamespace::Auth,
    )?;
    let oauth_limiter = build_namespaced_rate_limiter(
        ctx,
        settings,
        cache_service,
        "oauth",
        settings.rate_limit.oauth_requests_per_minute,
        settings.rate_limit.oauth_burst,
        SharedLimiterNamespace::Oauth,
    )?;
    let _search_limiter = build_namespaced_rate_limiter(
        ctx,
        settings,
        cache_service,
        "search",
        settings.rate_limit.requests_per_minute,
        settings.rate_limit.burst,
        SharedLimiterNamespace::Search,
    )?;

    Ok(RateLimitLayers {
        combined_state: PathRateLimitMiddlewareState {
            policies: Arc::new(vec![
                PathRateLimitPolicy {
                    limiter: oauth_limiter,
                    prefixes: Arc::new(vec![
                        "/api/oauth/token",
                        "/api/oauth/revoke",
                        "/api/oauth/authorize",
                    ]),
                },
                PathRateLimitPolicy {
                    limiter: auth_limiter,
                    prefixes: Arc::new(AUTH_RATE_LIMIT_PREFIXES.to_vec()),
                },
                PathRateLimitPolicy {
                    limiter: api_limiter.clone(),
                    prefixes: Arc::new(PUBLIC_READINESS_RATE_LIMIT_PREFIXES.to_vec()),
                },
                PathRateLimitPolicy {
                    limiter: api_limiter,
                    prefixes: Arc::new(vec!["/api/"]),
                },
            ]),
            auth_config,
            trusted_auth_dimensions,
            request_trust: settings.runtime.request_trust.clone(),
        },
    })
}

enum SharedLimiterNamespace {
    Api,
    Auth,
    Oauth,
    Search,
}

fn build_namespaced_rate_limiter(
    ctx: &ServerRuntimeContext,
    settings: &RustokSettings,
    cache_service: &CacheService,
    namespace: &'static str,
    requests_per_minute: u32,
    burst: u32,
    shared_namespace: SharedLimiterNamespace,
) -> Result<Arc<RateLimiter>> {
    let config = if settings.rate_limit.enabled {
        RateLimitConfig::per_minute(requests_per_minute, burst)
    } else {
        RateLimitConfig::disabled()
    };

    let limiter = Arc::new(
        RateLimiter::build_for_backend(
            config,
            settings.rate_limit.backend,
            &settings.rate_limit.redis_key_prefix,
            namespace,
            cache_service,
        )
        .map_err(Error::BadRequest)?,
    );

    match shared_namespace {
        SharedLimiterNamespace::Api => ctx.shared_insert(SharedApiRateLimiter(limiter.clone())),
        SharedLimiterNamespace::Auth => ctx.shared_insert(SharedAuthRateLimiter(limiter.clone())),
        SharedLimiterNamespace::Oauth => ctx.shared_insert(SharedOAuthRateLimiter(limiter.clone())),
        SharedLimiterNamespace::Search => {
            ctx.shared_insert(SharedSearchRateLimiter(limiter.clone()))
        }
    }

    if settings.rate_limit.enabled {
        let limiter_for_cleanup = limiter.clone();
        tokio::spawn(async move {
            cleanup_task(limiter_for_cleanup).await;
        });
    }

    Ok(limiter)
}

#[cfg(test)]
mod tests {
    use super::{AUTH_RATE_LIMIT_PREFIXES, validate_compiled_surface_contract};
    use crate::common::settings::{RuntimeHostMode, RuntimeSettings, RustokSettings};
    use crate::modules::DeploymentSurfaceContract;
    use rustok_build::DeploymentProfile;
    use rustok_test_utils::db::setup_test_db_with_migrations;

    struct TestModuleWorkRegistration {
        fail: bool,
    }

    #[async_trait::async_trait]
    impl rustok_runtime::ModuleWorkRegistration for TestModuleWorkRegistration {
        async fn register(
            &self,
            _host: &rustok_runtime::HostRuntimeContext,
            _scheduler: &rustok_runtime::ModuleWorkScheduler,
        ) -> std::result::Result<(), rustok_runtime::ModuleWorkError> {
            if self.fail {
                return Err(rustok_runtime::ModuleWorkError::Source(
                    "test failure".into(),
                ));
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn module_work_runtime_records_registration_and_host_mode() {
        use crate::services::app_lifecycle::{
            ModuleWorkSchedulerHandle, ModuleWorkSchedulerRequirement, StopHandle,
        };
        use crate::services::server_runtime_context::ServerRuntimeContext;
        use rustok_core::{ModuleRegistry, ModuleRuntimeExtensions};
        use rustok_runtime::ModuleWorkRegistrations;
        use std::sync::Arc;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let registry = ModuleRegistry::new();
        let empty_ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let mut extensions = ModuleRuntimeExtensions::default();
        // Both a missing registration collection and an explicitly empty one
        // are valid and must not require a scheduler.
        for explicit_empty in [false, true] {
            if explicit_empty {
                extensions.insert(ModuleWorkRegistrations::default());
            }
            super::initialize_module_work_runtime(&empty_ctx, &registry, &extensions)
                .await
                .unwrap();
            assert!(
                !empty_ctx
                    .shared_get::<ModuleWorkSchedulerRequirement>()
                    .unwrap()
                    .0
            );
            assert!(!empty_ctx.shared_contains::<ModuleWorkSchedulerHandle>());
        }

        let mut registrations = ModuleWorkRegistrations::default();
        registrations.register(Arc::new(TestModuleWorkRegistration { fail: false }));
        extensions.insert(registrations);
        for mode in [
            RuntimeHostMode::Full,
            RuntimeHostMode::Worker,
            RuntimeHostMode::Api,
            RuntimeHostMode::AdminSsr,
            RuntimeHostMode::StorefrontSsr,
            RuntimeHostMode::RegistryOnly,
        ] {
            let mut settings = RustokSettings::default();
            settings.runtime.host_mode = mode;
            let local = settings.runtime.runs_background_workers();
            let ctx = ServerRuntimeContext::new(db.clone(), settings);
            super::initialize_module_work_runtime(&ctx, &registry, &extensions)
                .await
                .unwrap();
            assert!(
                ctx.shared_get::<ModuleWorkSchedulerRequirement>()
                    .unwrap()
                    .0
            );
            assert_eq!(ctx.shared_contains::<ModuleWorkSchedulerHandle>(), local);
            if local {
                assert_eq!(
                    ctx.shared_map::<ModuleWorkSchedulerHandle, _>(
                        ModuleWorkSchedulerHandle::is_finished,
                    ),
                    Some(false)
                );
                ctx.shared_get::<StopHandle>().unwrap().stop().await;
                let handle = ctx.shared_take::<ModuleWorkSchedulerHandle>().unwrap();
                tokio::time::timeout(std::time::Duration::from_secs(2), handle.join())
                    .await
                    .expect("scheduler should stop")
                    .expect("scheduler task should not panic");
            }
        }

        let mut failing = ModuleWorkRegistrations::default();
        failing.register(Arc::new(TestModuleWorkRegistration { fail: true }));
        extensions.insert(failing);
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        assert!(
            super::initialize_module_work_runtime(&ctx, &registry, &extensions)
                .await
                .is_err()
        );
        assert!(
            ctx.shared_get::<ModuleWorkSchedulerRequirement>()
                .unwrap()
                .0
        );
        assert!(!ctx.shared_contains::<ModuleWorkSchedulerHandle>());

        // A failed registration must not leave a permanent reservation.
        let mut retry = ModuleWorkRegistrations::default();
        retry.register(Arc::new(TestModuleWorkRegistration { fail: false }));
        extensions.insert(retry);
        super::initialize_module_work_runtime(&ctx, &registry, &extensions)
            .await
            .unwrap();
        assert!(ctx.shared_contains::<ModuleWorkSchedulerHandle>());
        ctx.shared_get::<StopHandle>().unwrap().stop().await;
        let handle = ctx.shared_take::<ModuleWorkSchedulerHandle>().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.join())
            .await
            .unwrap()
            .unwrap();
    }

    struct BlockingWorkRegistration {
        entered: tokio::sync::Notify,
        release: tokio::sync::Semaphore,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl BlockingWorkRegistration {
        fn new() -> Self {
            Self {
                entered: tokio::sync::Notify::new(),
                release: tokio::sync::Semaphore::new(0),
                calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        async fn wait_until_entered(&self) {
            tokio::time::timeout(std::time::Duration::from_secs(2), self.entered.notified())
                .await
                .expect("registration should start");
        }
    }

    #[async_trait::async_trait]
    impl rustok_runtime::ModuleWorkRegistration for BlockingWorkRegistration {
        async fn register(
            &self,
            _host: &rustok_runtime::HostRuntimeContext,
            _scheduler: &rustok_runtime::ModuleWorkScheduler,
        ) -> std::result::Result<(), rustok_runtime::ModuleWorkError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.entered.notify_one();
            self.release
                .acquire()
                .await
                .expect("test semaphore open")
                .forget();
            Ok(())
        }
    }

    async fn blocking_module_work_fixture() -> (
        crate::services::server_runtime_context::ServerRuntimeContext,
        rustok_core::ModuleRegistry,
        rustok_core::ModuleRuntimeExtensions,
        std::sync::Arc<BlockingWorkRegistration>,
    ) {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(
            db,
            RustokSettings::default(),
        );
        let registration = std::sync::Arc::new(BlockingWorkRegistration::new());
        let mut registrations = rustok_runtime::ModuleWorkRegistrations::default();
        registrations.register(registration.clone());
        let mut extensions = rustok_core::ModuleRuntimeExtensions::default();
        extensions.insert(registrations);
        (
            ctx,
            rustok_core::ModuleRegistry::new(),
            extensions,
            registration,
        )
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_module_work_initialization_registers_only_once() {
        use crate::services::app_lifecycle::{ModuleWorkSchedulerHandle, StopHandle};
        use std::future::{Future, poll_fn};
        use std::sync::atomic::Ordering;
        use std::task::Poll;

        let (ctx, registry, extensions, registration) = blocking_module_work_fixture().await;
        let (first_ctx, first_registry, first_extensions) =
            (ctx.clone(), registry.clone(), extensions.clone());
        let first = tokio::spawn(async move {
            super::initialize_module_work_runtime(&first_ctx, &first_registry, &first_extensions)
                .await
        });
        registration.wait_until_entered().await;

        // Poll a second initialization while the first is inside register_all.
        // It must wait for the same reservation rather than enter registration.
        let mut second = Box::pin(super::initialize_module_work_runtime(
            &ctx,
            &registry,
            &extensions,
        ));
        let state = poll_fn(|cx| Poll::Ready(second.as_mut().poll(cx))).await;
        assert!(state.is_pending());
        assert_eq!(registration.calls.load(Ordering::SeqCst), 1);
        registration.release.add_permits(1);
        first.await.unwrap().unwrap();
        second.await.unwrap();
        assert_eq!(registration.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            ctx.shared_map::<ModuleWorkSchedulerHandle, _>(ModuleWorkSchedulerHandle::is_finished),
            Some(false),
        );
        StopHandle::ensure(&ctx).stop().await;
        let handle = ctx.shared_take::<ModuleWorkSchedulerHandle>().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.join())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn cancelled_module_work_initialization_can_be_retried() {
        use crate::services::app_lifecycle::{ModuleWorkSchedulerHandle, StopHandle};
        use std::sync::atomic::Ordering;

        let (ctx, registry, extensions, registration) = blocking_module_work_fixture().await;
        let (first_ctx, first_registry, first_extensions) =
            (ctx.clone(), registry.clone(), extensions.clone());
        let first = tokio::spawn(async move {
            super::initialize_module_work_runtime(&first_ctx, &first_registry, &first_extensions)
                .await
        });
        registration.wait_until_entered().await;
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        assert!(!ctx.shared_contains::<ModuleWorkSchedulerHandle>());
        registration.release.add_permits(1);
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            super::initialize_module_work_runtime(&ctx, &registry, &extensions),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(registration.calls.load(Ordering::SeqCst), 2);
        StopHandle::ensure(&ctx).stop().await;
        let handle = ctx.shared_take::<ModuleWorkSchedulerHandle>().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.join())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn shutdown_during_module_work_registration_prevents_startup() {
        use crate::services::app_lifecycle::{
            ModuleWorkSchedulerHandle, StopHandle, shutdown_runtime_workers,
        };
        use std::future::{Future, poll_fn};
        use std::sync::atomic::Ordering;
        use std::task::Poll;

        let (ctx, registry, extensions, registration) = blocking_module_work_fixture().await;
        let (first_ctx, first_registry, first_extensions) =
            (ctx.clone(), registry.clone(), extensions.clone());
        let first = tokio::spawn(async move {
            super::initialize_module_work_runtime(&first_ctx, &first_registry, &first_extensions)
                .await
        });
        registration.wait_until_entered().await;
        let mut shutdown = Box::pin(shutdown_runtime_workers(&ctx));
        // First poll publishes stop and yields. The second must still be waiting
        // for the initializer's lock rather than completing the drain early.
        let state = poll_fn(|cx| Poll::Ready(shutdown.as_mut().poll(cx))).await;
        assert!(state.is_pending());
        assert!(StopHandle::ensure(&ctx).is_stopping());
        let state = poll_fn(|cx| Poll::Ready(shutdown.as_mut().poll(cx))).await;
        assert!(state.is_pending());
        registration.release.add_permits(1);
        first.await.unwrap().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), shutdown)
            .await
            .unwrap();
        assert!(!ctx.shared_contains::<ModuleWorkSchedulerHandle>());

        // A later bootstrap cannot restart the scheduler in this stopped context.
        super::initialize_module_work_runtime(&ctx, &registry, &extensions)
            .await
            .unwrap();
        assert_eq!(registration.calls.load(Ordering::SeqCst), 1);
        assert!(!ctx.shared_contains::<ModuleWorkSchedulerHandle>());
    }

    #[tokio::test]
    async fn shutdown_deadline_bounds_wait_for_stuck_module_work_registration() {
        use crate::services::app_lifecycle::{ModuleWorkSchedulerHandle, shutdown_runtime_workers};

        let (ctx, registry, extensions, registration) = blocking_module_work_fixture().await;
        let (first_ctx, first_registry, first_extensions) =
            (ctx.clone(), registry.clone(), extensions.clone());
        let first = tokio::spawn(async move {
            super::initialize_module_work_runtime(&first_ctx, &first_registry, &first_extensions)
                .await
        });
        registration.wait_until_entered().await;
        // The callback is held longer than the host drain deadline.
        tokio::time::timeout(
            std::time::Duration::from_secs(8),
            shutdown_runtime_workers(&ctx),
        )
        .await
        .expect("stuck registration must not block shutdown indefinitely");
        assert!(!ctx.shared_contains::<ModuleWorkSchedulerHandle>());
        registration.release.add_permits(1);
        first.await.unwrap().unwrap();
        assert!(!ctx.shared_contains::<ModuleWorkSchedulerHandle>());
    }

    #[tokio::test]
    async fn shutdown_before_module_work_initialization_prevents_startup() {
        use crate::services::app_lifecycle::{
            ModuleWorkSchedulerHandle, ModuleWorkSchedulerRequirement, shutdown_runtime_workers,
        };
        use std::sync::atomic::Ordering;

        let (ctx, registry, extensions, registration) = blocking_module_work_fixture().await;
        shutdown_runtime_workers(&ctx).await;
        super::initialize_module_work_runtime(&ctx, &registry, &extensions)
            .await
            .unwrap();
        assert_eq!(registration.calls.load(Ordering::SeqCst), 0);
        assert!(
            ctx.shared_get::<ModuleWorkSchedulerRequirement>()
                .unwrap()
                .0
        );
        assert!(!ctx.shared_contains::<ModuleWorkSchedulerHandle>());
    }

    #[test]
    fn compiled_surface_contract_rejects_missing_embedded_admin() {
        let contract = DeploymentSurfaceContract {
            profile: DeploymentProfile::ServerWithAdmin,
            embed_admin: true,
            embed_storefront: false,
        };

        let error = validate_compiled_surface_contract(&contract, false, true).unwrap_err();
        assert!(error.to_string().contains("without feature `embed-admin`"));
    }

    #[test]
    fn compiled_surface_contract_rejects_missing_embedded_storefront() {
        let contract = DeploymentSurfaceContract {
            profile: DeploymentProfile::ServerWithStorefront,
            embed_admin: false,
            embed_storefront: true,
        };

        let error = validate_compiled_surface_contract(&contract, true, false).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("without feature `embed-storefront`")
        );
    }

    #[test]
    fn compiled_surface_contract_accepts_matching_features() {
        let contract = DeploymentSurfaceContract {
            profile: DeploymentProfile::Monolith,
            embed_admin: true,
            embed_storefront: true,
        };

        assert!(validate_compiled_surface_contract(&contract, true, true).is_ok());
    }

    #[test]
    fn compiled_surface_contract_allows_headless_profile_without_embedded_ui_features() {
        let contract = DeploymentSurfaceContract {
            profile: DeploymentProfile::HeadlessApi,
            embed_admin: false,
            embed_storefront: false,
        };

        assert!(validate_compiled_surface_contract(&contract, false, false).is_ok());
    }

    #[cfg(feature = "mod-workflow")]
    #[test]
    fn workflow_cron_requires_a_background_worker_runtime_profile() {
        for host_mode in [
            RuntimeHostMode::Api,
            RuntimeHostMode::AdminSsr,
            RuntimeHostMode::StorefrontSsr,
        ] {
            let settings = RustokSettings {
                runtime: RuntimeSettings {
                    host_mode,
                    ..RuntimeSettings::default()
                },
                ..RustokSettings::default()
            };
            assert!(
                !super::workflow_cron_enabled(&settings),
                "workflow cron must stay disabled for {host_mode:?}"
            );
        }

        let settings = RustokSettings {
            runtime: RuntimeSettings {
                host_mode: RuntimeHostMode::Worker,
                background_workers: crate::common::settings::RuntimeBackgroundWorkerSettings {
                    workflow_cron_enabled: true,
                    ..Default::default()
                },
                ..RuntimeSettings::default()
            },
            ..RustokSettings::default()
        };
        assert!(super::workflow_cron_enabled(&settings));
    }

    #[test]
    fn dedicated_auth_rate_limit_covers_verification_requests() {
        assert!(
            AUTH_RATE_LIMIT_PREFIXES
                .iter()
                .any(|prefix| "/api/auth/verify/request".starts_with(prefix))
        );
        assert!(AUTH_RATE_LIMIT_PREFIXES.contains(&"/api/auth/reset"));
        assert!(
            AUTH_RATE_LIMIT_PREFIXES
                .iter()
                .any(|prefix| "/api/auth/invite/accept".starts_with(prefix))
        );
    }

    #[tokio::test]
    async fn bootstrap_registry_only_runtime_forces_headless_surfaces() {
        let settings = RustokSettings {
            runtime: RuntimeSettings {
                host_mode: RuntimeHostMode::RegistryOnly,
                ..RuntimeSettings::default()
            },
            ..RustokSettings::default()
        };

        let db = setup_test_db_with_migrations::<rustok_migrations::SqliteTestMigrator>().await;
        let runtime_ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(
            db,
            settings.clone(),
        );
        let auth_config = crate::auth::auth_config_from_host_settings(
            "test-secret-must-be-at-least-32-bytes-long".to_string(),
            3_600,
            None,
        )
        .expect("test auth configuration should be valid");
        let runtime = super::bootstrap_app_runtime(runtime_ctx, auth_config)
            .await
            .expect("registry-only runtime should bootstrap");

        assert_eq!(
            runtime.deployment_surfaces.profile,
            DeploymentProfile::HeadlessApi
        );
        assert!(!runtime.deployment_surfaces.embed_admin);
        assert!(!runtime.deployment_surfaces.embed_storefront);
    }
}
