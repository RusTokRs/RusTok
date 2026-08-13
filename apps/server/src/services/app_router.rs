use axum::Extension;
use axum::Router as AxumRouter;
use axum::middleware as axum_middleware;
use axum::routing::post;
use leptos::prelude::provide_context;
use leptos_axum::handle_server_fns_with_context;
use rustok_api::{HostRuntimeContext, HostSettingsSnapshot};
use rustok_core::ModuleRuntimeExtensions;
use std::sync::Arc;

#[cfg(feature = "embed-admin")]
use rustok_admin as _;
#[cfg(feature = "embed-storefront")]
use rustok_storefront as _;

use crate::common::settings::RustokSettings;
use crate::error::{Error, Result};
use crate::middleware;
use crate::middleware::rate_limit::rate_limit_for_paths;
use crate::services::app_runtime::AppRuntimeBootstrap;
use crate::services::commerce_provider_runtime::attach_commerce_provider_registries;
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext};

pub(crate) mod routes_codegen {
    include!(concat!(env!("OUT_DIR"), "/app_routes_codegen.rs"));
}

#[cfg(feature = "embed-admin-assets")]
use axum::response::IntoResponse;
#[cfg(feature = "embed-admin-assets")]
use axum::{
    http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG},
    response::Response as AxumResponse,
};
#[cfg(feature = "embed-admin-assets")]
use rust_embed::RustEmbed;
#[cfg(feature = "embed-admin-assets")]
use rustok_web::CspNonce;
#[cfg(feature = "embed-admin-assets")]
use sha2::{Digest, Sha256};

#[cfg(feature = "embed-admin")]
#[derive(RustEmbed)]
#[folder = "../../apps/admin/dist"]
struct AdminAssets;

#[cfg(feature = "embed-admin")]
pub fn build_admin_router() -> AxumRouter {
    AxumRouter::new().fallback(move |request: axum::extract::Request| async move {
        let path = request.uri().path().trim_start_matches('/');
        let path = if path.is_empty() { "index.html" } else { path };
        let csp_nonce = request.extensions().get::<CspNonce>().cloned();

        match AdminAssets::get(path) {
            Some(content) => admin_asset_response(path, content.data, csp_nonce.as_ref()),
            None => match AdminAssets::get("index.html") {
                Some(content) => {
                    admin_asset_response("index.html", content.data, csp_nonce.as_ref())
                }
                None => (axum::http::StatusCode::NOT_FOUND, "Admin UI not bundled").into_response(),
            },
        }
    })
}

#[cfg(feature = "embed-admin-assets")]
fn admin_asset_response(
    path: &str,
    bytes: std::borrow::Cow<'static, [u8]>,
    csp_nonce: Option<&CspNonce>,
) -> AxumResponse {
    let is_document = path.ends_with("index.html");
    let raw_bytes = bytes.into_owned();
    let response_bytes = if is_document {
        if let Some(nonce) = csp_nonce {
            match std::str::from_utf8(raw_bytes.as_slice()) {
                Ok(html) => nonce_trusted_admin_elements(html, nonce).into_bytes(),
                Err(error) => {
                    tracing::error!(%error, path, "Embedded admin document is not valid UTF-8");
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        "Admin UI document is invalid",
                    )
                        .into_response();
                }
            }
        } else {
            raw_bytes
        }
    } else {
        raw_bytes
    };

    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut response = ([(CONTENT_TYPE, mime.as_ref())], response_bytes.clone()).into_response();
    let cache_control = if is_document {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };

    response.headers_mut().insert(
        CACHE_CONTROL,
        cache_control.parse().expect("cache-control header"),
    );
    // The document body carries a per-response nonce, so a stable ETag would be incorrect. Static
    // immutable assets retain their content-derived validators.
    if !is_document {
        let digest = hex::encode(Sha256::digest(response_bytes.as_slice()));
        response.headers_mut().insert(
            ETAG,
            format!("\"{}\"", &digest[..16])
                .parse()
                .expect("etag header"),
        );
    }
    response
}

#[cfg(feature = "embed-admin-assets")]
fn nonce_trusted_admin_elements(html: &str, csp_nonce: &CspNonce) -> String {
    // This transformation is intentionally limited to the immutable bundled index document. It
    // must never be applied to tenant or user-authored HTML because that would authorize injected
    // script or style elements.
    let script_opening = format!(r#"<script nonce="{}""#, csp_nonce.as_str());
    let style_opening = format!(r#"<style nonce="{}""#, csp_nonce.as_str());
    html.replace("<script", script_opening.as_str())
        .replace("<style", style_opening.as_str())
}

#[cfg(not(feature = "embed-admin"))]
pub fn build_admin_router() -> AxumRouter {
    AxumRouter::new().fallback(|| async {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Admin UI is disabled. Rebuild server with feature `embed-admin-assets` and prepare apps/admin/dist artifacts.",
        )
    })
}

#[cfg(feature = "embed-storefront")]
pub fn build_storefront_router(runtime: HostRuntimeContext) -> AxumRouter {
    let router = rustok_storefront::router(runtime);
    #[cfg(feature = "richtext-assets")]
    let router = router.merge(rustok_content::richtext_assets::router());
    router
}

#[cfg(not(feature = "embed-storefront"))]
pub fn build_storefront_router(_runtime: HostRuntimeContext) -> AxumRouter {
    AxumRouter::new().fallback(|| async {
        (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Storefront UI is disabled. Rebuild server with feature `embed-storefront`.",
        )
    })
}

pub fn mount_application_shell(
    router: AxumRouter,
    admin_router: Option<AxumRouter>,
    storefront_router: Option<AxumRouter>,
) -> AxumRouter {
    let router = if let Some(admin_router) = admin_router {
        router.nest("/admin", admin_router)
    } else {
        router
    };

    if let Some(storefront_router) = storefront_router {
        router.merge(storefront_router)
    } else {
        router
    }
}

pub fn compose_application_router(
    router: AxumRouter,
    middleware_runtime_ctx: ServerRuntimeContext,
    auth_runtime: ServerAuthRuntime,
    settings_snapshot: serde_json::Value,
    runtime: AppRuntimeBootstrap,
    rustok_settings: &RustokSettings,
) -> Result<AxumRouter> {
    // Observability and global registry boundaries are not tied to a particular
    // deployment profile. Install them before profile-specific middleware
    // diverges so registry-only cannot bypass the same guards.
    let router = router
        .layer(axum_middleware::from_fn(
            middleware::metrics_auth::require_bearer,
        ))
        .layer(axum_middleware::from_fn_with_state(
            middleware_runtime_ctx.clone(),
            middleware::registry_artifact_access::enforce,
        ))
        .layer(axum_middleware::from_fn_with_state(
            middleware_runtime_ctx.clone(),
            middleware::registry_remote_claim::claim_atomic,
        ));

    if rustok_settings.runtime.is_registry_only() || rustok_settings.runtime.is_worker_only() {
        return Ok(router
            .layer(Extension(runtime.registry))
            .layer(axum_middleware::from_fn_with_state(
                runtime.rate_limit_state,
                rate_limit_for_paths,
            ))
            .layer(axum_middleware::from_fn_with_state(
                auth_runtime,
                middleware::auth_context::resolve_optional,
            ))
            .layer(axum_middleware::from_fn_with_state(
                middleware_runtime_ctx,
                middleware::locale::resolve_locale,
            ))
            .layer(axum_middleware::from_fn(
                middleware::security_headers::security_headers,
            )));
    }

    let server_fn_runtime_ctx = {
        let runtime_ctx = HostRuntimeContext::new(middleware_runtime_ctx.db_clone())
            .with_shared_value(transactional_event_bus_from_context(
                &middleware_runtime_ctx,
            ))
            .with_shared_value(rustok_api::SharedEventDeliveryControl(Arc::new(
                crate::services::event_delivery_control_adapter::ServerEventDeliveryControl::new(
                    middleware_runtime_ctx.clone(),
                ),
            )))
            .with_shared_value(rustok_iggy_connector::SharedIggyConnectorControl(Arc::new(
                crate::services::iggy_connector_control_adapter::ServerIggyConnectorControl::new(
                    middleware_runtime_ctx.clone(),
                ),
            )))
            .with_shared_value(HostSettingsSnapshot::new(settings_snapshot));
        let runtime_ctx = if let Some(registry) =
            middleware_runtime_ctx.shared_get::<rustok_core::ModuleRegistry>()
        {
            runtime_ctx.with_shared_value(registry)
        } else {
            runtime_ctx
        };
        let runtime_ctx = if let Some(storage) =
            middleware_runtime_ctx.shared_get::<rustok_storage::StorageRuntime>()
        {
            runtime_ctx.with_shared_value(storage)
        } else {
            runtime_ctx
        };
        let runtime_ctx = if let Some(catalog) =
            middleware_runtime_ctx.shared_get::<rustok_modules::SharedModuleMarketplaceCatalog>()
        {
            runtime_ctx.with_shared_value(catalog)
        } else {
            runtime_ctx
        };
        let runtime_ctx = if let Some(build_control) =
            middleware_runtime_ctx.shared_get::<rustok_build::SharedBuildControl>()
        {
            runtime_ctx.with_shared_value(build_control)
        } else {
            runtime_ctx
        };
        if let Some(extensions) =
            middleware_runtime_ctx.shared_get::<Arc<ModuleRuntimeExtensions>>()
        {
            extensions
                .apply_to_host_runtime(runtime_ctx)
                .with_shared_value(extensions)
        } else {
            runtime_ctx
        }
    };
    let server_fn_runtime_ctx =
        attach_commerce_provider_registries(server_fn_runtime_ctx, &middleware_runtime_ctx);
    #[cfg(feature = "mod-alloy")]
    let server_fn_runtime_ctx = if let Some(alloy_runtime) =
        middleware_runtime_ctx.shared_get::<alloy::SharedAlloyRuntime>()
    {
        let storage = middleware_runtime_ctx
            .shared_get::<rustok_storage::StorageRuntime>()
            .ok_or_else(|| {
                Error::Message(
                    "Alloy published-release import requires initialized durable storage"
                        .to_string(),
                )
            })?;
        let server_fn_runtime_ctx = server_fn_runtime_ctx.with_shared_value(alloy_runtime);
        let server_fn_runtime_ctx = server_fn_runtime_ctx.with_shared_value(
            crate::services::registry_governance::alloy_release_governance_handle(
                middleware_runtime_ctx.db_clone(),
            ),
        );
        server_fn_runtime_ctx.with_shared_value(
            crate::services::registry_governance::alloy_published_rhai_source_provider_handle(
                middleware_runtime_ctx.db_clone(),
                storage,
            ),
        )
    } else {
        server_fn_runtime_ctx
    };
    let server_fn_registry = runtime.registry.clone();
    let storefront_runtime_ctx = server_fn_runtime_ctx.clone();

    let router =
        routes_codegen::append_optional_module_axum_routers(router, &server_fn_runtime_ctx)
            .map_err(|error| {
                Error::BadRequest(format!(
                    "Failed to compose optional module Axum routes: {error}"
                ))
            })?;

    let router = mount_application_shell(
        router.route(
            "/api/fn/{*fn_name}",
            post(move |req| {
                let runtime_ctx = server_fn_runtime_ctx.clone();
                let registry = server_fn_registry.clone();
                async move {
                    handle_server_fns_with_context(
                        move || {
                            provide_context(runtime_ctx.clone());
                            provide_context(registry.clone());
                        },
                        req,
                    )
                    .await
                }
            }),
        ),
        runtime
            .deployment_surfaces
            .embed_admin
            .then(build_admin_router),
        runtime
            .deployment_surfaces
            .embed_storefront
            .then(|| build_storefront_router(storefront_runtime_ctx)),
    )
    .layer(Extension(runtime.registry))
    .layer(Extension(runtime.graphql_schema));
    #[cfg(feature = "mod-cart")]
    let router = router.layer(axum_middleware::from_fn(
        rustok_cart::guest_access_http::resolve,
    ));

    Ok(router
        .layer(axum_middleware::from_fn_with_state(
            middleware_runtime_ctx.clone(),
            middleware::mcp_scaffold_workspace::authorize_workspace,
        ))
        .layer(axum_middleware::from_fn_with_state(
            runtime.rate_limit_state,
            rate_limit_for_paths,
        ))
        .layer(axum_middleware::from_fn_with_state(
            middleware_runtime_ctx.clone(),
            middleware::channel::resolve,
        ))
        .layer(axum_middleware::from_fn_with_state(
            auth_runtime.clone(),
            middleware::invite_accept::consume_once,
        ))
        .layer(axum_middleware::from_fn_with_state(
            auth_runtime,
            middleware::auth_context::resolve_optional,
        ))
        .layer(axum_middleware::from_fn_with_state(
            middleware_runtime_ctx.clone(),
            middleware::locale::resolve_locale,
        ))
        .layer(axum_middleware::from_fn_with_state(
            middleware_runtime_ctx,
            middleware::tenant::resolve,
        ))
        .layer(axum_middleware::from_fn(
            middleware::security_headers::security_headers,
        )))
}

#[cfg(test)]
mod tests {
    use super::mount_application_shell;
    use axum::Router as AxumRouter;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;

    #[cfg(not(feature = "embed-admin"))]
    use super::build_admin_router;
    #[cfg(feature = "embed-admin-assets")]
    use super::nonce_trusted_admin_elements;
    #[cfg(feature = "embed-admin-assets")]
    use rustok_web::CspNonce;

    #[tokio::test]
    async fn mount_application_shell_routes_requests_to_nested_routers() {
        let admin_router = AxumRouter::new().route("/dashboard", get(|| async { "admin" }));
        let storefront_router = AxumRouter::new().route("/", get(|| async { "storefront" }));

        let app = mount_application_shell(
            AxumRouter::new(),
            Some(admin_router),
            Some(storefront_router),
        );

        let admin_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/dashboard")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(admin_response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(admin_response.into_body(), usize::MAX)
                .await
                .unwrap(),
            "admin"
        );

        let storefront_response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(storefront_response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(storefront_response.into_body(), usize::MAX)
                .await
                .unwrap(),
            "storefront"
        );
    }

    #[tokio::test]
    async fn mount_application_shell_skips_admin_and_storefront_for_headless_profile() {
        let app = mount_application_shell(AxumRouter::new(), None, None);

        let root_response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(root_response.status(), StatusCode::NOT_FOUND);

        let admin_response = app
            .oneshot(
                Request::builder()
                    .uri("/admin")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(admin_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn mount_application_shell_supports_server_with_admin_profile() {
        let admin_router = AxumRouter::new().route("/dashboard", get(|| async { "admin" }));
        let app = mount_application_shell(AxumRouter::new(), Some(admin_router), None);

        let admin_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/dashboard")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(admin_response.status(), StatusCode::OK);

        let root_response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(root_response.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(feature = "embed-admin-assets")]
    #[test]
    fn trusted_admin_asset_scripts_and_styles_receive_csp_nonce() {
        let nonce = CspNonce::generate();
        let html = r#"<style>.app{display:block}</style><script src="/pkg/app.js"></script><script>bootstrap()</script>"#;

        let rendered = nonce_trusted_admin_elements(html, &nonce);

        assert_eq!(
            rendered,
            format!(
                r#"<style nonce="{0}">.app{{display:block}}</style><script nonce="{0}" src="/pkg/app.js"></script><script nonce="{0}">bootstrap()</script>"#,
                nonce.as_str()
            )
        );
    }

    #[cfg(not(feature = "embed-admin"))]
    #[tokio::test]
    async fn disabled_admin_router_returns_service_unavailable() {
        let response = build_admin_router()
            .oneshot(Request::builder().uri("/any").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[cfg(feature = "mod-translation")]
    mod translation {
        use std::sync::Arc;

        use axum::{
            Router,
            body::{Body, to_bytes},
            http::{Request, StatusCode, header},
        };
        use chrono::{Duration, Utc};
        use rustok_api::{Action, Resource};
        use rustok_build::DeploymentProfile;
        use rustok_cache::CacheService;
        use rustok_channel::ChannelService;
        use rustok_core::{ModuleRuntimeExtensions, UserRole, events::EventTransport};
        use rustok_outbox::{OutboxTransport, SysEventsMigration};
        use rustok_tenant::{
            PortActor, PortContext, TenantLocalePolicyPort, TenantService, entities::tenant_locale,
        };
        use sea_orm::{ActiveModelTrait, EntityTrait, Set};
        use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};
        use tower::ServiceExt;
        use uuid::Uuid;

        // Link the owner-owned server-function inventory into this isolated host profile.
        use rustok_translation_admin as _;

        use super::super::compose_application_router;
        use crate::{
            auth::{AuthConfig, encode_access_token},
            common::settings::{RuntimeHostMode, RustokSettings},
            graphql::schema::{Mutation, Query, Subscription},
            middleware::{
                rate_limit::{
                    PathRateLimitMiddlewareState, PathRateLimitPolicy, RateLimitConfig, RateLimiter,
                },
                tenant,
            },
            models::{
                _entities::{permissions, role_permissions, roles, user_roles},
                sessions, tenants, users,
            },
            modules::{DeploymentSurfaceContract, build_registry},
            services::{
                app_runtime::AppRuntimeBootstrap,
                server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext},
            },
        };

        const EXECUTE_PATH: &str = "/api/fn/translation-admin/execute";
        const READ_POLICY_BODY: &str = "operation%5Boperation%5D=read_policy";

        async fn setup_database() -> sea_orm::DatabaseConnection {
            const PLATFORM_MIGRATIONS: &[&str] = &[
                "m20250101_000001_create_tenants",
                "m20250101_000002_create_users",
                "m20250101_000004_create_sessions",
                "m20250101_000005_create_roles_and_permissions",
                "m20250130_000004_create_tenant_locales",
            ];
            const CHANNEL_MIGRATIONS: &[&str] = &[
                "m20260325_000001_create_channels",
                "m20260327_000006_add_channels_is_default",
                "m20260327_000007_create_channel_resolution_policy_sets",
                "m20260327_000008_create_channel_resolution_policy_rules",
            ];

            let database = rustok_test_utils::db::setup_test_db().await;
            let manager = SchemaManager::new(&database);

            for migration in rustok_migrations::Migrator::migrations() {
                if PLATFORM_MIGRATIONS.contains(&migration.name().to_string().as_str()) {
                    migration
                        .up(&manager)
                        .await
                        .expect("migrate platform request boundary");
                }
            }
            for migration in rustok_tenant::migrations::migrations() {
                migration
                    .up(&manager)
                    .await
                    .expect("migrate tenant locale policy");
            }
            for migration in rustok_channel::migrations::migrations() {
                if CHANNEL_MIGRATIONS.contains(&migration.name().to_string().as_str()) {
                    migration
                        .up(&manager)
                        .await
                        .expect("migrate channel request boundary");
                }
            }
            SysEventsMigration
                .up(&manager)
                .await
                .expect("migrate transactional outbox");
            for migration in rustok_translation::migrations::migrations() {
                migration.up(&manager).await.expect("migrate Translation");
            }

            database
        }

        async fn insert_tenant(
            database: &sea_orm::DatabaseConnection,
            slug: &str,
        ) -> tenants::Model {
            tenants::ActiveModel::new("Translation router tenant", slug)
                .insert(database)
                .await
                .expect("insert tenant")
        }

        async fn insert_reader(
            database: &sea_orm::DatabaseConnection,
            tenant_id: Uuid,
        ) -> (users::Model, Uuid) {
            let user = users::ActiveModel::new(
                tenant_id,
                "translation-router@example.com",
                "not-used-by-token-validation",
            )
            .insert(database)
            .await
            .expect("insert user");
            let session_id = Uuid::new_v4();
            let mut session = sessions::ActiveModel::new(
                tenant_id,
                user.id,
                "translation-router-refresh-token".to_string(),
                Utc::now() + Duration::hours(1),
                None,
                None,
            );
            session.id = Set(session_id);
            session.insert(database).await.expect("insert session");

            let now = Utc::now().fixed_offset();
            let role_id = Uuid::new_v4();
            roles::Entity::insert(roles::ActiveModel {
                id: Set(role_id),
                tenant_id: Set(tenant_id),
                name: Set("Translation reader".to_string()),
                slug: Set("translation_reader".to_string()),
                description: Set(Some(
                    "Application-router Translation evidence role".to_string(),
                )),
                is_system: Set(false),
                created_at: Set(now),
                updated_at: Set(now),
            })
            .exec(database)
            .await
            .expect("insert role");

            let permission_id = Uuid::new_v4();
            permissions::Entity::insert(permissions::ActiveModel {
                id: Set(permission_id),
                tenant_id: Set(tenant_id),
                resource: Set(Resource::Translations.to_string()),
                action: Set(Action::Read.to_string()),
                description: Set(Some("Read Translation policy".to_string())),
                created_at: Set(now),
            })
            .exec(database)
            .await
            .expect("insert permission");
            role_permissions::Entity::insert(role_permissions::ActiveModel {
                id: Set(Uuid::new_v4()),
                role_id: Set(role_id),
                permission_id: Set(permission_id),
            })
            .exec(database)
            .await
            .expect("bind role permission");
            user_roles::Entity::insert(user_roles::ActiveModel {
                id: Set(Uuid::new_v4()),
                user_id: Set(user.id),
                role_id: Set(role_id),
            })
            .exec(database)
            .await
            .expect("bind user role");

            (user, session_id)
        }

        async fn insert_locales(database: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
            let now = Utc::now().fixed_offset();
            for (locale, name, is_default, fallback_locale) in [
                ("en", "English", true, None),
                ("de-DE", "Deutsch", false, Some("en")),
            ] {
                tenant_locale::Entity::insert(tenant_locale::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.to_string()),
                    name: Set(name.to_string()),
                    native_name: Set(name.to_string()),
                    is_default: Set(is_default),
                    is_enabled: Set(true),
                    fallback_locale: Set(fallback_locale.map(str::to_string)),
                    policy_revision: Set(0),
                    created_at: Set(now),
                    updated_at: Set(now),
                })
                .exec(database)
                .await
                .expect("insert tenant locale");
            }
        }

        fn request(tenant_id: Uuid, access_token: &str) -> Request<Body> {
            Request::builder()
                .method("POST")
                .uri(EXECUTE_PATH)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .header("X-Tenant-ID", tenant_id.to_string())
                .header(header::ACCEPT_LANGUAGE, "de-DE,de;q=0.9")
                .body(Body::from(READ_POLICY_BODY))
                .expect("build Translation server-function request")
        }

        #[tokio::test]
        async fn application_router_executes_authenticated_server_function() {
            let database = setup_database().await;
            let tenant = insert_tenant(&database, "translation-router").await;
            let other_tenant = insert_tenant(&database, "translation-router-other").await;
            insert_locales(&database, tenant.id).await;
            let (user, session_id) = insert_reader(&database, tenant.id).await;
            TenantService::new(database.clone())
                .read_locale_policy(
                    PortContext::new(
                        tenant.id.to_string(),
                        PortActor::service("rustok-server.translation-router-test"),
                        "de-DE",
                        "translation-router-locale-preflight",
                    )
                    .with_deadline(std::time::Duration::from_secs(2)),
                )
                .await
                .expect("read tenant locale policy through production port");
            let channels = ChannelService::new(database.clone());
            channels
                .list_active_resolution_rules(tenant.id)
                .await
                .expect("read channel resolution policies");
            channels
                .get_default_channel(tenant.id)
                .await
                .expect("read default channel");

            let mut settings = RustokSettings::default();
            settings.runtime.host_mode = RuntimeHostMode::Api;
            let runtime_context = ServerRuntimeContext::new(database.clone(), settings.clone());
            let cache = CacheService::from_url(None);
            tenant::init_tenant_cache_infrastructure(&runtime_context, &cache).await;
            runtime_context.shared_insert(cache);
            let event_transport: Arc<dyn EventTransport> =
                Arc::new(OutboxTransport::new(database.clone()));
            runtime_context.shared_insert(event_transport);
            runtime_context.shared_insert(Arc::new(ModuleRuntimeExtensions::default()));

            let auth_config =
                AuthConfig::new("translation-router-test-secret-with-32-bytes".to_string());
            let access_token = encode_access_token(
                &auth_config,
                user.id,
                tenant.id,
                UserRole::Customer,
                session_id,
            )
            .expect("encode access token");
            let rate_limit_state = PathRateLimitMiddlewareState {
                policies: Arc::new(vec![PathRateLimitPolicy {
                    limiter: Arc::new(RateLimiter::new(RateLimitConfig::disabled())),
                    prefixes: Arc::new(vec!["/api/"]),
                }]),
                auth_config: Some(auth_config.clone()),
                trusted_auth_dimensions: false,
                request_trust: settings.runtime.request_trust.clone(),
            };
            let runtime = AppRuntimeBootstrap {
                deployment_surfaces: DeploymentSurfaceContract {
                    profile: DeploymentProfile::HeadlessApi,
                    embed_admin: false,
                    embed_storefront: false,
                },
                registry: build_registry(),
                graphql_schema: Arc::new(
                    async_graphql::Schema::build(
                        Query::default(),
                        Mutation::default(),
                        Subscription::default(),
                    )
                    .finish(),
                ),
                rate_limit_state,
            };
            let app = compose_application_router(
                Router::new(),
                runtime_context.clone(),
                ServerAuthRuntime::new(runtime_context, auth_config),
                serde_json::json!({}),
                runtime,
                &settings,
            )
            .expect("compose application router");

            let response = app
                .clone()
                .oneshot(request(tenant.id, &access_token))
                .await
                .expect("execute Translation request");
            let status = response.status();
            let headers = response.headers().clone();
            let body = to_bytes(response.into_body(), 1024 * 1024)
                .await
                .expect("read Translation response");
            assert_eq!(
                status,
                StatusCode::OK,
                "unexpected Translation response with headers {headers:?}: {}",
                String::from_utf8_lossy(&body)
            );
            assert_eq!(
                headers
                    .get(header::CONTENT_LANGUAGE)
                    .and_then(|value| value.to_str().ok()),
                Some("de-DE")
            );
            assert_eq!(
                headers
                    .get("x-content-type-options")
                    .and_then(|value| value.to_str().ok()),
                Some("nosniff")
            );
            let payload: serde_json::Value =
                serde_json::from_slice(&body).expect("decode Translation response");
            assert_eq!(payload["result"], "policy");
            assert_eq!(payload["value"]["tenantId"], tenant.id.to_string());
            assert_eq!(payload["value"]["tenantLocalePolicyRevision"], 0);

            let rejected = app
                .oneshot(request(other_tenant.id, &access_token))
                .await
                .expect("execute cross-tenant Translation request");
            assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
            let body = to_bytes(rejected.into_body(), 1024 * 1024)
                .await
                .expect("read cross-tenant rejection");
            assert_eq!(body, "Token belongs to another tenant");
        }
    }
}
