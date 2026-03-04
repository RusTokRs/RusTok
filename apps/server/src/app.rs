use async_trait::async_trait;
use axum::middleware as axum_middleware;
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
use crate::tasks;
use loco_rs::prelude::Queue;
use migration::Migrator;
use std::sync::Arc;

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
        config: Config,
    ) -> Result<BootResult> {
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

        let auth_limiter_for_middleware = auth_limiter.clone();
        let auth_rate_limit_middleware = axum_middleware::from_fn(
            move |request: axum::extract::Request, next: axum_middleware::Next| {
                let limiter = auth_limiter_for_middleware.clone();
                async move {
                    use axum::body::Body;

                    let path = request.uri().path().to_owned();
                    let is_auth_path = path.starts_with("/api/auth/login")
                        || path.starts_with("/api/auth/register")
                        || path.starts_with("/api/auth/reset");

                    if !is_auth_path {
                        return next.run(request).await;
                    }

                    let headers = request.headers().clone();
                    let client_id = crate::middleware::rate_limit::extract_client_id_pub(&headers);

                    match limiter.check_rate_limit(&client_id).await {
                        Ok(info) => {
                            let mut response = next.run(request).await;
                            let resp_headers = response.headers_mut();
                            if let Ok(v) =
                                axum::http::HeaderValue::from_str(&info.limit.to_string())
                            {
                                resp_headers.insert("x-ratelimit-limit", v);
                            }
                            if let Ok(v) =
                                axum::http::HeaderValue::from_str(&info.remaining.to_string())
                            {
                                resp_headers.insert("x-ratelimit-remaining", v);
                            }
                            if let Ok(v) =
                                axum::http::HeaderValue::from_str(&info.reset.to_string())
                            {
                                resp_headers.insert("x-ratelimit-reset", v);
                            }
                            response
                        }
                        Err(status) => {
                            let mut response =
                                axum::response::Response::new(Body::from("Rate limit exceeded"));
                            *response.status_mut() = status;
                            if let Ok(v) = axum::http::HeaderValue::from_str(
                                &limiter.window_secs().to_string(),
                            ) {
                                response.headers_mut().insert("retry-after", v);
                            }
                            response
                        }
                    }
                }
            },
        );

        Ok(router
            .nest("/api/alloy", alloy_rest_router)
            .layer(Extension(registry))
            .layer(Extension(alloy_state))
            .layer(auth_rate_limit_middleware)
            .layer(axum_middleware::from_fn_with_state(
                (api_limiter, api_prefixes),
                rate_limit_for_paths,
            ))
            .layer(axum_middleware::from_fn_with_state(
                ctx.clone(),
                middleware::tenant::resolve,
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
