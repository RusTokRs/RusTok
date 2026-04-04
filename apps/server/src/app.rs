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

use crate::channels;
use crate::common::settings::{EmailProvider, RustokSettings};
use crate::controllers;
use crate::initializers;
use crate::seeds;
use crate::services::app_lifecycle::{apply_boot_database_fallback, connect_runtime_workers};
use crate::services::app_router::compose_application_router;
use crate::services::app_runtime::bootstrap_app_runtime;
use crate::tasks;
use loco_rs::prelude::Queue;

use crate::error::Error;
use migration::Migrator;

mod routes_codegen {
    include!(concat!(env!("OUT_DIR"), "/app_routes_codegen.rs"));
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
        if apply_boot_database_fallback(&mut config) {
            tracing::info!(
                "No external database found. Falling back to local SQLite: {}",
                config.database.uri
            );
        }

        create_app::<Self, Migrator>(mode, environment, config).await
    }

    async fn after_context(mut ctx: AppContext) -> Result<AppContext> {
        // Initialise Loco's ctx.mailer when email.provider = "loco".
        // This must happen before after_routes so every request handler
        // can call email_service_from_ctx() and get a working Loco mailer.
        if let Ok(settings) = RustokSettings::from_settings(&ctx.config.settings) {
            if settings.email.provider == EmailProvider::Loco {
                match loco_rs::mailer::EmailSender::smtp(&loco_rs::config::SmtpMailer {
                    enable: settings.email.enabled,
                    host: settings.email.smtp.host,
                    port: settings.email.smtp.port,
                    secure: settings.email.smtp.port == 465,
                    auth: if settings.email.smtp.username.is_empty() {
                        None
                    } else {
                        Some(loco_rs::config::MailerAuth {
                            user: settings.email.smtp.username,
                            password: settings.email.smtp.password,
                        })
                    },
                    hello_name: None,
                }) {
                    Ok(sender) => {
                        ctx.mailer = Some(sender);
                        tracing::info!("Loco Mailer initialised from rustok email settings");
                    }
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            "Failed to initialise Loco Mailer; emails will be disabled"
                        );
                    }
                }
            }
        }
        Ok(ctx)
    }

    fn routes(_ctx: &AppContext) -> AppRoutes {
        let registry_only = _ctx
            .config
            .settings
            .as_ref()
            .and_then(|_| RustokSettings::from_settings(&_ctx.config.settings).ok())
            .is_some_and(|settings| settings.runtime.is_registry_only());

        let routes = if registry_only {
            AppRoutes::with_default_routes()
                .add_route(controllers::health::routes())
                .add_route(controllers::marketplace_registry::read_only_routes())
                .add_route(controllers::metrics::routes())
                .add_route(controllers::swagger::routes())
        } else {
            AppRoutes::with_default_routes()
                .add_route(controllers::health::routes())
                .add_route(controllers::marketplace_registry::routes())
                .add_route(controllers::metrics::routes())
                .add_route(controllers::swagger::routes())
                .add_route(controllers::admin_events::routes())
                .add_route(controllers::auth::routes())
                .add_route(controllers::channel::routes())
                .add_route(controllers::graphql::routes())
                .add_route(controllers::mcp::routes())
                .add_route(controllers::oauth::routes())
                .add_route(controllers::oauth_metadata::routes())
                .add_route(controllers::users::routes())
        };

        let mut routes = if registry_only {
            routes
        } else {
            routes_codegen::append_optional_module_routes(routes)
        };

        if !registry_only {
            routes = routes.add_route(channels::builds::routes());
        }

        routes
    }

    async fn after_routes(router: AxumRouter, ctx: &AppContext) -> Result<AxumRouter> {
        let rustok_settings = RustokSettings::from_settings(&ctx.config.settings)
            .map_err(|error| Error::BadRequest(format!("Invalid rustok settings: {error}")))?;
        let runtime = bootstrap_app_runtime(ctx, &rustok_settings).await?;
        connect_runtime_workers(ctx).await?;

        Ok(compose_application_router(
            router,
            ctx,
            runtime,
            &rustok_settings,
        ))
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

    /// Graceful shutdown: stop background workers and flush telemetry.
    async fn on_shutdown(ctx: &AppContext) {
        use crate::services::app_lifecycle::StopHandle;

        if let Some(handle) = ctx.shared_store.get::<StopHandle>() {
            tracing::info!("Stopping background workers…");
            handle.stop().await;
        }

        tracing::info!("RusTok server shut down cleanly");
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use loco_rs::{app::Hooks, tests_cfg::app::get_app_context};
    use migration::Migrator;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
    use sea_orm_migration::MigratorTrait;
    use serde_json::Value;
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
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for startup smoke");
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

    #[tokio::test]
    #[serial]
    async fn registry_catalog_endpoint_serves_v1_contract() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry catalog smoke");
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
        let response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/catalog")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("v1 catalog request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("v1 catalog body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected /v1/catalog response body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: Value =
            serde_json::from_slice(&body).expect("v1 catalog payload should be valid json");
        let modules = payload["modules"]
            .as_array()
            .expect("v1 catalog should return modules array");

        assert_eq!(payload["schema_version"], 1);
        assert!(
            !modules.is_empty(),
            "v1 catalog should expose first-party modules"
        );
        assert!(modules.iter().all(|module| module["source"] == "registry"));
        assert!(modules
            .iter()
            .all(|module| module["ownership"] == "first_party"));
    }

    #[tokio::test]
    #[serial]
    async fn registry_catalog_detail_endpoint_serves_module_contract() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry catalog detail smoke");
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
        let response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/catalog/blog")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("v1 catalog detail request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("v1 catalog detail body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected /v1/catalog/blog response body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: Value =
            serde_json::from_slice(&body).expect("v1 catalog detail payload should be valid json");

        assert_eq!(payload["slug"], "blog");
        assert_eq!(payload["source"], "registry");
        assert_eq!(payload["ownership"], "first_party");
    }

    #[tokio::test]
    #[serial]
    async fn registry_catalog_endpoint_supports_query_filters() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry catalog filter smoke");
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
        let response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/catalog?search=blog")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("v1 catalog filtered request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("v1 catalog filtered body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected /v1/catalog?search=blog response body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: Value = serde_json::from_slice(&body)
            .expect("v1 catalog filtered payload should be valid json");
        let modules = payload["modules"]
            .as_array()
            .expect("v1 catalog filtered response should return modules array");

        assert!(
            !modules.is_empty(),
            "filtered v1 catalog should not be empty"
        );
        assert!(modules.iter().all(|module| {
            module["slug"]
                .as_str()
                .is_some_and(|slug| slug.eq_ignore_ascii_case("blog"))
        }));
    }

    #[tokio::test]
    #[serial]
    async fn registry_catalog_endpoint_supports_limit_and_offset() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry catalog pagination smoke");
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
        let response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/catalog?limit=1&offset=1")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("v1 catalog paged request should succeed");
        let status = response.status();
        let total_count = response
            .headers()
            .get("x-total-count")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok())
            .expect("v1 catalog paged response should include x-total-count header");
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("v1 catalog paged body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected /v1/catalog?limit=1&offset=1 response body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: Value =
            serde_json::from_slice(&body).expect("v1 catalog paged payload should be valid json");
        let modules = payload["modules"]
            .as_array()
            .expect("v1 catalog paged response should return modules array");

        assert_eq!(modules.len(), 1, "paged v1 catalog should honor limit=1");
        assert!(
            total_count >= modules.len(),
            "x-total-count should describe the full filtered collection"
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_catalog_endpoint_is_sorted_by_slug_for_stable_paging() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry catalog sort smoke");
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
        let response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/catalog")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("v1 catalog sorted request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("v1 catalog sorted body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected /v1/catalog sorted response body: {}",
            String::from_utf8_lossy(&body)
        );

        let payload: Value =
            serde_json::from_slice(&body).expect("v1 catalog sorted payload should be valid json");
        let modules = payload["modules"]
            .as_array()
            .expect("v1 catalog sorted response should return modules array");
        let slugs = modules
            .iter()
            .filter_map(|module| module["slug"].as_str())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let mut sorted_slugs = slugs.clone();
        sorted_slugs.sort();

        assert_eq!(
            slugs, sorted_slugs,
            "v1 catalog should use stable slug ordering for pagination"
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_catalog_endpoint_honors_if_none_match() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry cache smoke");
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
        let first_response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/catalog?limit=1")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("initial v1 catalog cache request should succeed");
        let first_status = first_response.status();
        let etag = first_response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
            .expect("initial v1 catalog cache response should include etag");
        let total_count = first_response
            .headers()
            .get("x-total-count")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
            .expect("initial v1 catalog cache response should include x-total-count");
        assert_eq!(first_status, StatusCode::OK);

        let second_response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/catalog?limit=1")
                    .header("if-none-match", etag.as_str())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("conditional v1 catalog request should succeed");
        let second_status = second_response.status();
        let second_etag = second_response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .expect("conditional v1 catalog response should keep etag");
        let second_etag = second_etag.to_string();
        let second_total_count = second_response
            .headers()
            .get("x-total-count")
            .and_then(|value| value.to_str().ok())
            .expect("conditional v1 catalog response should keep x-total-count");
        let second_total_count = second_total_count.to_string();
        let second_cache_control = second_response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok())
            .expect("conditional v1 catalog response should keep cache-control");
        let second_cache_control = second_cache_control.to_string();
        let second_body = to_bytes(second_response.into_body(), usize::MAX)
            .await
            .expect("conditional v1 catalog body should read");

        assert_eq!(second_status, StatusCode::NOT_MODIFIED);
        assert_eq!(second_etag, etag);
        assert_eq!(second_total_count, total_count);
        assert_eq!(second_cache_control, "public, max-age=60");
        assert!(
            second_body.is_empty(),
            "304 response should not include catalog body"
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_publish_endpoint_accepts_dry_run_contract() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry publish smoke");
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
        let response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v2/catalog/publish")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": true,
                            "module": {
                                "slug": "blog",
                                "version": "0.1.0",
                                "crate_name": "rustok-blog",
                                "name": "Blog",
                                "description": "Blog and news module contract preview.",
                                "ownership": "first_party",
                                "trust_level": "verified",
                                "license": "MIT",
                                "entry_type": "BlogModule",
                                "marketplace": {
                                    "category": "content",
                                    "tags": ["content", "editorial"]
                                },
                                "ui_packages": {
                                    "admin": { "crate_name": "rustok-blog-admin" },
                                    "storefront": { "crate_name": "rustok-blog-storefront" }
                                }
                            }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("v2 publish request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("v2 publish body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected /v2/catalog/publish response body: {}",
            String::from_utf8_lossy(&body)
        );
        let payload: Value =
            serde_json::from_slice(&body).expect("v2 publish response should be valid json");
        assert_eq!(
            payload.get("action").and_then(Value::as_str),
            Some("publish")
        );
        assert_eq!(payload.get("dry_run").and_then(Value::as_bool), Some(true));
        assert_eq!(payload.get("accepted").and_then(Value::as_bool), Some(true));
    }

    #[tokio::test]
    #[serial]
    async fn registry_yank_endpoint_accepts_dry_run_contract() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry yank smoke");
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
        let response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v2/catalog/yank")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": true,
                            "slug": "blog",
                            "version": "0.1.0",
                            "reason": "Accidental publish"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("v2 yank request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("v2 yank body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected /v2/catalog/yank response body: {}",
            String::from_utf8_lossy(&body)
        );
        let payload: Value =
            serde_json::from_slice(&body).expect("v2 yank response should be valid json");
        assert_eq!(payload.get("action").and_then(Value::as_str), Some("yank"));
        assert_eq!(payload.get("dry_run").and_then(Value::as_bool), Some(true));
        assert_eq!(payload.get("accepted").and_then(Value::as_bool), Some(true));
    }

    #[tokio::test]
    #[serial]
    async fn registry_owner_transfer_endpoint_accepts_dry_run_contract() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry owner transfer smoke");
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
        let response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v2/catalog/owner-transfer")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": true,
                            "slug": "blog",
                            "new_owner_actor": "publisher:forum",
                            "reason": "Ownership moved to a new maintained publisher identity"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("v2 owner transfer request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("v2 owner transfer body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected /v2/catalog/owner-transfer response body: {}",
            String::from_utf8_lossy(&body)
        );
        let payload: Value =
            serde_json::from_slice(&body).expect("v2 owner transfer response should be valid json");
        assert_eq!(
            payload.get("action").and_then(Value::as_str),
            Some("owner_transfer")
        );
        assert_eq!(payload.get("dry_run").and_then(Value::as_bool), Some(true));
        assert_eq!(payload.get("accepted").and_then(Value::as_bool), Some(true));
    }

    #[tokio::test]
    #[serial]
    async fn registry_validation_stage_endpoint_accepts_dry_run_contract() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry validation stage smoke");
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

        let governance =
            crate::services::registry_governance::RegistryGovernanceService::new(ctx.db.clone());
        let created = governance
            .create_publish_request(
                &crate::services::marketplace_catalog::RegistryPublishRequest {
                    schema_version: 1,
                    dry_run: false,
                    module: crate::services::marketplace_catalog::RegistryPublishModuleRequest {
                        slug: "blog".to_string(),
                        version: "0.1.0".to_string(),
                        crate_name: "rustok-blog".to_string(),
                        name: "Blog".to_string(),
                        description: "Blog and news module contract preview.".to_string(),
                        ownership: "first_party".to_string(),
                        trust_level: "verified".to_string(),
                        license: "MIT".to_string(),
                        entry_type: Some("BlogModule".to_string()),
                        marketplace:
                            crate::services::marketplace_catalog::RegistryPublishMarketplaceRequest {
                                category: Some("content".to_string()),
                                tags: vec!["content".to_string()],
                            },
                        ui_packages:
                            crate::services::marketplace_catalog::RegistryPublishUiPackagesRequest {
                                admin: None,
                                storefront: None,
                            },
                    },
                },
                "xtask:module-publish",
                Some("publisher:blog"),
                &[],
            )
            .await
            .expect("publish request should be created for stage dry-run");
        let mut approved_active =
            crate::models::registry_publish_request::ActiveModel::from(created.clone());
        approved_active.status = sea_orm::Set(
            crate::models::registry_publish_request::RegistryPublishRequestStatus::Approved,
        );
        approved_active.validated_at = sea_orm::Set(Some(chrono::Utc::now()));
        approved_active.approved_at = sea_orm::Set(Some(chrono::Utc::now()));
        approved_active.updated_at = sea_orm::Set(chrono::Utc::now());
        let approved = approved_active
            .update(&ctx.db)
            .await
            .expect("request should become approved");

        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v2/catalog/publish/{}/stages", approved.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": true,
                            "stage": "compile_smoke",
                            "status": "passed",
                            "detail": "Compile smoke passed in external CI.",
                            "requeue": false
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("v2 validation stage request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("v2 validation stage body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected /v2/catalog/publish/{{request_id}}/stages response body: {}",
            String::from_utf8_lossy(&body)
        );
        let payload: Value = serde_json::from_slice(&body)
            .expect("v2 validation stage response should be valid json");
        assert_eq!(
            payload.get("action").and_then(Value::as_str),
            Some("validation_stage")
        );
        assert_eq!(payload.get("dry_run").and_then(Value::as_bool), Some(true));
        assert_eq!(payload.get("accepted").and_then(Value::as_bool), Some(true));
    }

    #[tokio::test]
    #[serial]
    async fn registry_validation_stage_endpoint_persists_live_running_update() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry validation stage live update");
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

        let approved = create_approved_publish_request(&ctx).await;
        insert_validation_stage(
            &ctx,
            &approved,
            "compile_smoke",
            crate::models::registry_validation_stage::RegistryValidationStageStatus::Queued,
            1,
            "Compile smoke queued for operator execution.",
        )
        .await;

        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v2/catalog/publish/{}/stages", approved.id))
                    .header("content-type", "application/json")
                    .header("x-rustok-actor", "governance:moderator")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": false,
                            "stage": "compile_smoke",
                            "status": "running",
                            "detail": "Compile smoke started in external CI.",
                            "requeue": false
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("live validation stage request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("live validation stage body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected live /v2/catalog/publish/{{request_id}}/stages response body: {}",
            String::from_utf8_lossy(&body)
        );
        let payload: Value = serde_json::from_slice(&body)
            .expect("live validation stage response should be valid json");
        assert_eq!(
            payload.get("action").and_then(Value::as_str),
            Some("validation_stage")
        );
        assert_eq!(payload.get("dry_run").and_then(Value::as_bool), Some(false));
        assert_eq!(
            payload.get("status").and_then(Value::as_str),
            Some("running")
        );

        let persisted = crate::models::registry_validation_stage::Entity::find()
            .filter(crate::models::registry_validation_stage::Column::RequestId.eq(approved.id))
            .filter(crate::models::registry_validation_stage::Column::StageKey.eq("compile_smoke"))
            .order_by_desc(crate::models::registry_validation_stage::Column::AttemptNumber)
            .one(&ctx.db)
            .await
            .expect("stage lookup should succeed")
            .expect("stage row should persist");
        assert_eq!(
            persisted.status,
            crate::models::registry_validation_stage::RegistryValidationStageStatus::Running
        );
        assert_eq!(persisted.attempt_number, 1);
        assert_eq!(persisted.detail, "Compile smoke started in external CI.");
    }

    #[tokio::test]
    #[serial]
    async fn registry_validation_stage_endpoint_rejects_invalid_live_transition() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry validation stage rejection");
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

        let approved = create_approved_publish_request(&ctx).await;
        insert_validation_stage(
            &ctx,
            &approved,
            "compile_smoke",
            crate::models::registry_validation_stage::RegistryValidationStageStatus::Passed,
            1,
            "Compile smoke already passed in external CI.",
        )
        .await;

        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v2/catalog/publish/{}/stages", approved.id))
                    .header("content-type", "application/json")
                    .header("x-rustok-actor", "governance:moderator")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": false,
                            "stage": "compile_smoke",
                            "status": "running",
                            "detail": "Attempting to restart a completed stage.",
                            "requeue": false
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("invalid transition request should complete");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("invalid transition body should read");
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "unexpected invalid transition response body: {}",
            String::from_utf8_lossy(&body)
        );

        let persisted = crate::models::registry_validation_stage::Entity::find()
            .filter(crate::models::registry_validation_stage::Column::RequestId.eq(approved.id))
            .filter(crate::models::registry_validation_stage::Column::StageKey.eq("compile_smoke"))
            .order_by_desc(crate::models::registry_validation_stage::Column::AttemptNumber)
            .one(&ctx.db)
            .await
            .expect("stage lookup should succeed")
            .expect("stage row should still exist");
        assert_eq!(
            persisted.status,
            crate::models::registry_validation_stage::RegistryValidationStageStatus::Passed
        );
        assert_eq!(persisted.attempt_number, 1);
        assert_eq!(
            persisted.detail,
            "Compile smoke already passed in external CI."
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_only_host_mode_limits_exposed_surface() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for registry-only smoke");
        ctx.config.settings = Some(serde_json::json!({
            "rustok": {
                "runtime": {
                    "host_mode": "registry_only"
                },
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
            .expect("registry-only base router should build");
        let app = <App as Hooks>::after_routes(base_router, &ctx)
            .await
            .expect("registry-only after_routes should wire runtime");

        let catalog_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/catalog")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("registry-only catalog request should succeed");
        assert_eq!(catalog_response.status(), StatusCode::OK);

        let health_ready_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("registry-only health/ready request should succeed");
        assert_eq!(health_ready_response.status(), StatusCode::OK);

        let health_modules_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health/modules")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("registry-only health/modules request should succeed");
        assert_eq!(health_modules_response.status(), StatusCode::OK);

        let publish_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v2/catalog/publish")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": true,
                            "module": {
                                "slug": "blog",
                                "version": "0.1.0",
                                "crate_name": "rustok-blog",
                                "name": "Blog",
                                "description": "Blog and news module contract preview.",
                                "ownership": "first_party",
                                "trust_level": "verified",
                                "license": "MIT"
                            }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("registry-only publish request should complete");
        assert_eq!(publish_response.status(), StatusCode::NOT_FOUND);

        let owner_transfer_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v2/catalog/owner-transfer")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": true,
                            "slug": "blog",
                            "new_owner_actor": "publisher:forum",
                            "reason": "Registry-only host must stay read-only"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("registry-only owner transfer request should complete");
        assert_eq!(owner_transfer_response.status(), StatusCode::NOT_FOUND);

        let yank_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v2/catalog/yank")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": true,
                            "slug": "blog",
                            "version": "0.1.0",
                            "reason": "Registry-only host must stay read-only"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("registry-only yank request should complete");
        assert_eq!(yank_response.status(), StatusCode::NOT_FOUND);

        let graphql_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/graphql")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("registry-only graphql request should complete");
        assert_eq!(graphql_response.status(), StatusCode::NOT_FOUND);

        let auth_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/auth/me")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("registry-only auth request should complete");
        assert_eq!(auth_response.status(), StatusCode::NOT_FOUND);

        let admin_response = app
            .oneshot(
                Request::builder()
                    .uri("/admin")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("registry-only admin request should complete");
        assert_eq!(admin_response.status(), StatusCode::NOT_FOUND);
    }

    async fn create_approved_publish_request(
        ctx: &loco_rs::app::AppContext,
    ) -> crate::models::registry_publish_request::Model {
        let governance =
            crate::services::registry_governance::RegistryGovernanceService::new(ctx.db.clone());
        let created = governance
            .create_publish_request(
                &crate::services::marketplace_catalog::RegistryPublishRequest {
                    schema_version: 1,
                    dry_run: false,
                    module: crate::services::marketplace_catalog::RegistryPublishModuleRequest {
                        slug: "blog".to_string(),
                        version: "0.1.0".to_string(),
                        crate_name: "rustok-blog".to_string(),
                        name: "Blog".to_string(),
                        description: "Blog and news module contract preview.".to_string(),
                        ownership: "first_party".to_string(),
                        trust_level: "verified".to_string(),
                        license: "MIT".to_string(),
                        entry_type: Some("BlogModule".to_string()),
                        marketplace:
                            crate::services::marketplace_catalog::RegistryPublishMarketplaceRequest {
                                category: Some("content".to_string()),
                                tags: vec!["content".to_string()],
                            },
                        ui_packages:
                            crate::services::marketplace_catalog::RegistryPublishUiPackagesRequest {
                                admin: None,
                                storefront: None,
                            },
                    },
                },
                "xtask:module-publish",
                Some("publisher:blog"),
                &[],
            )
            .await
            .expect("publish request should be created");
        let mut approved_active =
            crate::models::registry_publish_request::ActiveModel::from(created.clone());
        approved_active.status =
            Set(crate::models::registry_publish_request::RegistryPublishRequestStatus::Approved);
        approved_active.validated_at = Set(Some(chrono::Utc::now()));
        approved_active.approved_at = Set(Some(chrono::Utc::now()));
        approved_active.updated_at = Set(chrono::Utc::now());
        approved_active
            .update(&ctx.db)
            .await
            .expect("request should become approved")
    }

    async fn insert_validation_stage(
        ctx: &loco_rs::app::AppContext,
        request: &crate::models::registry_publish_request::Model,
        stage_key: &str,
        status: crate::models::registry_validation_stage::RegistryValidationStageStatus,
        attempt_number: i32,
        detail: &str,
    ) -> crate::models::registry_validation_stage::Model {
        let now = chrono::Utc::now();
        crate::models::registry_validation_stage::ActiveModel {
            id: Set(format!("rvs_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request.id.clone()),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            stage_key: Set(stage_key.to_string()),
            status: Set(status),
            triggered_by: Set("test:stage".to_string()),
            queue_reason: Set("test_setup".to_string()),
            attempt_number: Set(attempt_number),
            detail: Set(detail.to_string()),
            started_at: Set(None),
            finished_at: Set(None),
            last_error: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&ctx.db)
        .await
        .expect("validation stage should insert")
    }
}
