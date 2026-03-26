use axum::middleware as axum_middleware;
use axum::Extension;
use axum::Router as AxumRouter;
use loco_rs::app::AppContext;

use crate::middleware;
use crate::middleware::rate_limit::rate_limit_for_paths;
use crate::services::app_runtime::AppRuntimeBootstrap;

#[cfg(feature = "embed-admin-assets")]
use axum::response::IntoResponse;
#[cfg(feature = "embed-admin-assets")]
use rust_embed::RustEmbed;

#[cfg(feature = "embed-admin")]
#[derive(RustEmbed)]
#[folder = "../../apps/admin/dist"]
struct AdminAssets;

#[cfg(feature = "embed-admin")]
pub fn build_admin_router() -> AxumRouter {
    AxumRouter::new().fallback(move |uri: axum::http::Uri| async move {
        let path = uri.path().trim_start_matches('/');
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
    alloy_rest_router: AxumRouter,
    admin_router: Option<AxumRouter>,
    storefront_router: Option<AxumRouter>,
) -> AxumRouter {
    let router = router.nest("/api/alloy", alloy_rest_router);
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
    ctx: &AppContext,
    runtime: AppRuntimeBootstrap,
) -> AxumRouter {
    mount_application_shell(
        router,
        runtime.alloy_rest_router,
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
    .layer(Extension(runtime.graphql_schema))
    .layer(axum_middleware::from_fn_with_state(
        runtime.oauth_rate_limit_state,
        rate_limit_for_paths,
    ))
    .layer(axum_middleware::from_fn_with_state(
        runtime.auth_rate_limit_state,
        rate_limit_for_paths,
    ))
    .layer(axum_middleware::from_fn_with_state(
        runtime.api_rate_limit_state,
        rate_limit_for_paths,
    ))
    .layer(axum_middleware::from_fn_with_state(
        ctx.clone(),
        middleware::auth_context::resolve_optional,
    ))
    .layer(axum_middleware::from_fn_with_state(
        ctx.clone(),
        middleware::tenant::resolve,
    ))
    .layer(axum_middleware::from_fn(
        middleware::security_headers::security_headers,
    ))
    .layer(axum_middleware::from_fn(middleware::locale::resolve_locale))
}

#[cfg(test)]
mod tests {
    use super::mount_application_shell;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use axum::Router as AxumRouter;
    use tower::ServiceExt;

    #[cfg(not(feature = "embed-admin"))]
    use super::build_admin_router;

    #[tokio::test]
    async fn mount_application_shell_routes_requests_to_nested_routers() {
        let alloy_router = AxumRouter::new().route("/health", get(|| async { "alloy" }));
        let admin_router = AxumRouter::new().route("/dashboard", get(|| async { "admin" }));
        let storefront_router = AxumRouter::new().route("/", get(|| async { "storefront" }));

        let app = mount_application_shell(
            AxumRouter::new(),
            alloy_router,
            Some(admin_router),
            Some(storefront_router),
        );

        let alloy_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/alloy/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(alloy_response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(alloy_response.into_body(), usize::MAX)
                .await
                .unwrap(),
            "alloy"
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
        let alloy_router = AxumRouter::new().route("/health", get(|| async { "alloy" }));
        let app = mount_application_shell(AxumRouter::new(), alloy_router, None, None);

        let alloy_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/alloy/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(alloy_response.status(), StatusCode::OK);

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
        let alloy_router = AxumRouter::new().route("/health", get(|| async { "alloy" }));
        let admin_router = AxumRouter::new().route("/dashboard", get(|| async { "admin" }));
        let app =
            mount_application_shell(AxumRouter::new(), alloy_router, Some(admin_router), None);

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
