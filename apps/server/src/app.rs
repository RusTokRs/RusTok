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
        check_production_secrets(&ctx)?;

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
                .add_route(controllers::flex::routes())
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

/// Abort startup when known dev/test secrets are detected in a release build.
///
/// This is a defence-in-depth guard: a misconfigured production deployment that
/// copies the sample config verbatim will fail loudly at boot time rather than
/// silently running with a predictable JWT secret.  The check is compiled out in
/// debug builds so local development and tests are unaffected.
fn check_production_secrets(ctx: &AppContext) -> Result<()> {
    #[cfg(not(debug_assertions))]
    {
        let jwt_secret = ctx
            .config
            .auth
            .as_ref()
            .and_then(|auth| auth.jwt.as_ref())
            .map(|jwt| jwt.secret.as_str())
            .unwrap_or("");

        if let Some(fragment) = known_dev_jwt_fragment(jwt_secret) {
            return Err(loco_rs::Error::Message(format!(
                "FATAL: JWT secret contains a known development value (\"{fragment}\"). \
                 Set a strong, random secret in your production configuration."
            )));
        }

        if !jwt_secret.is_empty() && jwt_secret.len() < 32 {
            return Err(loco_rs::Error::Message(
                "FATAL: JWT secret is too short (< 32 characters) for production use. \
                 Generate a cryptographically random secret of at least 32 characters."
                    .to_string(),
            ));
        }

        if let Some(pattern) = sample_database_credentials_pattern(&ctx.config.database.uri) {
            return Err(loco_rs::Error::Message(format!(
                "FATAL: database URI matches known sample credentials ({pattern}). \
                 Set production database credentials before starting the release build."
            )));
        }

        if let Some((variable, password)) = configured_superadmin_password() {
            if let Some(sample) = known_sample_superadmin_password(&password) {
                return Err(loco_rs::Error::Message(format!(
                    "FATAL: env var {variable} contains sample superadmin password \"{sample}\". \
                     Set a unique secret before starting the release build."
                )));
            }
        }
    }

    let _ = ctx; // suppress unused warning in debug builds
    Ok(())
}

#[cfg_attr(debug_assertions, allow(dead_code))]
fn known_dev_jwt_fragment(secret: &str) -> Option<&'static str> {
    const KNOWN_DEV_SUBSTRINGS: &[&str] = &[
        "dev-secret",
        "test-secret",
        "change-in-production",
        "dev_secret",
        "rustok-dev-secret",
    ];

    KNOWN_DEV_SUBSTRINGS
        .iter()
        .copied()
        .find(|fragment| secret.contains(fragment))
}

#[cfg_attr(debug_assertions, allow(dead_code))]
fn sample_database_credentials_pattern(uri: &str) -> Option<&'static str> {
    const SAMPLE_PATTERNS: &[&str] = &["://postgres:postgres@", "://rustok:rustok@"];

    SAMPLE_PATTERNS
        .iter()
        .copied()
        .find(|pattern| uri.contains(pattern))
}

#[cfg_attr(debug_assertions, allow(dead_code))]
fn configured_superadmin_password() -> Option<(&'static str, String)> {
    for key in [
        "SUPERADMIN_PASSWORD",
        "SEED_ADMIN_PASSWORD",
        "RUSTOK_DEV_SEED_PASSWORD",
    ] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim().to_string();
            if !value.is_empty() {
                return Some((key, value));
            }
        }
    }

    None
}

#[cfg_attr(debug_assertions, allow(dead_code))]
fn known_sample_superadmin_password(password: &str) -> Option<&'static str> {
    const SAMPLE_PASSWORDS: &[&str] =
        &["change-me-in-production", "admin12345", "dev-password-123"];

    SAMPLE_PASSWORDS
        .iter()
        .copied()
        .find(|candidate| password == *candidate)
}

#[cfg(test)]
mod tests {
    use super::{
        known_dev_jwt_fragment, known_sample_superadmin_password,
        sample_database_credentials_pattern, App,
    };
    use axum::body::{to_bytes, Body};
    use axum::http::{Method, Request, StatusCode};
    use loco_rs::{app::Hooks, tests_cfg::app::get_app_context};
    use migration::Migrator;
    use rustok_api::context::{AuthContext, AuthContextExtension};
    use rustok_core::Permission;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
    use sea_orm_migration::MigratorTrait;
    use serde_json::Value;
    use serial_test::serial;
    use std::sync::Arc;
    use tower::ServiceExt;

    #[test]
    fn production_guardrail_detects_known_dev_jwt_fragments() {
        assert_eq!(
            known_dev_jwt_fragment("prefix-rustok-dev-secret-suffix"),
            Some("dev-secret")
        );
        assert_eq!(known_dev_jwt_fragment("totally-random-secret"), None);
    }

    #[test]
    fn production_guardrail_detects_sample_database_credentials() {
        assert_eq!(
            sample_database_credentials_pattern(
                "postgres://postgres:postgres@db.internal:5432/rustok"
            ),
            Some("://postgres:postgres@")
        );
        assert_eq!(
            sample_database_credentials_pattern("postgres://rustok:rustok@db.internal:5432/rustok"),
            Some("://rustok:rustok@")
        );
        assert_eq!(
            sample_database_credentials_pattern(
                "postgres://prod-user:strong-pass@db.internal:5432/rustok"
            ),
            None
        );
    }

    #[test]
    fn production_guardrail_detects_sample_superadmin_passwords() {
        assert_eq!(
            known_sample_superadmin_password("admin12345"),
            Some("admin12345")
        );
        assert_eq!(
            known_sample_superadmin_password("dev-password-123"),
            Some("dev-password-123")
        );
        assert_eq!(known_sample_superadmin_password("S3cure!Passphrase"), None);
    }

    use crate::graphql::SharedGraphqlSchema;
    use crate::middleware::rate_limit::{
        SharedApiRateLimiter, SharedAuthRateLimiter, SharedOAuthRateLimiter,
    };
    use crate::services::event_transport_factory::EventRuntime;
    use crate::services::marketplace_catalog::SharedMarketplaceCatalogService;
    use crate::services::registry_principal::{RegistryAuthority, RegistryPrincipalRef};

    fn principal_json(label: &str) -> serde_json::Value {
        RegistryPrincipalRef::from_legacy_value(label).to_json_value()
    }

    fn registry_authority(label: &str) -> RegistryAuthority {
        RegistryAuthority {
            principal: RegistryPrincipalRef::from_legacy_value(label),
            can_manage_modules: false,
        }
    }

    fn publish_status_auth(user_id: uuid::Uuid, can_manage_modules: bool) -> AuthContext {
        AuthContext {
            user_id,
            session_id: uuid::Uuid::new_v4(),
            tenant_id: uuid::Uuid::new_v4(),
            permissions: if can_manage_modules {
                vec![Permission::MODULES_MANAGE]
            } else {
                Vec::new()
            },
            client_id: None,
            scopes: Vec::new(),
            grant_type: "session".to_string(),
        }
    }

    fn governance_auth() -> AuthContext {
        publish_status_auth(uuid::Uuid::new_v4(), true)
    }

    fn oauth_service_token_auth() -> AuthContext {
        AuthContext {
            user_id: uuid::Uuid::nil(),
            session_id: uuid::Uuid::nil(),
            tenant_id: uuid::Uuid::new_v4(),
            permissions: vec![Permission::MODULES_MANAGE],
            client_id: Some(uuid::Uuid::new_v4()),
            scopes: Vec::new(),
            grant_type: "client_credentials".to_string(),
        }
    }

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
        let response = send_json_request(
            base_router.clone(),
            Method::POST,
            "/v2/catalog/owner-transfer",
            serde_json::json!({
                "schema_version": 1,
                "dry_run": true,
                "slug": "blog",
                "new_owner_user_id": uuid::Uuid::new_v4(),
                "reason": "Ownership moved to a new maintained publisher identity"
            }),
            Some(governance_auth()),
        )
        .await;
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
    async fn registry_publish_reject_endpoint_rejects_invalid_reason_code() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for reject invalid reason_code");
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
        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let response = send_json_request(
            base_router.clone(),
            Method::POST,
            format!("/v2/catalog/publish/{}/reject", approved.id),
            serde_json::json!({
                "schema_version": 1,
                "dry_run": false,
                "reason": "Ownership evidence is incomplete.",
                "reason_code": "not_supported"
            }),
            Some(governance_auth()),
        )
        .await;
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("live reject error body should read");
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "unexpected live /v2/catalog/publish/{{request_id}}/reject response body: {}",
            String::from_utf8_lossy(&body)
        );
        assert!(
            String::from_utf8_lossy(&body).contains("not supported"),
            "reject error should mention unsupported reason_code: {}",
            String::from_utf8_lossy(&body)
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_publish_reject_endpoint_rejects_oauth_service_tokens() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for reject service-token validation");
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
        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let response = send_json_request(
            base_router.clone(),
            Method::POST,
            format!("/v2/catalog/publish/{}/reject", approved.id),
            serde_json::json!({
                "schema_version": 1,
                "dry_run": false,
                "reason": "Ownership evidence is incomplete.",
                "reason_code": "ownership_mismatch"
            }),
            Some(oauth_service_token_auth()),
        )
        .await;
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("service token reject error body should read");
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "unexpected live /v2/catalog/publish/{{request_id}}/reject response body: {}",
            String::from_utf8_lossy(&body)
        );
        assert!(
            String::from_utf8_lossy(&body).contains("OAuth service tokens are not supported"),
            "service token error should mention unsupported principal type: {}",
            String::from_utf8_lossy(&body)
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_yank_endpoint_rejects_invalid_reason_code() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for yank invalid reason_code");
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

        let governance_user_id = uuid::Uuid::new_v4();
        insert_registry_owner_binding(&ctx, "blog", &format!("user:{governance_user_id}")).await;
        insert_active_release(
            &ctx,
            "blog",
            "0.1.0",
            Some(&format!("user:{governance_user_id}")),
            None,
        )
        .await;

        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let response = send_json_request(
            base_router.clone(),
            Method::POST,
            "/v2/catalog/yank",
            serde_json::json!({
                "schema_version": 1,
                "dry_run": false,
                "slug": "blog",
                "version": "0.1.0",
                "reason": "Release needs to be withdrawn.",
                "reason_code": "not_supported"
            }),
            Some(publish_status_auth(governance_user_id, true)),
        )
        .await;
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("live yank error body should read");
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "unexpected live /v2/catalog/yank response body: {}",
            String::from_utf8_lossy(&body)
        );
        assert!(
            String::from_utf8_lossy(&body).contains("not supported"),
            "yank error should mention unsupported reason_code: {}",
            String::from_utf8_lossy(&body)
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_owner_transfer_endpoint_rejects_invalid_reason_code() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for owner transfer invalid reason_code");
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

        insert_registry_owner_binding(&ctx, "blog", &format!("user:{}", uuid::Uuid::new_v4()))
            .await;

        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let response = send_json_request(
            base_router.clone(),
            Method::POST,
            "/v2/catalog/owner-transfer",
            serde_json::json!({
                "schema_version": 1,
                "dry_run": false,
                "slug": "blog",
                "new_owner_user_id": uuid::Uuid::new_v4(),
                "reason": "Ownership moved to a new maintained publisher identity.",
                "reason_code": "not_supported"
            }),
            Some(governance_auth()),
        )
        .await;
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("live owner transfer error body should read");
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "unexpected /v2/catalog/owner-transfer response body: {}",
            String::from_utf8_lossy(&body)
        );
        assert!(
            String::from_utf8_lossy(&body).contains("not supported"),
            "owner transfer error should mention unsupported reason_code: {}",
            String::from_utf8_lossy(&body)
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_owner_transfer_endpoint_rejects_legacy_headers_without_auth() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for legacy header rejection");
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
        let request = Request::builder()
            .method(Method::POST)
            .uri("/v2/catalog/owner-transfer")
            .header("content-type", "application/json")
            .header(concat!("x-rustok-", "actor"), "registry:admin")
            .body(Body::from(
                serde_json::json!({
                    "schema_version": 1,
                    "dry_run": false,
                    "slug": "blog",
                    "new_owner_user_id": uuid::Uuid::new_v4(),
                    "reason": "Ownership moved to a new maintained publisher identity.",
                    "reason_code": "maintenance_handoff"
                })
                .to_string(),
            ))
            .expect("request");

        let response = base_router
            .oneshot(request)
            .await
            .expect("legacy header rejection request should complete");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("legacy header rejection body should read");

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            String::from_utf8_lossy(&body).contains("legacy actor/publisher headers"),
            "legacy header rejection should explain the clean contract: {}",
            String::from_utf8_lossy(&body)
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_owner_transfer_endpoint_reports_conflict_for_same_owner() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for owner transfer conflict");
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

        let owner_user_id = uuid::Uuid::new_v4();
        insert_registry_owner_binding(&ctx, "blog", &format!("user:{owner_user_id}")).await;

        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let response = send_json_request(
            base_router.clone(),
            Method::POST,
            "/v2/catalog/owner-transfer",
            serde_json::json!({
                "schema_version": 1,
                "dry_run": false,
                "slug": "blog",
                "new_owner_user_id": owner_user_id,
                "reason": "Ownership moved to the same principal by mistake.",
                "reason_code": "maintenance_handoff"
            }),
            Some(governance_auth()),
        )
        .await;
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("owner transfer conflict body should read");

        assert_eq!(status, StatusCode::CONFLICT);
        assert!(
            String::from_utf8_lossy(&body).contains("already bound"),
            "owner transfer conflict should preserve the typed conflict message: {}",
            String::from_utf8_lossy(&body)
        );
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
                        default_locale: "en".to_string(),
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
                &registry_authority("user:00000000-0000-0000-0000-000000000111"),
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
        let response = send_json_request(
            base_router.clone(),
            Method::POST,
            format!("/v2/catalog/publish/{}/stages", approved.id),
            serde_json::json!({
                "schema_version": 1,
                "dry_run": true,
                "stage": "compile_smoke",
                "status": "passed",
                "detail": "Compile smoke passed in external CI.",
                "requeue": false
            }),
            Some(governance_auth()),
        )
        .await;
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
        let response = send_json_request(
            base_router.clone(),
            Method::POST,
            format!("/v2/catalog/publish/{}/stages", approved.id),
            serde_json::json!({
                "schema_version": 1,
                "dry_run": false,
                "stage": "compile_smoke",
                "status": "running",
                "detail": "Compile smoke started in external CI.",
                "requeue": false
            }),
            Some(governance_auth()),
        )
        .await;
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
        let response = send_json_request(
            base_router.clone(),
            Method::POST,
            format!("/v2/catalog/publish/{}/stages", approved.id),
            serde_json::json!({
                "schema_version": 1,
                "dry_run": false,
                "stage": "compile_smoke",
                "status": "running",
                "detail": "Attempting to restart a completed stage.",
                "requeue": false
            }),
            Some(governance_auth()),
        )
        .await;
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("invalid transition body should read");
        assert_eq!(
            status,
            StatusCode::CONFLICT,
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
    async fn registry_publish_reject_endpoint_requires_live_reason_code() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for publish reject reason_code validation");
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
        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let response = send_json_request(
            base_router.clone(),
            Method::POST,
            format!("/v2/catalog/publish/{}/reject", approved.id),
            serde_json::json!({
                "schema_version": 1,
                "dry_run": false,
                "reason": "Ownership evidence is incomplete."
            }),
            Some(governance_auth()),
        )
        .await;
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("live reject error body should read");
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "unexpected live /v2/catalog/publish/{{request_id}}/reject response body: {}",
            String::from_utf8_lossy(&body)
        );
        assert!(
            String::from_utf8_lossy(&body).contains("reason_code"),
            "reject error should mention missing reason_code: {}",
            String::from_utf8_lossy(&body)
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_publish_reject_endpoint_persists_reason_code_in_audit_event() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for publish reject reason_code audit");
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
        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let response = send_json_request(
            base_router.clone(),
            Method::POST,
            format!("/v2/catalog/publish/{}/reject", approved.id),
            serde_json::json!({
                "schema_version": 1,
                "dry_run": false,
                "reason": "Ownership evidence is incomplete.",
                "reason_code": "ownership_mismatch"
            }),
            Some(governance_auth()),
        )
        .await;
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("live reject body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected live /v2/catalog/publish/{{request_id}}/reject response body: {}",
            String::from_utf8_lossy(&body)
        );
        let payload: Value =
            serde_json::from_slice(&body).expect("live reject response should be valid json");
        assert_eq!(
            payload.get("action").and_then(Value::as_str),
            Some("reject")
        );
        assert_eq!(payload.get("dry_run").and_then(Value::as_bool), Some(false));
        assert_eq!(
            payload.get("status").and_then(Value::as_str),
            Some("rejected")
        );

        let persisted_request = crate::models::registry_publish_request::Entity::find()
            .filter(crate::models::registry_publish_request::Column::Id.eq(approved.id.clone()))
            .one(&ctx.db)
            .await
            .expect("request lookup should succeed")
            .expect("request should persist");
        assert_eq!(
            persisted_request.status,
            crate::models::registry_publish_request::RegistryPublishRequestStatus::Rejected
        );
        assert_eq!(
            persisted_request.rejection_reason.as_deref(),
            Some("Ownership evidence is incomplete.")
        );

        let event = crate::models::registry_governance_event::Entity::find()
            .filter(crate::models::registry_governance_event::Column::RequestId.eq(approved.id))
            .filter(
                crate::models::registry_governance_event::Column::EventType.eq("request_rejected"),
            )
            .order_by_desc(crate::models::registry_governance_event::Column::CreatedAt)
            .one(&ctx.db)
            .await
            .expect("governance event lookup should succeed")
            .expect("request_rejected event should persist");
        assert_eq!(
            event.details.get("reason_code").and_then(Value::as_str),
            Some("ownership_mismatch")
        );
        assert_eq!(
            event.details.get("reason").and_then(Value::as_str),
            Some("Ownership evidence is incomplete.")
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_publish_request_changes_endpoint_persists_reason_code_in_audit_event() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for request-changes audit");
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
        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let response = send_json_request(
            base_router.clone(),
            Method::POST,
            format!("/v2/catalog/publish/{}/request-changes", approved.id),
            serde_json::json!({
                "schema_version": 1,
                "dry_run": false,
                "reason": "Artifact metadata drifted from the reviewed contract.",
                "reason_code": "artifact_mismatch"
            }),
            Some(governance_auth()),
        )
        .await;
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("request-changes body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected live /v2/catalog/publish/{{request_id}}/request-changes response body: {}",
            String::from_utf8_lossy(&body)
        );
        let payload: Value =
            serde_json::from_slice(&body).expect("request-changes response should be valid json");
        assert_eq!(
            payload.get("action").and_then(Value::as_str),
            Some("request_changes")
        );
        assert_eq!(
            payload.get("status").and_then(Value::as_str),
            Some("changes_requested")
        );

        let persisted_request = crate::models::registry_publish_request::Entity::find()
            .filter(crate::models::registry_publish_request::Column::Id.eq(approved.id.clone()))
            .one(&ctx.db)
            .await
            .expect("request lookup should succeed")
            .expect("request should persist");
        assert_eq!(
            persisted_request.status,
            crate::models::registry_publish_request::RegistryPublishRequestStatus::ChangesRequested
        );
        assert_eq!(
            persisted_request.changes_requested_reason.as_deref(),
            Some("Artifact metadata drifted from the reviewed contract.")
        );
        assert_eq!(
            persisted_request.changes_requested_reason_code.as_deref(),
            Some("artifact_mismatch")
        );

        let event = crate::models::registry_governance_event::Entity::find()
            .filter(crate::models::registry_governance_event::Column::RequestId.eq(approved.id))
            .filter(
                crate::models::registry_governance_event::Column::EventType.eq("changes_requested"),
            )
            .order_by_desc(crate::models::registry_governance_event::Column::CreatedAt)
            .one(&ctx.db)
            .await
            .expect("governance event lookup should succeed")
            .expect("changes_requested event should persist");
        assert_eq!(
            event.details.get("reason_code").and_then(Value::as_str),
            Some("artifact_mismatch")
        );
        assert_eq!(
            event.details.get("reason").and_then(Value::as_str),
            Some("Artifact metadata drifted from the reviewed contract.")
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_publish_hold_and_resume_endpoints_round_trip_request_status() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for hold/resume lifecycle");
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
        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");

        let hold_response = send_json_request(
            base_router.clone(),
            Method::POST,
            format!("/v2/catalog/publish/{}/hold", approved.id),
            serde_json::json!({
                "schema_version": 1,
                "dry_run": false,
                "reason": "Release window is temporarily closed.",
                "reason_code": "release_window"
            }),
            Some(governance_auth()),
        )
        .await;
        let hold_status = hold_response.status();
        let hold_body = to_bytes(hold_response.into_body(), usize::MAX)
            .await
            .expect("hold body should read");
        assert_eq!(
            hold_status,
            StatusCode::OK,
            "unexpected live /v2/catalog/publish/{{request_id}}/hold response body: {}",
            String::from_utf8_lossy(&hold_body)
        );
        let hold_payload: Value =
            serde_json::from_slice(&hold_body).expect("hold response should be valid json");
        assert_eq!(
            hold_payload.get("action").and_then(Value::as_str),
            Some("hold")
        );
        assert_eq!(
            hold_payload.get("status").and_then(Value::as_str),
            Some("on_hold")
        );

        let held_request = crate::models::registry_publish_request::Entity::find()
            .filter(crate::models::registry_publish_request::Column::Id.eq(approved.id.clone()))
            .one(&ctx.db)
            .await
            .expect("held request lookup should succeed")
            .expect("held request should persist");
        assert_eq!(
            held_request.status,
            crate::models::registry_publish_request::RegistryPublishRequestStatus::OnHold
        );
        assert_eq!(held_request.held_from_status.as_deref(), Some("approved"));

        let resume_response = send_json_request(
            base_router.clone(),
            Method::POST,
            format!("/v2/catalog/publish/{}/resume", approved.id),
            serde_json::json!({
                "schema_version": 1,
                "dry_run": false,
                "reason": "Release window reopened after review.",
                "reason_code": "review_complete"
            }),
            Some(governance_auth()),
        )
        .await;
        let resume_status = resume_response.status();
        let resume_body = to_bytes(resume_response.into_body(), usize::MAX)
            .await
            .expect("resume body should read");
        assert_eq!(
            resume_status,
            StatusCode::OK,
            "unexpected live /v2/catalog/publish/{{request_id}}/resume response body: {}",
            String::from_utf8_lossy(&resume_body)
        );
        let resume_payload: Value =
            serde_json::from_slice(&resume_body).expect("resume response should be valid json");
        assert_eq!(
            resume_payload.get("action").and_then(Value::as_str),
            Some("resume")
        );
        assert_eq!(
            resume_payload.get("status").and_then(Value::as_str),
            Some("approved")
        );

        let resumed_request = crate::models::registry_publish_request::Entity::find()
            .filter(crate::models::registry_publish_request::Column::Id.eq(approved.id.clone()))
            .one(&ctx.db)
            .await
            .expect("resumed request lookup should succeed")
            .expect("resumed request should persist");
        assert_eq!(
            resumed_request.status,
            crate::models::registry_publish_request::RegistryPublishRequestStatus::Approved
        );

        let event = crate::models::registry_governance_event::Entity::find()
            .filter(crate::models::registry_governance_event::Column::RequestId.eq(approved.id))
            .filter(
                crate::models::registry_governance_event::Column::EventType.eq("request_resumed"),
            )
            .order_by_desc(crate::models::registry_governance_event::Column::CreatedAt)
            .one(&ctx.db)
            .await
            .expect("resume event lookup should succeed")
            .expect("request_resumed event should persist");
        assert_eq!(
            event
                .details
                .get("resumed_to_status")
                .and_then(Value::as_str),
            Some("approved")
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_publish_status_actor_filtering_keeps_summary_fields_stable_for_review_actions(
    ) {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for actor-aware publish status");
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
        let owner_user_id = uuid::Uuid::new_v4();
        let governance_user_id = uuid::Uuid::new_v4();
        let unrelated_user_id = uuid::Uuid::new_v4();
        insert_registry_owner_binding(&ctx, "blog", &format!("user:{owner_user_id}")).await;
        insert_validation_stage(
            &ctx,
            &approved,
            "compile_smoke",
            crate::models::registry_validation_stage::RegistryValidationStageStatus::Queued,
            1,
            "Compile smoke still waits for external completion.",
        )
        .await;

        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let actorless_payload =
            fetch_publish_status_payload(base_router.clone(), &approved.id, None).await;
        let owner_payload = fetch_publish_status_payload(
            base_router.clone(),
            &approved.id,
            Some(publish_status_auth(owner_user_id, false)),
        )
        .await;
        let governance_payload = fetch_publish_status_payload(
            base_router.clone(),
            &approved.id,
            Some(publish_status_auth(governance_user_id, true)),
        )
        .await;
        let unrelated_payload = fetch_publish_status_payload(
            base_router,
            &approved.id,
            Some(publish_status_auth(unrelated_user_id, false)),
        )
        .await;

        let actorless_actions = publish_status_action_keys(&actorless_payload);
        let owner_actions = publish_status_action_keys(&owner_payload);
        let governance_actions = publish_status_action_keys(&governance_payload);
        let unrelated_actions = publish_status_action_keys(&unrelated_payload);

        for action in ["approve", "request_changes", "hold", "reject"] {
            assert!(
                actorless_actions
                    .iter()
                    .any(|candidate| candidate == action),
                "actorless status should advertise '{action}': {:?}",
                actorless_actions
            );
            assert!(
                owner_actions.iter().any(|candidate| candidate == action),
                "owner actor should advertise '{action}': {:?}",
                owner_actions
            );
            assert!(
                governance_actions
                    .iter()
                    .any(|candidate| candidate == action),
                "governance actor should advertise '{action}': {:?}",
                governance_actions
            );
            assert!(
                !unrelated_actions
                    .iter()
                    .any(|candidate| candidate == action),
                "unrelated actor should not advertise '{action}': {:?}",
                unrelated_actions
            );
        }

        for payload in [
            &actorless_payload,
            &owner_payload,
            &governance_payload,
            &unrelated_payload,
        ] {
            assert_eq!(
                payload
                    .get("approvalOverrideRequired")
                    .and_then(Value::as_bool),
                Some(true)
            );
            assert_eq!(
                payload.get("validationStages"),
                actorless_payload.get("validationStages")
            );
            assert_eq!(
                payload.get("followUpGates"),
                actorless_payload.get("followUpGates")
            );
            assert_eq!(payload.get("next_step"), actorless_payload.get("next_step"));
        }
    }

    #[tokio::test]
    #[serial]
    async fn registry_publish_status_actor_filtering_handles_validate_and_resume_actions() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for validate/resume actor-aware status");
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

        let owner_forum_user_id = uuid::Uuid::new_v4();
        let owner_pages_user_id = uuid::Uuid::new_v4();
        let governance_user_id = uuid::Uuid::new_v4();
        let unrelated_user_id = uuid::Uuid::new_v4();

        let mut submitted = create_approved_publish_request_for_slug(&ctx, "forum").await;
        let mut submitted_active =
            crate::models::registry_publish_request::ActiveModel::from(submitted.clone());
        submitted_active.status =
            Set(crate::models::registry_publish_request::RegistryPublishRequestStatus::Submitted);
        submitted_active.approved_at = Set(None);
        submitted_active.updated_at = Set(chrono::Utc::now());
        submitted = submitted_active
            .update(&ctx.db)
            .await
            .expect("submitted request should persist");
        insert_registry_owner_binding(&ctx, "forum", &format!("user:{owner_forum_user_id}")).await;

        let mut held = create_approved_publish_request_for_slug(&ctx, "pages").await;
        let mut held_active =
            crate::models::registry_publish_request::ActiveModel::from(held.clone());
        held_active.status =
            Set(crate::models::registry_publish_request::RegistryPublishRequestStatus::OnHold);
        held_active.held_from_status = Set(Some("submitted".to_string()));
        held_active.held_at = Set(Some(chrono::Utc::now()));
        held_active.held_by = Set(Some(principal_json(&format!("user:{governance_user_id}"))));
        held_active.held_reason = Set(Some("Release train paused.".to_string()));
        held_active.held_reason_code = Set(Some("release_window".to_string()));
        held_active.approved_at = Set(None);
        held_active.updated_at = Set(chrono::Utc::now());
        held = held_active
            .update(&ctx.db)
            .await
            .expect("held request should persist");
        insert_registry_owner_binding(&ctx, "pages", &format!("user:{owner_pages_user_id}")).await;

        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");

        let submitted_owner = fetch_publish_status_payload(
            base_router.clone(),
            &submitted.id,
            Some(publish_status_auth(owner_forum_user_id, false)),
        )
        .await;
        let submitted_governance = fetch_publish_status_payload(
            base_router.clone(),
            &submitted.id,
            Some(publish_status_auth(governance_user_id, true)),
        )
        .await;
        let submitted_unrelated = fetch_publish_status_payload(
            base_router.clone(),
            &submitted.id,
            Some(publish_status_auth(unrelated_user_id, false)),
        )
        .await;
        let held_owner = fetch_publish_status_payload(
            base_router.clone(),
            &held.id,
            Some(publish_status_auth(owner_pages_user_id, false)),
        )
        .await;
        let held_governance = fetch_publish_status_payload(
            base_router.clone(),
            &held.id,
            Some(publish_status_auth(governance_user_id, true)),
        )
        .await;
        let held_unrelated = fetch_publish_status_payload(
            base_router,
            &held.id,
            Some(publish_status_auth(unrelated_user_id, false)),
        )
        .await;

        let submitted_owner_actions = publish_status_action_keys(&submitted_owner);
        let submitted_governance_actions = publish_status_action_keys(&submitted_governance);
        let submitted_unrelated_actions = publish_status_action_keys(&submitted_unrelated);
        assert!(
            submitted_owner_actions
                .iter()
                .any(|candidate| candidate == "validate"),
            "owner actor should advertise validate: {:?}",
            submitted_owner_actions
        );
        assert!(
            submitted_governance_actions
                .iter()
                .any(|candidate| candidate == "validate"),
            "governance actor should advertise validate: {:?}",
            submitted_governance_actions
        );
        assert!(
            !submitted_unrelated_actions
                .iter()
                .any(|candidate| candidate == "validate"),
            "unrelated actor should not advertise validate: {:?}",
            submitted_unrelated_actions
        );

        let held_owner_actions = publish_status_action_keys(&held_owner);
        let held_governance_actions = publish_status_action_keys(&held_governance);
        let held_unrelated_actions = publish_status_action_keys(&held_unrelated);
        assert!(
            held_owner_actions
                .iter()
                .any(|candidate| candidate == "resume"),
            "owner actor should advertise resume: {:?}",
            held_owner_actions
        );
        assert!(
            held_governance_actions
                .iter()
                .any(|candidate| candidate == "resume"),
            "governance actor should advertise resume: {:?}",
            held_governance_actions
        );
        assert!(
            !held_unrelated_actions
                .iter()
                .any(|candidate| candidate == "resume"),
            "unrelated actor should not advertise resume: {:?}",
            held_unrelated_actions
        );
    }

    #[tokio::test]
    #[serial]
    async fn registry_remote_runner_claim_and_complete_round_trip_stage_status() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None)
            .await
            .expect("server migrations should apply for remote runner lifecycle");
        ctx.config.settings = Some(serde_json::json!({
            "rustok": {
                "registry": {
                    "remote_executor": {
                        "enabled": true,
                        "shared_token": "test-runner-token",
                        "lease_ttl_ms": 120000,
                        "requeue_scan_interval_ms": 15000
                    }
                },
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
            "Compile smoke queued for remote runner execution.",
        )
        .await;

        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let claim_response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v2/catalog/runner/claim")
                    .header("content-type", "application/json")
                    .header("x-rustok-runner-token", "test-runner-token")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "runner_id": "worker-1",
                            "supportedStages": ["compile_smoke", "targeted_tests"]
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("runner claim request should succeed");
        assert_eq!(claim_response.status(), StatusCode::OK);
        let claim_body = to_bytes(claim_response.into_body(), usize::MAX)
            .await
            .expect("claim body should read");
        let claim_payload: Value =
            serde_json::from_slice(&claim_body).expect("claim response should be valid json");
        let claim_id = claim_payload["claim"]["claimId"]
            .as_str()
            .expect("claimId should be present")
            .to_string();
        assert_eq!(
            claim_payload["claim"]["stageKey"].as_str(),
            Some("compile_smoke")
        );

        let claimed_stage = crate::models::registry_validation_stage::Entity::find()
            .filter(
                crate::models::registry_validation_stage::Column::RequestId.eq(approved.id.clone()),
            )
            .filter(crate::models::registry_validation_stage::Column::StageKey.eq("compile_smoke"))
            .order_by_desc(crate::models::registry_validation_stage::Column::AttemptNumber)
            .one(&ctx.db)
            .await
            .expect("claimed stage lookup should succeed")
            .expect("claimed stage should persist");
        assert_eq!(
            claimed_stage.status,
            crate::models::registry_validation_stage::RegistryValidationStageStatus::Running
        );
        assert_eq!(claimed_stage.claimed_by.as_deref(), Some("worker-1"));

        let complete_response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v2/catalog/runner/{claim_id}/complete"))
                    .header("content-type", "application/json")
                    .header("x-rustok-runner-token", "test-runner-token")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "runner_id": "worker-1",
                            "detail": "Compile smoke completed successfully on remote worker.",
                            "reason_code": "local_runner_passed"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("runner completion request should succeed");
        assert_eq!(complete_response.status(), StatusCode::OK);

        let completed_stage = crate::models::registry_validation_stage::Entity::find()
            .filter(crate::models::registry_validation_stage::Column::RequestId.eq(approved.id))
            .filter(crate::models::registry_validation_stage::Column::StageKey.eq("compile_smoke"))
            .order_by_desc(crate::models::registry_validation_stage::Column::AttemptNumber)
            .one(&ctx.db)
            .await
            .expect("completed stage lookup should succeed")
            .expect("completed stage should persist");
        assert_eq!(
            completed_stage.status,
            crate::models::registry_validation_stage::RegistryValidationStageStatus::Passed
        );
        assert!(completed_stage.claim_id.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn registry_catalog_detail_excludes_approved_but_unpublished_v2_requests() {
        let mut ctx = get_app_context().await;
        Migrator::up(&ctx.db, None).await.expect(
            "server migrations should apply for approved-but-unpublished v1 projection test",
        );
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

        let slug = "module-system-shadow-v2-only";
        let _approved = create_approved_publish_request_for_slug(&ctx, slug).await;
        let base_router = App::routes(&ctx)
            .to_router::<App>(ctx.clone(), axum::Router::new())
            .expect("base router should build");
        let response = base_router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/catalog/{slug}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("v1 catalog detail request should complete");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
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

        let validate_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v2/catalog/publish/rpr_test/validate")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": true
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("registry-only validate request should complete");
        assert_eq!(validate_response.status(), StatusCode::NOT_FOUND);

        let stage_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v2/catalog/publish/rpr_test/stages")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": true,
                            "stage": "compile_smoke",
                            "status": "passed",
                            "detail": "Registry-only host must stay read-only",
                            "requeue": false
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("registry-only stage request should complete");
        assert_eq!(stage_response.status(), StatusCode::NOT_FOUND);

        let request_changes_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v2/catalog/publish/rpr_test/request-changes")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": true,
                            "reason": "Registry-only host must stay read-only",
                            "reason_code": "artifact_mismatch"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("registry-only request-changes request should complete");
        assert_eq!(request_changes_response.status(), StatusCode::NOT_FOUND);

        let hold_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v2/catalog/publish/rpr_test/hold")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": true,
                            "reason": "Registry-only host must stay read-only",
                            "reason_code": "release_window"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("registry-only hold request should complete");
        assert_eq!(hold_response.status(), StatusCode::NOT_FOUND);

        let resume_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v2/catalog/publish/rpr_test/resume")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "schema_version": 1,
                            "dry_run": true,
                            "reason": "Registry-only host must stay read-only",
                            "reason_code": "review_complete"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("registry-only resume request should complete");
        assert_eq!(resume_response.status(), StatusCode::NOT_FOUND);

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
                            "new_owner_user_id": uuid::Uuid::new_v4(),
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
        create_approved_publish_request_for_slug(ctx, "blog").await
    }

    async fn create_approved_publish_request_for_slug(
        ctx: &loco_rs::app::AppContext,
        slug: &str,
    ) -> crate::models::registry_publish_request::Model {
        let governance =
            crate::services::registry_governance::RegistryGovernanceService::new(ctx.db.clone());
        let created = governance
            .create_publish_request(
                &crate::services::marketplace_catalog::RegistryPublishRequest {
                    schema_version: 1,
                    dry_run: false,
                    module: crate::services::marketplace_catalog::RegistryPublishModuleRequest {
                        slug: slug.to_string(),
                        version: "0.1.0".to_string(),
                        crate_name: format!("rustok-{}", slug.replace('-', "_")),
                        default_locale: "en".to_string(),
                        name: format!("{} module", slug),
                        description: format!(
                            "Registry publish request test contract preview for slug {}.",
                            slug
                        ),
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
                &registry_authority("user:00000000-0000-0000-0000-000000000111"),
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
            claim_id: Set(None),
            claimed_by: Set(None),
            claim_expires_at: Set(None),
            last_heartbeat_at: Set(None),
            runner_kind: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&ctx.db)
        .await
        .expect("validation stage should insert")
    }

    async fn insert_registry_owner_binding(
        ctx: &loco_rs::app::AppContext,
        slug: &str,
        owner_principal: &str,
    ) -> crate::models::registry_module_owner::Model {
        let now = chrono::Utc::now();
        crate::models::registry_module_owner::ActiveModel {
            slug: Set(slug.to_string()),
            owner_principal: Set(principal_json(owner_principal)),
            bound_by: Set(principal_json("test:setup")),
            bound_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&ctx.db)
        .await
        .expect("owner binding should insert")
    }

    async fn insert_active_release(
        ctx: &loco_rs::app::AppContext,
        slug: &str,
        version: &str,
        publisher: Option<&str>,
        request_id: Option<&str>,
    ) -> crate::models::registry_module_release::Model {
        let now = chrono::Utc::now();
        let release = crate::models::registry_module_release::ActiveModel {
            id: Set(format!("rrl_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request_id.map(ToString::to_string)),
            slug: Set(slug.to_string()),
            version: Set(version.to_string()),
            crate_name: Set(format!("rustok-{}", slug.replace('-', "_"))),
            default_locale: Set("en".to_string()),
            ownership: Set("first_party".to_string()),
            trust_level: Set("verified".to_string()),
            license: Set("MIT".to_string()),
            entry_type: Set(Some("BlogModule".to_string())),
            marketplace: Set(serde_json::json!({
                "category": "content",
                "tags": ["content"]
            })),
            ui_packages: Set(serde_json::json!({})),
            status: Set(
                crate::models::registry_module_release::RegistryModuleReleaseStatus::Active,
            ),
            publisher: Set(principal_json(publisher.unwrap_or("publisher:blog"))),
            artifact_storage_key: Set(Some(format!("registry/artifacts/{slug}/{version}.tar"))),
            checksum_sha256: Set(Some("deadbeef".repeat(8))),
            artifact_size: Set(Some(1024)),
            yanked_reason: Set(None),
            yanked_by: Set(None),
            yanked_at: Set(None),
            published_at: Set(now),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&ctx.db)
        .await
        .expect("active release should insert");
        crate::models::registry_module_release_translation::ActiveModel {
            release_id: Set(release.id.clone()),
            locale: Set("en".to_string()),
            name: Set(format!("{} module", slug)),
            description: Set(format!(
                "Published release test contract preview for slug {}.",
                slug
            )),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&ctx.db)
        .await
        .expect("release translation should insert");
        release
    }

    async fn send_json_request(
        router: axum::Router,
        method: Method,
        uri: impl Into<String>,
        payload: Value,
        auth: Option<AuthContext>,
    ) -> axum::response::Response {
        let mut request = Request::builder()
            .method(method)
            .uri(uri.into())
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request");
        if let Some(auth) = auth {
            request.extensions_mut().insert(AuthContextExtension(auth));
        }

        router
            .oneshot(request)
            .await
            .expect("json request should complete")
    }

    async fn fetch_publish_status_payload(
        router: axum::Router,
        request_id: &str,
        auth: Option<AuthContext>,
    ) -> Value {
        let mut request = Request::builder()
            .uri(format!("/v2/catalog/publish/{request_id}"))
            .body(Body::empty())
            .expect("request");
        if let Some(auth) = auth {
            request.extensions_mut().insert(AuthContextExtension(auth));
        }

        let response = router
            .oneshot(request)
            .await
            .expect("publish status request should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("publish status body should read");
        assert_eq!(
            status,
            StatusCode::OK,
            "unexpected /v2/catalog/publish/{{request_id}} response body: {}",
            String::from_utf8_lossy(&body)
        );

        serde_json::from_slice(&body).expect("publish status response should be valid json")
    }

    fn publish_status_action_keys(payload: &Value) -> Vec<String> {
        payload
            .get("governanceActions")
            .and_then(Value::as_array)
            .map(|actions| {
                actions
                    .iter()
                    .filter_map(|action| action.get("key").and_then(Value::as_str))
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }
}
