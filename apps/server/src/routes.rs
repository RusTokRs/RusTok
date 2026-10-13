//! Shared Axum router state and global routing table for the executable server host.

use axum::response::IntoResponse;
use crate::common::settings::RuntimeHostMode;
use crate::error::Error;
use crate::services::server_runtime_context::ServerAuthRuntime;

/// A router which still requires the host's auth/runtime state.
pub type ServerRouter = axum::Router<ServerAuthRuntime>;

/// Canonical fallback handler for unmatched routes, returning structured JSON error envelope.
pub async fn not_found_fallback(uri: axum::http::Uri) -> impl IntoResponse {
    tracing::debug!(path = %uri.path(), "Route not found");
    Error::NotFound
}

/// Builds the un-configured global application router matching the host mode.
pub fn build_application_router(host_mode: RuntimeHostMode) -> ServerRouter {
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

    router
        .merge(crate::controllers::graphql::router())
        .merge(crate::controllers::installer::router())
        .merge(crate::controllers::mcp::router())
        .merge(crate::controllers::oauth::router())
        .merge(crate::controllers::oauth_metadata::router())
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

    #[tokio::test]
    async fn worker_mode_router_routes_health_check() {
        let router = build_application_router(RuntimeHostMode::Worker);
        let db = rustok_test_utils::setup_test_db().await;
        let ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(
            db,
            crate::common::settings::RustokSettings::default(),
        );
        let app = router.with_state(ServerAuthRuntime::without_auth(ctx));

        let req = Request::builder()
            .uri("/health")
            .body(axum::body::Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
