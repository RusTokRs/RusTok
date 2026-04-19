use std::sync::Arc;

use loco_rs::app::AppContext;

use crate::error::{Error, Result};
use rustok_core::ModuleRegistry;

use crate::auth::auth_config_from_ctx;
use crate::common::settings::{RustokSettings, SharedRustokSettings};
use crate::graphql::AppSchema;
use crate::middleware;
use crate::middleware::rate_limit::{
    cleanup_task, PathRateLimitMiddlewareState, PathRateLimitPolicy, RateLimitConfig, RateLimiter,
    SharedApiRateLimiter, SharedAuthRateLimiter, SharedOAuthRateLimiter, SharedSearchRateLimiter,
};
use crate::modules;
use crate::modules::{DeploymentSurfaceContract, ManifestManager};
use crate::services::content_orchestration::init_content_orchestration;
use crate::services::event_transport_factory::build_event_runtime;
use crate::services::graphql_schema::init_graphql_schema;
use crate::services::marketplace_catalog::{
    MarketplaceCatalogService, SharedMarketplaceCatalogService,
};
use crate::services::module_event_dispatcher::spawn_module_event_dispatcher;
use crate::services::oauth_app::sync_manifest_managed_apps_for_all_tenants;
use rustok_cache::CacheService;

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
    ctx: &AppContext,
    settings: &RustokSettings,
) -> Result<AppRuntimeBootstrap> {
    let cache_service = CacheService::from_url(settings.cache.redis_url.as_deref());
    ctx.shared_store.insert(cache_service.clone());

    // Cache parsed settings so per-request middleware avoids repeated JSON deserialization.
    ctx.shared_store
        .insert(SharedRustokSettings(Arc::new(settings.clone())));

    init_marketplace_catalog(ctx);

    let manifest = ManifestManager::load()
        .map_err(|error| Error::BadRequest(format!("modules.toml validation failed: {error}")))?;
    let deployment_surfaces = if settings.runtime.is_registry_only() {
        DeploymentSurfaceContract {
            profile: crate::models::build::DeploymentProfile::HeadlessApi,
            embed_admin: false,
            embed_storefront: false,
        }
    } else {
        let deployment_surfaces = ManifestManager::deployment_surface_contract(&manifest);
        validate_compiled_surface_contract(
            &deployment_surfaces,
            cfg!(feature = "embed-admin"),
            cfg!(feature = "embed-storefront"),
        )?;
        deployment_surfaces
    };

    let registry = modules::build_registry();
    ctx.shared_store
        .insert(rustok_ai::SharedAiModuleRegistry(registry.clone()));
    ManifestManager::validate(&manifest)
        .and_then(|_| ManifestManager::validate_with_registry(&manifest, &registry))
        .map_err(|error| Error::BadRequest(format!("modules.toml validation failed: {error}")))?;
    if !settings.runtime.is_registry_only() {
        let event_runtime = build_event_runtime(ctx).await?;
        ctx.shared_store.insert(event_runtime.transport.clone());
        spawn_module_event_dispatcher(ctx, settings, &registry);
        ctx.shared_store.insert(Arc::new(event_runtime));
        ctx.shared_store
            .insert(crate::services::mcp_runtime::DbBackedMcpRuntimeBridge::shared(ctx.db.clone()));
        sync_manifest_managed_apps_for_all_tenants(&ctx.db, &manifest)
            .await
            .map_err(|error| {
                Error::Message(format!(
                    "Failed to sync manifest-managed OAuth apps: {error}"
                ))
            })?;
        middleware::tenant::init_tenant_cache_infrastructure(ctx, &cache_service).await;
        init_content_orchestration(ctx);

        init_storage(ctx, settings).await?;

        #[cfg(feature = "mod-workflow")]
        init_workflow_runtime(ctx);

        init_alloy_runtime(ctx);
    }

    let graphql_schema = init_graphql_schema(ctx);
    let rate_limits = init_rate_limit_layers(ctx, settings, &cache_service)?;

    Ok(AppRuntimeBootstrap {
        deployment_surfaces,
        registry,
        graphql_schema,
        rate_limit_state: rate_limits.combined_state,
    })
}

async fn init_storage(ctx: &AppContext, settings: &RustokSettings) -> Result<()> {
    use rustok_storage::StorageService;

    let service = StorageService::from_config(&settings.storage)
        .await
        .map_err(|error| {
            Error::Message(format!("Failed to initialize storage backend: {error}"))
        })?;
    tracing::info!(driver = ?settings.storage.driver, "Initialized storage backend");
    ctx.shared_store.insert(service);
    Ok(())
}

fn init_marketplace_catalog(ctx: &AppContext) {
    let marketplace_catalog = Arc::new(MarketplaceCatalogService::evolutionary_defaults());
    tracing::info!(
        providers = ?marketplace_catalog.provider_keys(),
        "Initialized evolutionary marketplace catalog provider chain"
    );
    ctx.shared_store
        .insert(SharedMarketplaceCatalogService(marketplace_catalog));
}

fn init_alloy_runtime(ctx: &AppContext) {
    #[cfg(feature = "mod-alloy")]
    {
        alloy::init(ctx);
    }
}

#[cfg(feature = "mod-workflow")]
fn init_workflow_runtime(ctx: &AppContext) {
    use rustok_workflow::WorkflowCronScheduler;
    let db = ctx.db.clone();

    // Start the cron scheduler
    let scheduler = WorkflowCronScheduler::new(db);
    let handle = scheduler.start();
    tokio::spawn(async move {
        if let Err(error) = handle.await {
            tracing::error!("Workflow cron scheduler panicked: {:?}", error);
        }
    });

    tracing::info!("Workflow runtime initialized (cron scheduler)");
}

struct RateLimitLayers {
    combined_state: PathRateLimitMiddlewareState,
}

fn init_rate_limit_layers(
    ctx: &AppContext,
    settings: &RustokSettings,
    cache_service: &CacheService,
) -> Result<RateLimitLayers> {
    let auth_config = auth_config_from_ctx(ctx).ok();
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
                    prefixes: Arc::new(vec![
                        "/api/auth/login",
                        "/api/auth/register",
                        "/api/auth/reset",
                    ]),
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
    ctx: &AppContext,
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
        SharedLimiterNamespace::Api => ctx
            .shared_store
            .insert(SharedApiRateLimiter(limiter.clone())),
        SharedLimiterNamespace::Auth => ctx
            .shared_store
            .insert(SharedAuthRateLimiter(limiter.clone())),
        SharedLimiterNamespace::Oauth => ctx
            .shared_store
            .insert(SharedOAuthRateLimiter(limiter.clone())),
        SharedLimiterNamespace::Search => ctx
            .shared_store
            .insert(SharedSearchRateLimiter(limiter.clone())),
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
    use super::validate_compiled_surface_contract;
    use crate::common::settings::{RuntimeHostMode, RuntimeSettings, RustokSettings};
    use crate::models::build::DeploymentProfile;
    use crate::modules::DeploymentSurfaceContract;
    use loco_rs::tests_cfg::app::get_app_context;

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
        assert!(error
            .to_string()
            .contains("without feature `embed-storefront`"));
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

    #[tokio::test]
    async fn bootstrap_registry_only_runtime_forces_headless_surfaces() {
        let ctx = get_app_context().await;
        let mut settings = RustokSettings::default();
        settings.runtime = RuntimeSettings {
            host_mode: RuntimeHostMode::RegistryOnly,
            ..RuntimeSettings::default()
        };

        let runtime = super::bootstrap_app_runtime(&ctx, &settings)
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
