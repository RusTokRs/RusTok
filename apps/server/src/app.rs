use async_trait::async_trait;
use axum::middleware as axum_middleware;
#[cfg(feature = "embed-admin-assets")]
use axum::response::IntoResponse;
use axum::Extension;
use axum::Router as AxumRouter;
use loco_rs::{
    app::{AppContext, Hooks, Initializer},
    boot::{create_app, BootResult, StartMode},
    config::Config,
    controller::AppRoutes,
    environment::Environment,
    task::Tasks,
    Result,
};
use std::path::Path;

use sea_orm::EntityTrait;

use crate::controllers;
use crate::initializers;
use crate::middleware;
use crate::middleware::rate_limit::{
    cleanup_task, rate_limit_for_paths, RateLimitConfig, RateLimiter,
};
use crate::modules;
use crate::seeds;
use crate::services::event_transport_factory::{
    build_event_runtime, spawn_outbox_relay_worker, EventRuntime,
};
use crate::services::index_dispatcher::spawn_index_dispatcher;
use crate::services::marketplace_catalog::{
    MarketplaceCatalogService, SharedMarketplaceCatalogService,
};
use crate::tasks;
use loco_rs::prelude::Queue;
use migration::Migrator;
#[cfg(feature = "embed-admin-assets")]
use rust_embed::RustEmbed;
use std::sync::Arc;

#[cfg(feature = "embed-admin-assets")]
#[derive(RustEmbed)]
#[folder = "../../apps/admin/dist"]
struct AdminAssets;

#[cfg(feature = "embed-admin-assets")]
fn admin_router() -> AxumRouter {
    AxumRouter::new().fallback(move |path: axum::extract::Path<String>| async move {
        let path = path.0.trim_start_matches('/');
        let path = if path.is_empty() { "index.html" } else { path };

        match AdminAssets::get(path) {
            Some(content) => {
                let mime = mime_guess::from_path(path).first_or_octet_stream();
                (
                    [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                    content.data,
                )
                    .into_response()
            }
            None => match AdminAssets::get("index.html") {
                Some(content) => (
                    [(axum::http::header::CONTENT_TYPE, "text/html")],
                    content.data,
                )
                    .into_response(),
                None => (axum::http::StatusCode::NOT_FOUND, "Admin UI not bundled").into_response(),
            },
        }
    })
}

#[cfg(not(feature = "embed-admin-assets"))]
fn admin_router() -> AxumRouter {
    AxumRouter::new().fallback(|| async {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Admin UI is disabled. Rebuild server with feature `embed-admin-assets` and prepare apps/admin/dist artifacts.",
        )
    })
}

pub struct App;

#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn app_version() -> String {
        format!(
            "{} ({})",
            env!("CARGO_PKG_VERSION"),
            option_env!("BUILD_SHA")
                .or(option_env!("GITHUB_SHA"))
                .unwrap_or("dev")
        )
    }

    async fn boot(
        mode: StartMode,
        environment: &Environment,
        mut config: Config,
    ) -> Result<BootResult> {
        if std::env::var("DATABASE_URL").is_err()
            && (config.database.uri.is_empty()
                || config.database.uri.contains("localhost:5432")
                || config.database.uri.contains("db:5432"))
        {
            config.database.uri = "sqlite://rustok.sqlite?mode=rwc".to_string();
            tracing::info!(
                "No external database found. Falling back to local SQLite: {}",
                config.database.uri
            );
        }

        create_app::<Self, Migrator>(mode, environment, config).await
    }

    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::with_default_routes()
            .add_route(controllers::health::routes())
            .add_route(controllers::metrics::routes())
            .add_route(controllers::swagger::routes())
            .add_route(controllers::admin_events::routes())
            .add_route(controllers::auth::routes())
            .add_route(controllers::graphql::routes())
            .add_route(controllers::oauth::routes())
            .add_route(controllers::oauth_metadata::routes())
            .add_route(controllers::commerce::routes())
            .add_route(controllers::content::routes())
            .add_route(controllers::blog::routes())
            .add_route(controllers::forum::routes())
            .add_route(controllers::pages::routes())
    }

    async fn after_routes(router: AxumRouter, ctx: &AppContext) -> Result<AxumRouter> {
        let event_runtime = build_event_runtime(ctx).await?;
        ctx.shared_store.insert(event_runtime.transport.clone());
        spawn_index_dispatcher(ctx);
        ctx.shared_store.insert(Arc::new(event_runtime));
        let marketplace_catalog = Arc::new(MarketplaceCatalogService::evolutionary_defaults());
        tracing::info!(
            providers = ?marketplace_catalog.provider_keys(),
            "Initialized evolutionary marketplace catalog provider chain"
        );
        ctx.shared_store
            .insert(SharedMarketplaceCatalogService(marketplace_catalog));
        let registry = modules::build_registry();
        modules::validate_registry_vs_manifest(&registry)?;
        middleware::tenant::init_tenant_cache_infrastructure(ctx).await;
        let engine = Arc::new(alloy_scripting::create_default_engine());
        let storage = Arc::new(alloy_scripting::SeaOrmStorage::new(ctx.db.clone()));
        let orchestrator = Arc::new(alloy_scripting::ScriptOrchestrator::new(
            engine.clone(),
            storage.clone(),
        ));
        let execution_log = Arc::new(alloy_scripting::SeaOrmExecutionLog::new(ctx.db.clone()));
        let alloy_state = crate::graphql::alloy::AlloyState::new(
            engine.clone(),
            storage.clone(),
            orchestrator.clone(),
            execution_log,
        );

        let executor = alloy_scripting::ScriptExecutor::new(engine.clone(), storage.clone());
        let scheduler = Arc::new(alloy_scripting::Scheduler::new(executor, storage.clone()));
        tokio::spawn(async move {
            if let Err(err) = scheduler.load_jobs().await {
                tracing::warn!("Failed to load scheduler jobs: {}", err);
            }
            scheduler.start().await;
        });

        let alloy_app_state = Arc::new(alloy_scripting::AppState {
            registry: storage,
            orchestrator,
            engine,
        });
        let alloy_rest_router = controllers::alloy::router(alloy_app_state);

        // Global API rate limiter: 300 req/min per IP for all /api/* endpoints
        let api_limiter = Arc::new(RateLimiter::new(RateLimitConfig::new(300, 60)));
        let api_limiter_for_cleanup = api_limiter.clone();
        tokio::spawn(async move {
            cleanup_task(api_limiter_for_cleanup).await;
        });
        let api_prefixes = Arc::new(vec!["/api/"]);

        let auth_limiter = Arc::new(RateLimiter::new(RateLimitConfig::new(20, 60)));
        let auth_limiter_for_cleanup = auth_limiter.clone();
        tokio::spawn(async move {
            cleanup_task(auth_limiter_for_cleanup).await;
        });

        let auth_prefixes = Arc::new(vec![
            "/api/auth/login",
            "/api/auth/register",
            "/api/auth/reset",
        ]);

        let admin_router = admin_router();

        let storefront_router = rustok_storefront::router();

        Ok(router
            .nest("/api/alloy", alloy_rest_router)
            .nest("/admin", admin_router)
            .nest("/", storefront_router)
            .layer(Extension(registry))
            .layer(Extension(alloy_state))
            .layer(axum_middleware::from_fn_with_state(
                (auth_limiter, auth_prefixes),
                rate_limit_for_paths,
            ))
            .layer(axum_middleware::from_fn_with_state(
                (api_limiter, api_prefixes),
                rate_limit_for_paths,
            ))
            .layer(axum_middleware::from_fn_with_state(
                ctx.clone(),
                middleware::tenant::resolve,
            ))
            .layer(axum_middleware::from_fn(
                middleware::security_headers::security_headers,
            )))
    }

    async fn truncate(ctx: &AppContext) -> Result<()> {
        tracing::info!("Truncating database...");

        let releases = crate::models::release::Entity::delete_many()
            .exec(&ctx.db)
            .await?;
        let builds = crate::models::build::Entity::delete_many()
            .exec(&ctx.db)
            .await?;
        let tenant_modules = crate::models::_entities::tenant_modules::Entity::delete_many()
            .exec(&ctx.db)
            .await?;
        let sessions = crate::models::sessions::Entity::delete_many()
            .exec(&ctx.db)
            .await?;
        let users = crate::models::users::Entity::delete_many()
            .exec(&ctx.db)
            .await?;
        let tenants = crate::models::tenants::Entity::delete_many()
            .exec(&ctx.db)
            .await?;

        tracing::info!(
            releases = releases.rows_affected,
            builds = builds.rows_affected,
            tenant_modules = tenant_modules.rows_affected,
            sessions = sessions.rows_affected,
            users = users.rows_affected,
            tenants = tenants.rows_affected,
            "Database truncation complete"
        );
        Ok(())
    }

    fn register_tasks(tasks: &mut Tasks) {
        tasks::register(tasks);
    }

    async fn initializers(ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
        initializers::create(ctx).await
    }

    async fn connect_workers(ctx: &AppContext, _queue: &Queue) -> Result<()> {
        let event_runtime = ctx
            .shared_store
            .get::<Arc<EventRuntime>>()
            .ok_or_else(|| loco_rs::Error::Message("EventRuntime not initialized".to_string()))?;

        if let Some(relay_config) = event_runtime.relay_config.clone() {
            let handle = spawn_outbox_relay_worker(relay_config);
            ctx.shared_store.insert(Arc::new(handle));
        }

        Ok(())
    }

    async fn seed(ctx: &AppContext, path: &Path) -> Result<()> {
        seeds::seed(ctx, path).await
    }
}
