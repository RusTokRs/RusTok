use std::sync::Arc;

use crate::error::{Error, Result};
use rustok_core::ModuleRegistry;

use crate::auth::AuthConfig;
use crate::common::settings::{RuntimeHostMode, RustokSettings, SharedRustokSettings};
use crate::graphql::AppSchema;
use crate::middleware;
use crate::middleware::rate_limit::{
    cleanup_task, PathRateLimitMiddlewareState, PathRateLimitPolicy, RateLimitConfig, RateLimiter,
    SharedApiRateLimiter, SharedAuthRateLimiter, SharedOAuthRateLimiter, SharedSearchRateLimiter,
};
use crate::modules;
use crate::modules::{DeploymentSurfaceContract, ManifestManager};
use crate::services::cache_runtime::ensure_cache_service;
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::event_transport_factory::build_event_runtime;
use crate::services::graphql_schema::init_graphql_schema;
use crate::services::marketplace_catalog::{
    LocalManifestMarketplaceProvider, MarketplaceCatalogService, SharedMarketplaceCatalogService,
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
    settings: &RustokSettings,
) -> Result<AppRuntimeBootstrap> {
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
        settings,
        runtime_ctx.clone(),
        auth_config.clone(),
    )?;
    runtime_ctx.shared_insert(runtime_extensions.clone());
    runtime_ctx.shared_insert(registry.clone());
    ManifestManager::validate(&manifest)
        .and_then(|_| ManifestManager::validate_with_registry(&manifest, &registry))
        .map_err(|error| Error::BadRequest(format!("modules.toml validation failed: {error}")))?;
    if !settings.runtime.is_registry_only() {
        let event_runtime = build_event_runtime(&runtime_ctx).await?;
        runtime_ctx.shared_insert(event_runtime.transport.clone());
        spawn_module_event_dispatcher(&runtime_ctx, &registry, runtime_extensions.clone());
        runtime_ctx.shared_insert(Arc::new(event_runtime));
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
        middleware::tenant::init_tenant_cache_infrastructure(&runtime_ctx, &cache_service).await;
        runtime_ctx.shared_insert(
            rustok_content_orchestration::build_content_orchestration_service(
                runtime_ctx.db_clone(),
                transactional_event_bus_from_context(&runtime_ctx),
            ),
        );

        #[cfg(feature = "mod-workflow")]
        if settings.runtime.background_workers.workflow_cron_enabled {
            init_workflow_runtime(&runtime_ctx);
        } else {
            tracing::info!("Workflow cron scheduler disabled by runtime.background_workers config");
        }

        init_alloy_runtime(&runtime_ctx);
    }

    if settings.runtime.is_registry_only() {
        use rustok_core::events::MemoryTransport;

        // Registry-only mode does not bootstrap full event runtime, but
        // GraphQL schema construction still expects an EventTransport in shared_store.
        // Seed a local memory transport to keep shared initialization deterministic
        // for tests and non-GraphQL surfaces.
        if runtime_ctx
            .shared_get::<std::sync::Arc<dyn rustok_core::events::EventTransport>>()
            .is_none()
        {
            runtime_ctx.shared_insert(std::sync::Arc::new(MemoryTransport::new())
                as std::sync::Arc<dyn rustok_core::events::EventTransport>);
        }
    }

    initialize_module_work_runtime(&runtime_ctx, &registry, runtime_extensions.as_ref()).await?;

    let graphql_schema = init_graphql_schema(&runtime_ctx);
    let rate_limits =
        init_rate_limit_layers(&runtime_ctx, settings, &cache_service, Some(auth_config))?;

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
    let host = extensions.apply_to_host_runtime(
        rustok_api::HostRuntimeContext::new(ctx.db_clone())
            .with_shared_value(transactional_event_bus_from_context(ctx))
            .with_shared_value(registry.clone()),
    );
    let host = if let Some(storage) = ctx.shared_get::<rustok_storage::StorageService>() {
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
    let host = if ctx.shared_get::<rustok_storage::StorageService>().is_some() {
        let executor = ctx
            .shared_get::<rustok_modules::SharedArtifactBindingExecutor>()
            .unwrap_or(crate::services::artifact_runtime::compose_artifact_binding_executor(ctx)?);
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
    let Some(registrations) = extensions.get::<rustok_runtime::ModuleWorkRegistrations>() else {
        return Ok(());
    };
    if registrations.is_empty() || !ctx.settings().runtime.runs_background_workers() {
        return Ok(());
    }
    let scheduler = rustok_runtime::ModuleWorkScheduler::new();
    registrations
        .register_all(&host, &scheduler)
        .await
        .map_err(|error| Error::Message(format!("module work registration failed: {error}")))?;
    if !ctx.shared_contains::<crate::services::app_lifecycle::StopHandle>() {
        let (stop_handle, _stop_rx) = crate::services::app_lifecycle::StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop = ctx
        .shared_get::<crate::services::app_lifecycle::StopHandle>()
        .expect("StopHandle must be registered before module work startup")
        .subscribe();
    tokio::spawn(async move {
        scheduler
            .run_until_stopped(stop, std::time::Duration::from_secs(1))
            .await;
    });
    Ok(())
}

pub fn module_runtime_extensions_from_ctx(
    ctx: &ServerRuntimeContext,
) -> Arc<ModuleRuntimeExtensions> {
    ctx.shared_get::<Arc<ModuleRuntimeExtensions>>()
        .expect("ModuleRuntimeExtensions not initialized; bootstrap_app_runtime must run first")
}
