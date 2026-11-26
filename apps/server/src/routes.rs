//! Shared Axum router state and global routing table for the executable server host.

use crate::common::settings::{RuntimeHostMode, RustokSettings};
use crate::error::Error;
use crate::services::server_runtime_context::ServerAuthRuntime;
use axum::response::IntoResponse;

/// A router which still requires the host's auth/runtime state.
pub type ServerRouter = axum::Router<ServerAuthRuntime>;

/// Canonical fallback handler for unmatched routes, returning structured JSON error envelope.
pub async fn not_found_fallback(uri: axum::http::Uri) -> impl IntoResponse {
    tracing::debug!(
        path_group = crate::middleware::http_stack::trace_path_family(uri.path()),
        "Route not found"
    );
    Error::NotFound
}

/// Returns whether this host mode actually mounts the OAuth protocol and
/// discovery surface. Worker and registry-only hosts intentionally omit it.
pub(crate) fn oauth_surface_enabled(
    host_mode: RuntimeHostMode,
    settings: &RustokSettings,
) -> bool {
    !matches!(
        host_mode,
        RuntimeHostMode::Worker | RuntimeHostMode::RegistryOnly
    ) && settings.features.oauth_enabled
}

/// Builds the un-configured global application router matching the host mode
/// and deployment feature gates.
///
/// `oauth_enabled` is a real composition gate: when it is false the OAuth
/// protocol surface and its discovery metadata are not mounted at all. The
/// default remains fail-closed so a missing feature block cannot silently
/// expose an authorization server.
pub fn build_application_router(
    host_mode: RuntimeHostMode,
    settings: &RustokSettings,
) -> ServerRouter {
    let router = axum::Router::new()
        .merge(crate::controllers::health::router())
        .merge(crate::controllers::metrics::router());

    if host_mode == RuntimeHostMode::Worker {
        return router;
    }

    let router = router.merge(crate::controllers::swagger::router());
    if host_mode == RuntimeHostMode::RegistryOnly {
        return router.merge(crate::controllers::marketplace_registry::read_only_router());
    }

    let router = router
        .merge(crate::controllers::marketplace_registry::router())
        .merge(crate::controllers::artifact_http::router())
        .merge(crate::controllers::artifact_permissions::router())
        .merge(crate::controllers::admin_events::router())
        .merge(crate::controllers::auth::router())
        .merge(crate::controllers::channel::router());

    // Flex is an optional capability module. Its standalone REST surface
    // must not exist in a server build where the module is not composed.
    #[cfg(feature = "mod-flex")]
    let router = router.merge(crate::controllers::flex::router());

    let router = router
        .merge(crate::controllers::graphql::router())
        .merge(crate::controllers::installer::router())
        .merge(crate::controllers::mcp::router());

    let router = if oauth_surface_enabled(host_mode, settings) {
        router
            .merge(crate::controllers::oauth::router())
            .merge(crate::controllers::oauth_metadata::router())
    } else {
        router
    };

    router
        .merge(crate::controllers::users::router())
        .merge(crate::channels::builds::router())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use rustok_web::ErrorBody;
    use tower::ServiceExt;

    #[tokio::test]
    async fn not_found_fallback_returns_json_error_envelope() {
        let uri: axum::http::Uri = "/unmapped/path".parse().unwrap();
        let response = not_found_fallback(uri).await.into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let bytes = to_bytes(response.into_body(), 1024 * 64)
            .await
            .expect("read body");
        let body: ErrorBody = serde_json::from_slice(&bytes).expect("parse json");
        assert_eq!(body.code, "not_found");
        assert_eq!(body.message, "Not found");
    }

    #[test]
    fn oauth_surface_gate_matches_host_mode_and_feature_composition() {
        let mut settings = RustokSettings::default();
        settings.features.oauth_enabled = true;

        for host_mode in [
            RuntimeHostMode::Full,
            RuntimeHostMode::Api,
            RuntimeHostMode::AdminSsr,
            RuntimeHostMode::StorefrontSsr,
        ] {
            assert!(
                oauth_surface_enabled(host_mode, &settings),
                "OAuth should be mounted for {host_mode:?}"
            );
        }
        for host_mode in [RuntimeHostMode::Worker, RuntimeHostMode::RegistryOnly] {
            assert!(
                !oauth_surface_enabled(host_mode, &settings),
                "OAuth must not be required for {host_mode:?}"
            );
        }

        settings.features.oauth_enabled = false;
        assert!(!oauth_surface_enabled(RuntimeHostMode::Api, &settings));
    }

    #[tokio::test]
    async fn oauth_surface_is_not_mounted_when_the_feature_gate_is_closed() {
        let settings = crate::common::settings::RustokSettings::default();
        assert!(!settings.features.oauth_enabled);
        let router = build_application_router(RuntimeHostMode::Api, &settings);
        let db = rustok_test_utils::setup_test_db().await;
        let ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(db, settings);
        let app = router.with_state(ServerAuthRuntime::without_auth(ctx));

        for (method, path) in [
            ("POST", "/api/oauth/token"),
            ("POST", "/api/oauth/revoke"),
            ("GET", "/.well-known/oauth-authorization-server"),
            ("GET", "/.well-known/openid-configuration"),
        ] {
            let req = Request::builder()
                .method(method)
                .uri(path)
                .body(axum::body::Body::empty())
                .unwrap();
            let response = app.clone().oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[tokio::test]
    async fn oauth_surface_is_mounted_when_the_feature_gate_is_open() {
        let mut settings = crate::common::settings::RustokSettings::default();
        settings.features.oauth_enabled = true;
        let router = build_application_router(RuntimeHostMode::Api, &settings);
        let db = rustok_test_utils::setup_test_db().await;
        let ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(db, settings);
        let app = router.with_state(ServerAuthRuntime::without_auth(ctx));

        for (method, path) in [
            ("POST", "/api/oauth/token"),
            ("POST", "/api/oauth/revoke"),
            ("GET", "/.well-known/oauth-authorization-server"),
            ("GET", "/.well-known/openid-configuration"),
        ] {
            let req = Request::builder()
                .method(method)
                .uri(path)
                .body(axum::body::Body::empty())
                .unwrap();
            let response = app.clone().oneshot(req).await.unwrap();
            assert_ne!(response.status(), StatusCode::NOT_FOUND, "{path}");
            assert_ne!(response.status(), StatusCode::METHOD_NOT_ALLOWED, "{path}");
        }
    }

    #[tokio::test]
    async fn worker_mode_router_routes_health_check() {
        let settings = crate::common::settings::RustokSettings::default();
        let router = build_application_router(RuntimeHostMode::Worker, &settings);
        let db = rustok_test_utils::setup_test_db().await;
        let ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(db, settings);
        let app = router.with_state(ServerAuthRuntime::without_auth(ctx));

        let req = Request::builder()
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn registry_only_mode_router_mounts_health_and_excludes_auth() {
        let settings = crate::common::settings::RustokSettings::default();
        let router = build_application_router(RuntimeHostMode::RegistryOnly, &settings);
        let db = rustok_test_utils::setup_test_db().await;
        let ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(db, settings);
        let app = router.with_state(ServerAuthRuntime::without_auth(ctx));

        // Health check is mounted
        let req = Request::builder()
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Auth endpoint is NOT mounted in RegistryOnly mode
        let req_auth = Request::builder()
            .method("POST")
            .uri("/api/auth/login")
            .body(axum::body::Body::empty())
            .unwrap();
        let response_auth = app.oneshot(req_auth).await.unwrap();
        assert_eq!(response_auth.status(), StatusCode::NOT_FOUND);
    }
}
