use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderName, Method, StatusCode, header};
use axum::middleware::{Next, from_fn};
use axum::response::{IntoResponse, Response};
use tower::ServiceBuilder;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::compression::CompressionLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::trace::{DefaultOnFailure, DefaultOnResponse, MakeSpan, TraceLayer};

use super::cors::build_cors_layer;
use crate::services::marketplace_catalog::registry_publish_artifact_path;

pub const DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS: u64 = 30;
pub const DEFAULT_HTTP_UPLOAD_TIMEOUT_SECONDS: u64 = 300;
pub const DEFAULT_HTTP_CONCURRENCY_LIMIT: usize = 512;

static HTTP_REQUEST_TIMEOUT_SECS: AtomicU64 = AtomicU64::new(DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS);
static HTTP_UPLOAD_TIMEOUT_SECS: AtomicU64 = AtomicU64::new(DEFAULT_HTTP_UPLOAD_TIMEOUT_SECONDS);

pub fn resolve_http_concurrency_limit() -> usize {
    std::env::var("RUSTOK_HTTP_CONCURRENCY_LIMIT")
        .ok()
        .and_then(|val| val.parse::<usize>().ok())
        .filter(|&limit| limit > 0)
        .unwrap_or(DEFAULT_HTTP_CONCURRENCY_LIMIT)
}

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
    let path = request.uri().path();
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let multipart = content_type
        .map(|value| value.starts_with("multipart/form-data"))
        .unwrap_or(false);

    // Extended deadlines belong to owner-declared binary upload transports,
    // not to any URL containing `/media` or any client-supplied Content-Type.
    // Media's owner streaming endpoint is `MEDIA_OWNER_STREAMING_UPLOAD_PATH`.
    if method == Method::POST && multipart && path.trim_end_matches('/') == "/api/media" {
        return true;
    }
    // Registry artifact publication is the host-owned artifact upload route.
    // `/api/artifacts/{installation}/{*path}` is bounded JSON dispatch and must
    // keep the ordinary request budget.
    method == Method::PUT && path_matches_route_template(path, registry_publish_artifact_path())
}

fn path_matches_route_template(path: &str, template: &str) -> bool {
    let path_segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let template_segments: Vec<&str> = template
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    path_segments.len() == template_segments.len()
        && path_segments
            .iter()
            .zip(template_segments.iter())
            .all(|(actual, expected)| {
                if expected.starts_with('{') && expected.ends_with('}') {
                    !actual.is_empty()
                } else {
                    actual == expected
                }
            })
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

#[derive(Clone, Default)]
struct MatchedRouteSlot(Arc<Mutex<Option<String>>>);

impl MatchedRouteSlot {
    fn set(&self, path: &str) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(path.to_string());
        }
    }

    fn take(&self) -> Option<String> {
        self.0.lock().ok().and_then(|mut slot| slot.take())
    }
}

/// Runs as a route layer, i.e. after Axum selected the route and attached
/// `MatchedPath`. The outer metrics middleware wraps the router before routing,
/// so it cannot observe that extension directly.
async fn capture_matched_path(request: Request, next: Next) -> Response {
    if let (Some(slot), Some(matched)) = (
        request.extensions().get::<MatchedRouteSlot>(),
        request.extensions().get::<MatchedPath>(),
    ) {
        slot.set(matched.as_str());
    }
    next.run(request).await
}

/// Records HTTP metrics with a bounded path-label vocabulary:
/// routed requests use the router's path template; unmatched requests use a
/// fixed bucket regardless of status. Raw URL segments never become labels.
pub async fn record_http_metrics(mut request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = method_str(request.method());
    let raw_path = request.uri().path().to_string();
    let slot = MatchedRouteSlot::default();
    request.extensions_mut().insert(slot.clone());

    let response = next.run(request).await;
    let latency = start.elapsed().as_secs_f64();
    let status = response.status();
    let status_str = status_code_str(status);
    let effective_path = match slot.take() {
        Some(matched) => matched,
        None if status == StatusCode::NOT_FOUND => not_found_metrics_path(&raw_path).to_string(),
        None => unmatched_metrics_path(&raw_path).to_string(),
    };

    rustok_telemetry::HTTP_REQUESTS_TOTAL
        .with_label_values(&[method, effective_path.as_str(), status_str])
        .inc();
    rustok_telemetry::HTTP_REQUEST_DURATION_SECONDS
        .with_label_values(&[method, effective_path.as_str()])
        .observe(latency);

    response
}

fn not_found_metrics_path(path: &str) -> &'static str {
    if path == "/health" || path == "/metrics" || path == "/" {
        if path == "/health" {
            "/health"
        } else if path == "/metrics" {
            "/metrics"
        } else {
            "/"
        }
    } else if path.starts_with("/api") {
        "/api/:not_found"
    } else if path.starts_with("/admin") {
        "/admin/:not_found"
    } else {
        "/:not_found"
    }
}

/// Unmatched non-404 responses (SPA fallbacks, method errors, redirects) must
/// not create a new time series per requested URL.
fn unmatched_metrics_path(path: &str) -> &'static str {
    if path.starts_with("/api") {
        "/api/:unmatched"
    } else if path.starts_with("/admin") {
        "/admin/:unmatched"
    } else {
        "/:unmatched"
    }
}

/// Applies the canonical production HTTP middleware stack:
///
/// 1. `SetRequestIdLayer` (`x-request-id` generation and request extension attachment)
/// 2. `PropagateRequestIdLayer` (`x-request-id` header copied to outgoing response)
/// 3. `SetSensitiveRequestHeadersLayer` (masks `Authorization`, `Cookie`, `Set-Cookie`, `x-rustok-runner-token`,
///    `x-rustok-host-token`, and `x-rustok-setup-token` BEFORE tracing so credentials never leak to logs or spans)
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
        header::PROXY_AUTHORIZATION,
        HeaderName::from_static("x-rustok-runner-token"),
        HeaderName::from_static("x-rustok-host-token"),
        HeaderName::from_static("x-rustok-setup-token"),
        HeaderName::from_static("x-rustok-observability-token"),
        HeaderName::from_static("x-api-key"),
    ]);
    let compression_layer = CompressionLayer::new();

    let concurrency_limit = resolve_http_concurrency_limit();
    let service_stack = ServiceBuilder::new()
        .layer(SetRequestIdLayer::new(
            x_request_id.clone(),
            MakeRequestUuid,
        ))
        .layer(PropagateRequestIdLayer::new(x_request_id))
        .layer(sensitive_headers_layer)
        .layer(trace_layer)
        .layer(from_fn(record_http_metrics))
        .layer(cors_layer)
        .layer(from_fn(adaptive_timeout))
        .layer(ConcurrencyLimitLayer::new(concurrency_limit))
        .layer(compression_layer);

    // Route layers run after routing and can capture `MatchedPath` for the
    // outer metrics middleware. Fallbacks intentionally leave the slot empty.
    router
        .route_layer(from_fn(capture_matched_path))
        .layer(service_stack)
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
    fn unmatched_paths_use_a_bounded_label_vocabulary() {
        assert_eq!(
            unmatched_metrics_path("/api/products/550e8400-e29b-41d4-a716-446655440000"),
            "/api/:unmatched"
        );
        assert_eq!(
            unmatched_metrics_path("/admin/modules/blog/settings"),
            "/admin/:unmatched"
        );
        assert_eq!(
            unmatched_metrics_path("/random/attacker-path"),
            "/:unmatched"
        );
        assert_eq!(
            not_found_metrics_path("/api/unknown/123"),
            "/api/:not_found"
        );
        assert_eq!(not_found_metrics_path("/health"), "/health");
        assert_eq!(not_found_metrics_path("/metrics"), "/metrics");
        assert_eq!(not_found_metrics_path("/"), "/");
    }

    #[tokio::test]
    async fn matched_route_slot_receives_only_routed_templates() {
        let router = Router::new()
            .route("/items/{id}", get(|| async { "ok" }))
            .route_layer(from_fn(capture_matched_path));
        let slot = MatchedRouteSlot::default();
        let mut request = Request::builder()
            .uri("/items/secret-customer-id")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(slot.clone());
        let response = router.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(slot.take().as_deref(), Some("/items/{id}"));

        let fallback = Router::new()
            .fallback(|| async { "spa" })
            .route_layer(from_fn(capture_matched_path));
        let slot = MatchedRouteSlot::default();
        let mut request = Request::builder()
            .uri("/anything/attacker-controlled")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(slot.clone());
        let response = fallback.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(slot.take(), None);
    }

    #[test]
    fn upload_request_requires_an_owner_upload_route() {
        let get_req = Request::builder()
            .method("GET")
            .uri("/api/media/avatar.png")
            .body(())
            .unwrap();
        assert!(!is_upload_request(&get_req));

        let post_req = Request::builder()
            .method("POST")
            .uri("/api/media/")
            .header("content-type", "multipart/form-data; boundary=abc")
            .body(())
            .unwrap();
        assert!(is_upload_request(&post_req));

        let put_req = Request::builder()
            .method("PUT")
            .uri("/v2/catalog/publish/550e8400-e29b-41d4-a716-446655440000/artifact")
            .header("content-type", "application/octet-stream")
            .body(())
            .unwrap();
        assert!(is_upload_request(&put_req));

        // Artifact HTTP dispatch is bounded JSON, not an upload transport.
        let dispatch_req = Request::builder()
            .method("POST")
            .uri("/api/artifacts/installation/dispatch")
            .header("content-type", "application/octet-stream")
            .body(())
            .unwrap();
        assert!(!is_upload_request(&dispatch_req));

        let login_req = Request::builder()
            .method("POST")
            .uri("/api/auth/login")
            .header("content-type", "multipart/form-data; boundary=abc")
            .body(())
            .unwrap();
        assert!(!is_upload_request(&login_req));
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
        assert!(
            response
                .headers()
                .contains_key("access-control-allow-methods")
        );
    }

    #[tokio::test]
    async fn http_edge_stack_marks_custom_tokens_as_sensitive_headers() {
        let app = Router::new().route(
            "/sensitive",
            get(|request: Request<Body>| async move {
                let headers = request.headers();
                let runner = headers
                    .get("x-rustok-runner-token")
                    .map(|v| v.is_sensitive())
                    .unwrap_or(false);
                let host = headers
                    .get("x-rustok-host-token")
                    .map(|v| v.is_sensitive())
                    .unwrap_or(false);
                let setup = headers
                    .get("x-rustok-setup-token")
                    .map(|v| v.is_sensitive())
                    .unwrap_or(false);
                let auth = headers
                    .get("authorization")
                    .map(|v| v.is_sensitive())
                    .unwrap_or(false);
                let obs = headers
                    .get("x-rustok-observability-token")
                    .map(|v| v.is_sensitive())
                    .unwrap_or(false);
                let api_key = headers
                    .get("x-api-key")
                    .map(|v| v.is_sensitive())
                    .unwrap_or(false);
                let proxy_auth = headers
                    .get("proxy-authorization")
                    .map(|v| v.is_sensitive())
                    .unwrap_or(false);
                if runner && host && setup && auth && obs && api_key && proxy_auth {
                    StatusCode::OK
                } else {
                    StatusCode::BAD_REQUEST
                }
            }),
        );
        let app = apply_http_edge_stack(app, false, None, 10);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/sensitive")
                    .header("authorization", "Bearer test-secret")
                    .header("proxy-authorization", "Basic test-proxy")
                    .header("x-rustok-runner-token", "runner-secret")
                    .header("x-rustok-host-token", "host-secret")
                    .header("x-rustok-setup-token", "setup-secret")
                    .header("x-rustok-observability-token", "obs-secret")
                    .header("x-api-key", "api-secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
