use std::time::{Duration, Instant};

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderName, StatusCode, header};
use axum::middleware::{Next, from_fn};
use axum::response::{IntoResponse, Response};
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::trace::{DefaultOnFailure, DefaultOnResponse, MakeSpan, TraceLayer};

use super::cors::build_cors_layer;

pub const DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS: u64 = 30;
pub const DEFAULT_HTTP_UPLOAD_TIMEOUT_SECONDS: u64 = 300;

pub fn resolve_http_timeout_seconds() -> u64 {
    std::env::var("RUSTOK_HTTP_REQUEST_TIMEOUT_SECONDS")
        .ok()
        .and_then(|val| val.parse::<u64>().ok())
        .filter(|&secs| secs > 0)
        .unwrap_or(DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS)
}

pub fn resolve_upload_timeout_seconds() -> u64 {
    std::env::var("RUSTOK_HTTP_UPLOAD_TIMEOUT_SECONDS")
        .ok()
        .and_then(|val| val.parse::<u64>().ok())
        .filter(|&secs| secs > 0)
        .unwrap_or(DEFAULT_HTTP_UPLOAD_TIMEOUT_SECONDS)
}

/// Custom [`MakeSpan`] that attaches the request's `x-request-id` directly to the
/// root HTTP tracing span for complete end-to-end trace correlation.
#[derive(Clone, Debug)]
pub struct RusTokMakeSpan;

impl<B> MakeSpan<B> for RusTokMakeSpan {
    fn make_span(&mut self, request: &axum::http::Request<B>) -> tracing::Span {
        let request_id = request
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-");
        tracing::info_span!(
            "http_request",
            method = %request.method(),
            uri = %request.uri().path(),
            version = ?request.version(),
            request_id = %request_id,
        )
    }
}

/// Adaptive request timeout middleware.
///
/// Regular requests are budgeted with `resolve_http_timeout_seconds()` (default 30s).
/// Large multipart / artifact upload endpoints receive an extended budget
/// `resolve_upload_timeout_seconds()` (default 300s / 5m) to avoid terminating
/// legitimate slow uploads on slow client networks.
pub async fn adaptive_timeout(request: Request, next: Next) -> Response {
    let is_upload = is_upload_request(&request);
    let timeout_seconds = if is_upload {
        resolve_upload_timeout_seconds().max(resolve_http_timeout_seconds())
    } else {
        resolve_http_timeout_seconds()
    };

    match tokio::time::timeout(Duration::from_secs(timeout_seconds), next.run(request)).await {
        Ok(response) => response,
        Err(_) => StatusCode::REQUEST_TIMEOUT.into_response(),
    }
}

fn is_upload_request(request: &Request) -> bool {
    let path = request.uri().path();
    if path.contains("/artifacts") || path.contains("/upload") || path.contains("/media") {
        return true;
    }
    request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("multipart/form-data") || ct.starts_with("application/octet-stream"))
        .unwrap_or(false)
}

/// Axum middleware to record live Prometheus HTTP metrics into `rustok_telemetry`.
///
/// Bounded path normalization protects Prometheus against metric cardinality explosion
/// from dynamic IDs/UUIDs and resource slugs in URL paths.
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

/// Sanitizes URL path segments replacing dynamic UUIDs, hashes, numeric IDs,
/// and collection slugs with `:id` or `:slug` to prevent metric cardinality explosion.
pub fn sanitize_metrics_path(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').collect();
    let mut sanitized = Vec::with_capacity(segments.len());
    let mut prev_is_collection = false;

    for seg in segments {
        if seg.is_empty() {
            sanitized.push(String::new());
            continue;
        }
        if is_identifier_like(seg) {
            sanitized.push(":id".to_string());
            prev_is_collection = false;
        } else if prev_is_collection && !is_known_action(seg) {
            sanitized.push(":slug".to_string());
            prev_is_collection = false;
        } else {
            prev_is_collection = is_collection_name(seg);
            sanitized.push(seg.to_string());
        }
    }
    sanitized.join("/")
}

fn is_collection_name(s: &str) -> bool {
    matches!(
        s,
        "products"
            | "categories"
            | "collections"
            | "blog"
            | "posts"
            | "articles"
            | "pages"
            | "topics"
            | "users"
            | "tenants"
            | "channels"
            | "orders"
            | "media"
            | "bundles"
            | "brands"
            | "tags"
            | "artifacts"
    )
}

fn is_known_action(s: &str) -> bool {
    matches!(
        s,
        "new"
            | "create"
            | "edit"
            | "delete"
            | "status"
            | "download"
            | "upload"
            | "search"
            | "preview"
            | "list"
            | "dlq"
            | "items"
            | "health"
            | "metrics"
            | "schema"
            | "auth"
            | "login"
            | "register"
            | "oauth"
            | "public"
            | "images"
    )
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
/// 2. `TraceLayer` (per-request spans with latency, status, and embedded `x-request-id`)
/// 3. `SetSensitiveRequestHeadersLayer` (masks `Authorization` and `Cookie`)
/// 4. `record_http_metrics` (populates `HTTP_REQUESTS_TOTAL` and `HTTP_REQUEST_DURATION_SECONDS`)
/// 5. `CorsLayer` (explicit restricted origin preflight handling)
/// 6. `adaptive_timeout` (guards request deadline budget with extended timeout for uploads)
/// 7. `CompressionLayer` (gzip response compression)
pub fn apply_http_edge_stack(
    router: axum::Router,
    is_production: bool,
    allowed_origins: Option<&[String]>,
    _timeout_seconds: u64,
) -> axum::Router {
    let x_request_id = HeaderName::from_static("x-request-id");
    let cors_layer = build_cors_layer(is_production, allowed_origins);
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(RusTokMakeSpan)
        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO))
        .on_failure(DefaultOnFailure::new().level(tracing::Level::ERROR));
    let sensitive_headers_layer =
        SetSensitiveRequestHeadersLayer::new([header::AUTHORIZATION, header::COOKIE]);
    let compression_layer = CompressionLayer::new();

    let service_stack = ServiceBuilder::new()
        .layer(SetRequestIdLayer::new(x_request_id.clone(), MakeRequestUuid))
        .layer(PropagateRequestIdLayer::new(x_request_id))
        .layer(trace_layer)
        .layer(sensitive_headers_layer)
        .layer(from_fn(record_http_metrics))
        .layer(cors_layer)
        .layer(from_fn(adaptive_timeout))
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
    fn path_sanitization_masks_dynamic_identifiers_and_slugs() {
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
        assert_eq!(
            sanitize_metrics_path("/products/nike-air-max-90"),
            "/products/:slug"
        );
        assert_eq!(
            sanitize_metrics_path("/blog/announcing-rustok-v1"),
            "/blog/:slug"
        );
        assert_eq!(
            sanitize_metrics_path("/categories/mens-footwear"),
            "/categories/:slug"
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
