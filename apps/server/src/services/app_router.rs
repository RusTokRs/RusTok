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
#[allow(unused_imports)]
use rustok_admin as _;
#[cfg(feature = "embed-storefront")]
#[allow(unused_imports)]
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
pub fn build_storefront_router() -> AxumRouter {
    rustok_storefront::router()
}

#[cfg(not(feature = "embed-storefront"))]
pub fn build_storefront_router() -> AxumRouter {
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
        let server_fn_runtime_ctx = server_fn_runtime_ctx.with_shared_value(alloy_runtime);
        server_fn_runtime_ctx.with_shared_value(
            crate::services::registry_governance::alloy_release_governance_handle(
                middleware_runtime_ctx.db_clone(),
            ),
        )
    } else {
        server_fn_runtime_ctx
    };
    let server_fn_registry = runtime.registry.clone();

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
            .then(build_storefront_router),
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
}
