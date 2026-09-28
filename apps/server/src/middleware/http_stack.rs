use std::time::{Duration, Instant};

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderName, header};
use axum::middleware::{Next, from_fn};
use axum::response::Response;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnFailure, DefaultOnResponse, TraceLayer};

use super::cors::build_cors_layer;

pub const DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS: u64 = 30;

pub fn resolve_http_timeout_seconds() -> u64 {
    std::env::var("RUSTOK_HTTP_REQUEST_TIMEOUT_SECONDS")
        .ok()
        .and_then(|val| val.parse::<u64>().ok())
        .filter(|&secs| secs > 0)
        .unwrap_or(DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS)
}

/// Axum middleware to record live Prometheus HTTP metrics into `rustok_telemetry`.
///
/// Bounded path normalization protects Prometheus against metric cardinality explosion
/// from dynamic IDs/UUIDs in URL paths.
pub async fn record_http_metrics(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().as_str().to_string();
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|matched| matched.as_str().to_string())
        .unwrap_or_else(|| sanitize_metrics_path(request.uri().path()));

    let response = next.run(request).await;
    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    rustok_telemetry::HTTP_REQUESTS_TOTAL
        .with_label_values(&[&method, &path, &status])
        .inc();
    rustok_telemetry::HTTP_REQUEST_DURATION_SECONDS
        .with_label_values(&[&method, &path])
        .observe(latency);

    response
}

/// Sanitizes URL path segments replacing dynamic UUIDs, hashes, or numeric IDs with `:id`.
pub fn sanitize_metrics_path(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').collect();
    let sanitized: Vec<String> = segments
        .into_iter()
        .map(|seg| {
            if seg.is_empty() {
                String::new()
            } else if is_identifier_like(seg) {
                ":id".to_string()
            } else {
                seg.to_string()
            }
        })
        .collect();
    sanitized.join("/")
}

fn is_identifier_like(s: &str) -> bool {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    if s.len() == 36 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return true;
    }
    if s.len() >= 32 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    false
}

/// Applies the canonical production HTTP middleware stack:
///
/// 1. `SetRequestIdLayer` + `PropagateRequestIdLayer` (`x-request-id` correlation)
/// 2. `TraceLayer` (per-request spans with latency and status)
/// 3. `SetSensitiveRequestHeadersLayer` (masks `Authorization` and `Cookie`)
/// 4. `record_http_metrics` (populates `HTTP_REQUESTS_TOTAL` and `HTTP_REQUEST_DURATION_SECONDS`)
/// 5. `CorsLayer` (explicit restricted origin preflight handling)
/// 6. `TimeoutLayer` (enforces request deadline budget)
/// 7. `CompressionLayer` (gzip response compression)
pub fn apply_http_edge_stack(
    router: axum::Router,
    is_production: bool,
    allowed_origins: Option<&[String]>,
    timeout_seconds: u64,
) -> axum::Router {
    let x_request_id = HeaderName::from_static("x-request-id");
    let cors_layer = build_cors_layer(is_production, allowed_origins);
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(
            DefaultMakeSpan::new()
                .level(tracing::Level::INFO)
                .include_headers(false),
        )
        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO))
        .on_failure(DefaultOnFailure::new().level(tracing::Level::ERROR));
    let sensitive_headers_layer =
        SetSensitiveRequestHeadersLayer::new([header::AUTHORIZATION, header::COOKIE]);
    let timeout_layer = TimeoutLayer::with_status_code(
        Duration::from_secs(timeout_seconds),
        axum::http::StatusCode::REQUEST_TIMEOUT,
    );
    let compression_layer = CompressionLayer::new();

    let service_stack = ServiceBuilder::new()
        .layer(SetRequestIdLayer::new(x_request_id.clone(), MakeRequestUuid))
        .layer(PropagateRequestIdLayer::new(x_request_id))
        .layer(trace_layer)
        .layer(sensitive_headers_layer)
        .layer(from_fn(record_http_metrics))
        .layer(cors_layer)
        .layer(timeout_layer)
        .layer(compression_layer);

    router.layer(service_stack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;

    #[test]
    fn path_sanitization_masks_dynamic_identifiers() {
        assert_eq!(
            sanitize_metrics_path("/api/users/123/orders"),
            "/api/users/:id/orders"
        );
        assert_eq!(
            sanitize_metrics_path("/api/tenants/550e8400-e29b-41d4-a716-446655440000/settings"),
            "/api/tenants/:id/settings"
        );
        assert_eq!(
            sanitize_metrics_path("/api/products/c3ab8ff13720e8ad9047dd39466b3c89"),
            "/api/products/:id"
        );
        assert_eq!(sanitize_metrics_path("/health"), "/health");
    }

    #[tokio::test]
    async fn http_edge_stack_attaches_request_id_and_cors_headers() {
        let app = Router::new().route("/ping", get(|| async { "pong" }));
        let app = apply_http_edge_stack(app, false, None, 10);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ping")
                    .header("origin", "http://localhost:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("x-request-id"));
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .and_then(|v| v.to_str().ok()),
            Some("http://localhost:3000")
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body, "pong");
    }

    #[tokio::test]
    async fn http_edge_stack_handles_cors_preflight_options() {
        let app = Router::new().route("/items", get(|| async { "items" }));
        let app = apply_http_edge_stack(app, false, None, 10);

        let response = app
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/items")
                    .header("origin", "http://localhost:3000")
                    .header("access-control-request-method", "GET")
                    .header("access-control-request-headers", "authorization")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .and_then(|v| v.to_str().ok()),
            Some("http://localhost:3000")
        );
        assert!(response.headers().contains_key("access-control-allow-methods"));
    }
}
