use async_trait::async_trait;
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

use crate::common::settings::RustokSettings;
use crate::controllers;
use crate::initializers;
use crate::seeds;
use crate::services::app_lifecycle::{apply_boot_database_fallback, connect_runtime_workers};
use crate::services::app_router::compose_application_router;
use crate::services::app_runtime::bootstrap_app_runtime;
use crate::tasks;
use loco_rs::prelude::Queue;
use migration::Migrator;

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
        if apply_boot_database_fallback(&mut config) {
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
        let rustok_settings =
            RustokSettings::from_settings(&ctx.config.settings).map_err(|error| {
                loco_rs::Error::BadRequest(format!("Invalid rustok settings: {error}"))
            })?;
        let runtime = bootstrap_app_runtime(ctx, &rustok_settings).await?;
        connect_runtime_workers(ctx).await?;

        Ok(compose_application_router(router, ctx, runtime))
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

    async fn connect_workers(_ctx: &AppContext, _queue: &Queue) -> Result<()> {
        // Workers are started in after_routes where the full runtime is available.
        Ok(())
    }

    async fn seed(ctx: &AppContext, path: &Path) -> Result<()> {
        seeds::seed(ctx, path).await
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use loco_rs::{app::Hooks, tests_cfg::app::get_app_context};
    use serial_test::serial;
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::graphql::SharedGraphqlSchema;
    use crate::middleware::rate_limit::{
        SharedApiRateLimiter, SharedAuthRateLimiter, SharedOAuthRateLimiter,
    };
    use crate::services::event_transport_factory::EventRuntime;
    use crate::services::marketplace_catalog::SharedMarketplaceCatalogService;

    #[tokio::test]
    #[serial]
    async fn startup_smoke_builds_router_and_runtime_shared_state() {
        let mut ctx = get_app_context().await;
        ctx.config.settings = Some(serde_json::json!({
            "rustok": {
                "events": {
                    "transport": "memory"
                },
                "rate_limit": {
                    "enabled": false
                }
            }
        }));

        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let app = <App as Hooks>::after_routes(base_router, &ctx)
            .await
            .expect("after_routes should wire runtime");

        assert!(ctx.shared_store.contains::<Arc<EventRuntime>>());
        assert!(ctx
            .shared_store
            .contains::<SharedMarketplaceCatalogService>());
        assert!(ctx.shared_store.contains::<SharedGraphqlSchema>());
        assert!(ctx.shared_store.contains::<SharedApiRateLimiter>());
        assert!(ctx.shared_store.contains::<SharedAuthRateLimiter>());
        assert!(ctx.shared_store.contains::<SharedOAuthRateLimiter>());

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health/live")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("health/live request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("health/live body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected /health/live response body: {}",
            String::from_utf8_lossy(&body)
        );
    }
}
