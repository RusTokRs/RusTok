use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderName, Method, StatusCode, header};
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

static HTTP_REQUEST_TIMEOUT_SECS: AtomicU64 = AtomicU64::new(DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS);
static HTTP_UPLOAD_TIMEOUT_SECS: AtomicU64 = AtomicU64::new(DEFAULT_HTTP_UPLOAD_TIMEOUT_SECONDS);

/// Configures global lock-free timeout thresholds for HTTP request budgets.
pub fn init_http_timeouts(request_secs: u64, upload_secs: u64) {
    let req = if request_secs > 0 {
        request_secs
    } else {
        DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS
    };
    let upl = if upload_secs > 0 {
        upload_secs
    } else {
        DEFAULT_HTTP_UPLOAD_TIMEOUT_SECONDS
    };
    HTTP_REQUEST_TIMEOUT_SECS.store(req, Ordering::Relaxed);
    HTTP_UPLOAD_TIMEOUT_SECS.store(upl, Ordering::Relaxed);
}

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

/// Custom [`MakeSpan`] that attaches the request's `x-request-id` and `x-correlation-id`
/// directly to the root HTTP tracing span for complete end-to-end trace correlation.
#[derive(Clone, Debug)]
pub struct RusTokMakeSpan;

impl<B> MakeSpan<B> for RusTokMakeSpan {
    fn make_span(&mut self, request: &axum::http::Request<B>) -> tracing::Span {
        let request_id = request
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-");
        let correlation_id = request
            .headers()
            .get("x-correlation-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-");
        let traceparent = request
            .headers()
            .get("traceparent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-");
        tracing::info_span!(
            "http_request",
            method = %request.method(),
            uri = %request.uri().path(),
            version = ?request.version(),
            request_id = %request_id,
            correlation_id = %correlation_id,
            traceparent = %traceparent,
        )
    }
}

/// Adaptive request timeout middleware.
///
/// Regular requests are budgeted with `HTTP_REQUEST_TIMEOUT_SECS` (default 30s).
/// Large multipart / artifact upload endpoints receive an extended budget
/// `HTTP_UPLOAD_TIMEOUT_SECS` (default 300s / 5m) to avoid terminating
/// legitimate slow uploads on slow client networks.
///
/// Both limits are read lock-free from atomics with zero heap allocations and
/// zero system calls per request.
pub async fn adaptive_timeout(request: Request, next: Next) -> Response {
    let is_upload = is_upload_request(&request);
    let request_timeout = Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS.load(Ordering::Relaxed));
    let upload_timeout = Duration::from_secs(HTTP_UPLOAD_TIMEOUT_SECS.load(Ordering::Relaxed));
    let timeout = if is_upload {
        upload_timeout.max(request_timeout)
    } else {
        request_timeout
    };

    match tokio::time::timeout(timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => (StatusCode::REQUEST_TIMEOUT, "Request timed out").into_response(),
    }
}

pub fn is_upload_request<B>(request: &axum::http::Request<B>) -> bool {
    let method = request.method();
    // Only mutation methods can upload payloads; GET/HEAD must never be granted
    // extended upload deadlines (prevents slowloris attacks on static media downloads).
    if !matches!(*method, Method::POST | Method::PUT | Method::PATCH) {
        return false;
    }

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

fn method_str(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::DELETE => "DELETE",
        Method::PATCH => "PATCH",
        Method::OPTIONS => "OPTIONS",
        Method::HEAD => "HEAD",
        _ => "OTHER",
    }
}

fn status_code_str(status: StatusCode) -> &'static str {
    match status.as_u16() {
        200 => "200",
        201 => "201",
        202 => "202",
        204 => "204",
        301 => "301",
        302 => "302",
        304 => "304",
        307 => "307",
        308 => "308",
        400 => "400",
        401 => "401",
        403 => "403",
        404 => "404",
        405 => "405",
        408 => "408",
        409 => "409",
        410 => "410",
        413 => "413",
        415 => "415",
        422 => "422",
        429 => "429",
        500 => "500",
        501 => "501",
        502 => "502",
        503 => "503",
        504 => "504",
        _ => match status.as_u16() / 100 {
            1 => "1xx",
            2 => "2xx",
            3 => "3xx",
            4 => "4xx",
            5 => "5xx",
            _ => "unknown",
        },
    }
}

/// Axum middleware to record live Prometheus HTTP metrics into `rustok_telemetry`.
///
/// Bounded path normalization protects Prometheus against metric cardinality explosion:
/// 1. 404 responses from vulnerability scanners or invalid routes are bucketed into
///    `/:not_found`, `/api/:not_found`, or `/admin/:not_found`.
/// 2. Known health/metric probes (`/health`, `/metrics`, `/`) use static strings.
/// 3. Dynamic IDs/UUIDs and resource slugs in URL paths are sanitized to `:id` or `:slug`.
/// 4. Path traversal depth is strictly capped at 8 segments.
/// 5. Method and status labels are statically referenced with zero heap allocations.
pub async fn record_http_metrics(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = method_str(request.method());
    let uri = request.uri().clone();

    let matched_path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string());

    let response = next.run(request).await;
    let latency = start.elapsed().as_secs_f64();
    let status = response.status();
    let status_str = status_code_str(status);
    let raw_path = uri.path();

    // Cardinality protection: bucket 404 scan probes so bots cannot explode Prometheus memory
    let (sanitized_holder, path_ref): (Option<String>, &str) = if status == StatusCode::NOT_FOUND {
        if raw_path.starts_with("/api") {
            (None, "/api/:not_found")
        } else if raw_path.starts_with("/admin") {
            (None, "/admin/:not_found")
        } else {
            (None, "/:not_found")
        }
    } else if let Some(ref matched) = matched_path {
        (None, matched.as_str())
    } else if raw_path == "/health" || raw_path == "/metrics" || raw_path == "/" {
        (None, raw_path)
    } else {
        let s = sanitize_metrics_path(raw_path);
        (Some(s), "")
    };

    let effective_path = if let Some(ref s) = sanitized_holder {
        s.as_str()
    } else {
        path_ref
    };

    rustok_telemetry::HTTP_REQUESTS_TOTAL
        .with_label_values(&[method, effective_path, status_str])
        .inc();
    rustok_telemetry::HTTP_REQUEST_DURATION_SECONDS
        .with_label_values(&[method, effective_path])
        .observe(latency);

    response
}

/// Sanitizes URL path segments replacing dynamic UUIDs, hashes, numeric IDs,
/// and collection slugs with `:id` or `:slug` to prevent metric cardinality explosion.
/// Trailing slashes and duplicate slashes are normalized. Segment depth is capped at 8.
pub fn sanitize_metrics_path(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return "/".to_string();
    }

    let segments: Vec<&str> = trimmed
        .split('/')
        .filter(|s| !s.is_empty())
        .take(8)
        .collect();

    let mut sanitized = Vec::with_capacity(segments.len() + 1);
    sanitized.push("");
    let mut prev_is_collection = false;

    for seg in segments {
        if is_identifier_like(seg) {
            sanitized.push(":id");
            prev_is_collection = false;
        } else if prev_is_collection && !is_known_action(seg) {
            sanitized.push(":slug");
            prev_is_collection = false;
        } else {
            prev_is_collection = is_collection_name(seg);
            sanitized.push(seg);
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
            | "customers"
            | "reviews"
            | "variants"
            | "locales"
            | "roles"
            | "permissions"
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
/// 1. `SetRequestIdLayer` (`x-request-id` generation and request extension attachment)
/// 2. `PropagateRequestIdLayer` (`x-request-id` header copied to outgoing response)
/// 3. `SetSensitiveRequestHeadersLayer` (masks `Authorization`, `Cookie`, `Set-Cookie`, `x-rustok-runner-token`
///    BEFORE tracing so credentials never leak to logs or spans)
/// 4. `TraceLayer` (per-request spans with latency in ms, status, and embedded `x-request-id` + `x-correlation-id`)
/// 5. `record_http_metrics` (populates `HTTP_REQUESTS_TOTAL` and `HTTP_REQUEST_DURATION_SECONDS` with cardinality guards)
/// 6. `CorsLayer` (explicit restricted origin preflight handling, credentials enabled, comprehensive headers)
/// 7. `adaptive_timeout` (guards request deadline budget with extended timeout for upload mutations)
/// 8. `CompressionLayer` (transparent response compression)
pub fn apply_http_edge_stack(
    router: axum::Router,
    is_production: bool,
    allowed_origins: Option<&[String]>,
    timeout_seconds: u64,
) -> axum::Router {
    init_http_timeouts(timeout_seconds, resolve_upload_timeout_seconds());

    let x_request_id = HeaderName::from_static("x-request-id");
    let cors_layer = build_cors_layer(is_production, allowed_origins);
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(RusTokMakeSpan)
        .on_response(
            DefaultOnResponse::new()
                .level(tracing::Level::INFO)
                .latency_unit(tower_http::LatencyUnit::Millis),
        )
        .on_failure(DefaultOnFailure::new().level(tracing::Level::ERROR));
    let sensitive_headers_layer = SetSensitiveRequestHeadersLayer::new([
        header::AUTHORIZATION,
        header::COOKIE,
        header::SET_COOKIE,
        HeaderName::from_static("x-rustok-runner-token"),
    ]);
    let compression_layer = CompressionLayer::new();

    let service_stack = ServiceBuilder::new()
        .layer(SetRequestIdLayer::new(x_request_id.clone(), MakeRequestUuid))
        .layer(PropagateRequestIdLayer::new(x_request_id))
        .layer(sensitive_headers_layer)
        .layer(trace_layer)
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
            sanitize_metrics_path("/api/users/123/orders/"),
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
            sanitize_metrics_path("/blog/announcing-rustok-v1/"),
            "/blog/:slug"
        );
        assert_eq!(
            sanitize_metrics_path("/categories/mens-footwear"),
            "/categories/:slug"
        );
        assert_eq!(sanitize_metrics_path("/health"), "/health");
        assert_eq!(sanitize_metrics_path("/"), "/");
    }

    #[test]
    fn upload_request_requires_mutation_method() {
        let get_req = Request::builder()
            .method("GET")
            .uri("/api/media/avatar.png")
            .body(())
            .unwrap();
        assert!(!is_upload_request(&get_req));

        let post_req = Request::builder()
            .method("POST")
            .uri("/api/media/upload")
            .body(())
            .unwrap();
        assert!(is_upload_request(&post_req));

        let put_req = Request::builder()
            .method("PUT")
            .uri("/api/artifacts/bundle.tar.gz")
            .body(())
            .unwrap();
        assert!(is_upload_request(&put_req));
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
