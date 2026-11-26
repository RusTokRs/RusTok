use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{MatchedPath, Request, State};
use axum::http::{HeaderName, HeaderValue, Method, StatusCode, header};
use axum::middleware::{Next, from_fn, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use tower::ServiceBuilder;
use tower::limit::GlobalConcurrencyLimitLayer;
use tower_http::compression::CompressionLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::trace::{DefaultOnFailure, DefaultOnResponse, MakeSpan, TraceLayer};

use super::cors::build_cors_layer;
use crate::services::marketplace_catalog::registry_publish_artifact_path;

pub const DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS: u64 = 30;
pub const DEFAULT_HTTP_UPLOAD_TIMEOUT_SECONDS: u64 = 300;
pub const DEFAULT_HTTP_CONCURRENCY_LIMIT: usize = 512;

// Keep operator-provided values within ranges that are safe for Tokio deadlines
// and Tower's semaphore-backed concurrency limiter.
const MAX_HTTP_REQUEST_TIMEOUT_SECONDS: u64 = 600;
const MAX_HTTP_UPLOAD_TIMEOUT_SECONDS: u64 = 3_600;
const MAX_HTTP_CONCURRENCY_LIMIT: usize = 16_384;

#[derive(Clone, Copy)]
struct HttpTimeouts {
    request: Duration,
    upload: Duration,
}

// Compatibility defaults for callers using adaptive_timeout as a standalone
// middleware. Composed routers carry their own immutable policy instead.
static STANDALONE_HTTP_TIMEOUTS: Mutex<HttpTimeouts> = Mutex::new(HttpTimeouts {
    request: Duration::from_secs(DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS),
    upload: Duration::from_secs(DEFAULT_HTTP_UPLOAD_TIMEOUT_SECONDS),
});

fn bounded_timeout_seconds(
    value: u64,
    default: u64,
    maximum: u64,
    setting: &'static str,
) -> u64 {
    if value == 0 {
        tracing::warn!(setting, default, "HTTP timeout must be positive; using the default");
        return default;
    }
    if value > maximum {
        tracing::warn!(
            setting,
            configured = value,
            maximum,
            "HTTP timeout exceeds the supported maximum; clamping"
        );
        return maximum;
    }
    value
}

fn parse_timeout_setting(
    raw: Option<&str>,
    setting: &'static str,
    default: u64,
    maximum: u64,
) -> u64 {
    let Some(raw) = raw else {
        return default;
    };
    match raw.parse::<u64>() {
        Ok(value) => bounded_timeout_seconds(value, default, maximum, setting),
        Err(_error) => {
            tracing::warn!(
                setting,
                error_code = "http_timeout_setting_invalid",
                "Invalid HTTP timeout setting; using the default"
            );
            default
        }
    }
}

fn parse_http_concurrency_limit(raw: Option<&str>) -> usize {
    let Some(raw) = raw else {
        return DEFAULT_HTTP_CONCURRENCY_LIMIT;
    };
    match raw.parse::<usize>() {
        Ok(0) => {
            tracing::warn!(
                setting = "RUSTOK_HTTP_CONCURRENCY_LIMIT",
                default = DEFAULT_HTTP_CONCURRENCY_LIMIT,
                "HTTP concurrency limit must be positive; using the default"
            );
            DEFAULT_HTTP_CONCURRENCY_LIMIT
        }
        Ok(limit) if limit > MAX_HTTP_CONCURRENCY_LIMIT => {
            tracing::warn!(
                setting = "RUSTOK_HTTP_CONCURRENCY_LIMIT",
                configured = limit,
                maximum = MAX_HTTP_CONCURRENCY_LIMIT,
                "HTTP concurrency limit exceeds the supported maximum; clamping"
            );
            MAX_HTTP_CONCURRENCY_LIMIT
        }
        Ok(limit) => limit,
        Err(error) => {
            tracing::warn!(
                setting = "RUSTOK_HTTP_CONCURRENCY_LIMIT",
                %error,
                "Invalid HTTP concurrency limit; using the default"
            );
            DEFAULT_HTTP_CONCURRENCY_LIMIT
        }
    }
}

pub fn resolve_http_concurrency_limit() -> usize {
    let raw = std::env::var("RUSTOK_HTTP_CONCURRENCY_LIMIT").ok();
    parse_http_concurrency_limit(raw.as_deref())
}

impl HttpTimeouts {
    fn new(request_secs: u64, upload_secs: u64) -> Self {
        let request = bounded_timeout_seconds(
            request_secs,
            DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS,
            MAX_HTTP_REQUEST_TIMEOUT_SECONDS,
            "RUSTOK_HTTP_REQUEST_TIMEOUT_SECONDS",
        );
        let upload = bounded_timeout_seconds(
            upload_secs,
            DEFAULT_HTTP_UPLOAD_TIMEOUT_SECONDS,
            MAX_HTTP_UPLOAD_TIMEOUT_SECONDS,
            "RUSTOK_HTTP_UPLOAD_TIMEOUT_SECONDS",
        );
        Self {
            request: Duration::from_secs(request),
            upload: Duration::from_secs(upload),
        }
    }

    fn for_request<B>(self, request: &axum::http::Request<B>) -> Duration {
        if is_upload_request(request) {
            self.upload.max(self.request)
        } else {
            self.request
        }
    }
}

/// Configures the standalone adaptive_timeout middleware. Already composed
/// HTTP routers retain the timeout policy captured when they were built.
pub fn init_http_timeouts(request_secs: u64, upload_secs: u64) {
    let timeouts = HttpTimeouts::new(request_secs, upload_secs);
    *STANDALONE_HTTP_TIMEOUTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = timeouts;
}

pub fn resolve_http_timeout_seconds() -> u64 {
    let raw = std::env::var("RUSTOK_HTTP_REQUEST_TIMEOUT_SECONDS").ok();
    parse_timeout_setting(
        raw.as_deref(),
        "RUSTOK_HTTP_REQUEST_TIMEOUT_SECONDS",
        DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS,
        MAX_HTTP_REQUEST_TIMEOUT_SECONDS,
    )
}

pub fn resolve_upload_timeout_seconds() -> u64 {
    let raw = std::env::var("RUSTOK_HTTP_UPLOAD_TIMEOUT_SECONDS").ok();
    parse_timeout_setting(
        raw.as_deref(),
        "RUSTOK_HTTP_UPLOAD_TIMEOUT_SECONDS",
        DEFAULT_HTTP_UPLOAD_TIMEOUT_SECONDS,
        MAX_HTTP_UPLOAD_TIMEOUT_SECONDS,
    )
}

/// Fixed log vocabulary for HTTP methods. Extension methods are accepted
/// by the router but may be arbitrarily long and are not safe trace fields.
pub(crate) fn trace_method_family(method: &Method) -> &'static str {
    match method.as_str() {
        "GET" => "GET",
        "POST" => "POST",
        "PUT" => "PUT",
        "PATCH" => "PATCH",
        "DELETE" => "DELETE",
        "HEAD" => "HEAD",
        "OPTIONS" => "OPTIONS",
        "CONNECT" => "CONNECT",
        "TRACE" => "TRACE",
        _ => "OTHER",
    }
}

/// Request/correlation identifiers originate in untrusted HTTP headers.
/// Preserve canonical UUIDs and 32-digit hex trace IDs for cross-service
/// correlation; never render arbitrary strings (including bearer-like or
/// malformed IDs) into the root span. HTTP propagation remains unchanged.
fn trace_safe_request_id(value: Option<&HeaderValue>) -> &str {
    let Some(value) = value else {
        return "-";
    };
    if value.as_bytes().len() > 36 {
        return "-";
    }
    let Ok(value) = value.to_str() else {
        return "-";
    };
    let shape = value.len() == 36
        || (value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit()));
    if shape && uuid::Uuid::parse_str(value).is_ok() {
        value
    } else {
        "-"
    }
}

/// W3C trace-context version 00 has a fixed 55-byte ASCII grammar. Invalid,
/// all-zero or future-version strings stay available to the downstream
/// transport but are not reflected into diagnostic spans.
fn trace_safe_traceparent(value: Option<&HeaderValue>) -> &str {
    let Some(value) = value else {
        return "-";
    };
    let bytes = value.as_bytes();
    if bytes.len() != 55
        || &bytes[0..3] != b"00-"
        || bytes[35] != b'-'
        || bytes[52] != b'-'
        || !bytes[3..35].iter().all(|byte| byte.is_ascii_hexdigit())
        || !bytes[36..52].iter().all(|byte| byte.is_ascii_hexdigit())
        || !bytes[53..55].iter().all(|byte| byte.is_ascii_hexdigit())
        || bytes[3..35].iter().all(|byte| *byte == b'0')
        || bytes[36..52].iter().all(|byte| *byte == b'0')
    {
        return "-";
    }
    value.to_str().unwrap_or("-")
}

/// Custom [`MakeSpan`] that attaches validated, bounded tracing identifiers
/// to the root HTTP span. Untrusted custom header content never becomes a
/// log field, but routing and response-header propagation are unchanged.
#[derive(Clone, Debug)]
pub struct RusTokMakeSpan;

impl<B> MakeSpan<B> for RusTokMakeSpan {
    fn make_span(&mut self, request: &axum::http::Request<B>) -> tracing::Span {
        let request_id = trace_safe_request_id(request.headers().get("x-request-id"));
        let correlation_id = trace_safe_request_id(request.headers().get("x-correlation-id"));
        let traceparent = trace_safe_traceparent(request.headers().get("traceparent"));
        tracing::info_span!(
            "http_request",
            method = trace_method_family(request.method()),
            path_group = trace_path_family(request.uri().path()),
            version = ?request.version(),
            request_id = %request_id,
            correlation_id = %correlation_id,
            traceparent = %traceparent,
        )
    }
}

/// Privacy-bounded path family for pre-routing HTTP traces and middleware
/// diagnostics. Axum's matched route template is not yet available to the
/// outer edge span. Never record raw path segments (they can contain tokens,
/// account identifiers, or arbitrary user input). Full route templates remain
/// available to the existing post-routing metrics collector.
pub(crate) fn trace_path_family(path: &str) -> &'static str {
    match path {
        "/" => "/",
        "/health" => "/health",
        "/health/live" => "/health/live",
        "/health/ready" => "/health/ready",
        "/health/runtime" => "/health/runtime",
        "/health/modules" => "/health/modules",
        "/metrics" | "/metrics/" => "/metrics",
        "/graphql" | "/api/graphql" => "/graphql",
        _ if path == "/api" || path.starts_with("/api/") => "/api/:request",
        _ if path == "/admin" || path.starts_with("/admin/") => "/admin/:request",
        _ => "/:request",
    }
}

/// Standalone adaptive request timeout middleware. The canonical edge stack
/// uses a router-local immutable policy instead of these compatibility defaults.
pub async fn adaptive_timeout(request: Request, next: Next) -> Response {
    let timeouts = *STANDALONE_HTTP_TIMEOUTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    scoped_adaptive_timeout(State(timeouts), request, next).await
}

/// Regular requests use the captured request budget. Owner upload routes
/// receive at least that budget and may use the larger upload budget.
/// No global configuration is read or mutated on this request path.
async fn scoped_adaptive_timeout(
    State(timeouts): State<HttpTimeouts>,
    request: Request,
    next: Next,
) -> Response {
    let timeout = timeouts.for_request(&request);
    enforce_request_timeout(timeout, next.run(request)).await
}

async fn enforce_request_timeout(
    timeout: Duration,
    response: impl std::future::Future<Output = Response>,
) -> Response {
    match tokio::time::timeout(timeout, response).await {
        Ok(response) => response,
        Err(_) => timeout_response(),
    }
}

fn timeout_response() -> Response {
    rustok_web::HttpError::gateway_timeout("timeout", "Request timed out").into_response()
}

pub fn is_upload_request<B>(request: &axum::http::Request<B>) -> bool {
    let method = request.method();
    let path = request.uri().path();
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let multipart = content_type
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("multipart/form-data"));

    // Extended deadlines belong to owner-declared binary upload transports,
    // not to any URL containing `/media` or any client-supplied Content-Type.
    // Media's owner streaming endpoint is `MEDIA_OWNER_STREAMING_UPLOAD_PATH`.
    if method == Method::POST && multipart && matches!(path, "/api/media" | "/api/media/") {
        return true;
    }
    // Registry artifact publication is the host-owned artifact upload route.
    // `/api/artifacts/{installation}/{*path}` is bounded JSON dispatch and must
    // keep the ordinary request budget.
    method == Method::PUT && path_matches_route_template(path, registry_publish_artifact_path())
}

fn path_matches_route_template(path: &str, template: &str) -> bool {
    // Match the literal route grammar; do not collapse empty segments or
    // silently accept an unregistered trailing slash. No per-request allocation.
    let mut actual_segments = path.split('/');
    for expected in template.split('/') {
        let Some(actual) = actual_segments.next() else {
            return false;
        };
        if expected.starts_with('{') && expected.ends_with('}') {
            if actual.is_empty() {
                return false;
            }
        } else if actual != expected {
            return false;
        }
    }
    actual_segments.next().is_none()
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
    } else if (path == "/api" || path.starts_with("/api/")) {
        "/api/:not_found"
    } else if (path == "/admin" || path.starts_with("/admin/")) {
        "/admin/:not_found"
    } else {
        "/:not_found"
    }
}

/// Unmatched non-404 responses (SPA fallbacks, method errors, redirects) must
/// not create a new time series per requested URL.
fn unmatched_metrics_path(path: &str) -> &'static str {
    if (path == "/api" || path.starts_with("/api/")) {
        "/api/:unmatched"
    } else if (path == "/admin" || path.starts_with("/admin/")) {
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
/// 7. `scoped_adaptive_timeout` (router-local request budget with extended timeout for upload mutations)
/// 8. `GlobalConcurrencyLimitLayer` (one shared in-flight budget across routes)
/// 9. `CompressionLayer` (transparent response compression)
pub fn apply_http_edge_stack(
    router: axum::Router,
    is_production: bool,
    allowed_origins: Option<&[String]>,
    timeout_seconds: u64,
) -> axum::Router {
    apply_http_edge_stack_with_concurrency_limit(
        router,
        is_production,
        allowed_origins,
        timeout_seconds,
        resolve_http_concurrency_limit(),
    )
}

fn apply_http_edge_stack_with_concurrency_limit(
    router: axum::Router,
    is_production: bool,
    allowed_origins: Option<&[String]>,
    timeout_seconds: u64,
    concurrency_limit: usize,
) -> axum::Router {
    let timeouts = HttpTimeouts::new(timeout_seconds, resolve_upload_timeout_seconds());

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
        .layer(from_fn_with_state(timeouts, scoped_adaptive_timeout))
        // Router::layer applies the layer to multiple route services.
        // Share one semaphore instead of allocating a separate quota for each.
        .layer(GlobalConcurrencyLimitLayer::new(concurrency_limit))
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

    #[tokio::test]
    async fn request_timeout_uses_the_canonical_gateway_timeout_envelope() {
        let response =
            enforce_request_timeout(Duration::from_millis(1), std::future::pending::<Response>())
                .await;
        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let body = to_bytes(response.into_body(), 4096)
            .await
            .expect("read timeout body");
        let body: rustok_web::ErrorBody =
            serde_json::from_slice(&body).expect("JSON timeout envelope");
        assert_eq!(body.code, "timeout");
        assert_eq!(body.message, "Request timed out");
    }

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
        assert_eq!(
            unmatched_metrics_path("/apiary/private"),
            "/:unmatched"
        );
        assert_eq!(
            unmatched_metrics_path("/administrator/private"),
            "/:unmatched"
        );
        assert_eq!(
            not_found_metrics_path("/apiary/private"),
            "/:not_found"
        );
        assert_eq!(
            not_found_metrics_path("/administrator/private"),
            "/:not_found"
        );
        assert_eq!(unmatched_metrics_path("/api"), "/api/:unmatched");
        assert_eq!(unmatched_metrics_path("/admin"), "/admin/:unmatched");
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
            .route("/known", get(|| async { "known" }))
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

        let malformed_multipart_req = Request::builder()
            .method("POST")
            .uri("/api/media")
            .header("content-type", "multipart/form-data-evil; boundary=abc")
            .body(())
            .unwrap();
        assert!(
            !is_upload_request(&malformed_multipart_req),
            "a malformed media type must not receive the extended upload deadline"
        );

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

    #[test]
    fn extended_upload_budget_does_not_normalize_unregistered_paths() {
        for path in ["/api/media", "/api/media/"] {
            let request = Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "multipart/form-data; boundary=abc")
                .body(())
                .unwrap();
            assert!(is_upload_request(&request), "{path}");
        }
        let request = Request::builder()
            .method("POST")
            .uri("/api/media//")
            .header("content-type", "multipart/form-data; boundary=abc")
            .body(())
            .unwrap();
        assert!(!is_upload_request(&request));

        for path in [
            "/v2//catalog/publish/request/artifact",
            "/v2/catalog/publish//artifact",
            "/v2/catalog/publish/request//artifact",
            "/v2/catalog/publish/request/artifact/",
        ] {
            let request = Request::builder()
                .method("PUT")
                .uri(path)
                .body(())
                .unwrap();
            assert!(!is_upload_request(&request), "{path}");
        }
        assert!(path_matches_route_template(
            "/v2/catalog/publish/request/artifact",
            registry_publish_artifact_path(),
        ));
    }

    #[tokio::test]
    async fn http_edge_concurrency_budget_is_shared_across_routes() {
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let router = Router::new()
            .route(
                "/first",
                get({
                    let entered = Arc::clone(&entered);
                    let release = Arc::clone(&release);
                    move || {
                        let entered = Arc::clone(&entered);
                        let release = Arc::clone(&release);
                        async move {
                            entered.notify_one();
                            release.notified().await;
                            "first"
                        }
                    }
                }),
            )
            .route("/second", get(|| async { "second" }));
        let app = apply_http_edge_stack_with_concurrency_limit(router, false, None, 10, 1);
        let first = tokio::spawn(app.clone().oneshot(
            Request::builder().uri("/first").body(Body::empty()).unwrap(),
        ));
        tokio::time::timeout(Duration::from_secs(1), entered.notified())
            .await
            .expect("first handler must hold the only permit");
        let second = app.oneshot(
            Request::builder().uri("/second").body(Body::empty()).unwrap(),
        );
        tokio::pin!(second);
        let early = tokio::time::timeout(Duration::from_millis(50), &mut second).await;
        // Release the first request even if the assertion below fails.
        release.notify_one();
        assert!(early.is_err(), "a distinct route bypassed the shared permit");

        assert_eq!(
            first.await.unwrap().unwrap().status(),
            StatusCode::OK,
        );
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), second)
                .await
                .expect("second request must acquire the released permit")
                .unwrap()
                .status(),
            StatusCode::OK,
        );
    }

    #[tokio::test]
    async fn composed_routers_keep_independent_request_budgets() {
        let release_long = Arc::new(tokio::sync::Notify::new());
        let short = apply_http_edge_stack(
            Router::new().route("/wait", get(|| async {
                std::future::pending::<&'static str>().await
            })),
            false,
            None,
            1,
        );
        let long = apply_http_edge_stack(
            Router::new().route("/wait", get({
                let release_long = Arc::clone(&release_long);
                move || {
                    let release_long = Arc::clone(&release_long);
                    async move {
                        release_long.notified().await;
                        "long"
                    }
                }
            })),
            false,
            None,
            10,
        );

        let short_request = short.oneshot(
            Request::builder().uri("/wait").body(Body::empty()).unwrap(),
        );
        let long_request = long.oneshot(
            Request::builder().uri("/wait").body(Body::empty()).unwrap(),
        );

        let (short_response, long_response) = tokio::time::timeout(
            Duration::from_secs(3),
            async {
                tokio::join!(
                    async {
                        let response = short_request.await.unwrap();
                        release_long.notify_one();
                        response
                    },
                    long_request,
                )
            },
        )
        .await
        .expect("building the second router must not extend the first deadline");
        assert_eq!(short_response.status(), StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(long_response.unwrap().status(), StatusCode::OK);
    }

    #[test]
    fn upload_timeout_never_shortens_the_regular_request_budget() {
        let upload = Request::builder()
            .method("PUT")
            .uri("/v2/catalog/publish/request/artifact")
            .body(())
            .unwrap();
        let regular = Request::builder().uri("/health").body(()).unwrap();
        let timeouts = HttpTimeouts::new(30, 5);
        assert_eq!(timeouts.for_request(&upload), Duration::from_secs(30));
        assert_eq!(timeouts.for_request(&regular), Duration::from_secs(30));
        let timeouts = HttpTimeouts::new(30, 90);
        assert_eq!(timeouts.for_request(&upload), Duration::from_secs(90));
        assert_eq!(timeouts.for_request(&regular), Duration::from_secs(30));
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

    #[test]
    fn trace_fields_reject_unbounded_or_malformed_client_identifiers() {
        use axum::http::HeaderValue;

        let generated = HeaderValue::from_static("550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(
            trace_safe_request_id(Some(&generated)),
            "550e8400-e29b-41d4-a716-446655440000"
        );
        let trace_id = HeaderValue::from_static("463ac35c9f6413ad48485a3953bb6124");
        assert_eq!(
            trace_safe_request_id(Some(&trace_id)),
            "463ac35c9f6413ad48485a3953bb6124"
        );
        let overlong = "a".repeat(4096);
        for invalid in [
            "Bearer secret",
            "hello\\nsecret",
            "secret",
            "https://user:password@example.org",
            overlong.as_str(),
        ] {
            let header = HeaderValue::from_str(invalid).expect("valid HTTP header bytes");
            assert_eq!(
                trace_safe_request_id(Some(&header)),
                "-",
                "untrusted request/correlation header values must not reach logs"
            );
        }
        assert_eq!(trace_safe_request_id(None), "-");

        let valid_parent = HeaderValue::from_static(
            "00-463ac35c9f6413ad48485a3953bb6124-0020000000000001-01"
        );
        assert_eq!(
            trace_safe_traceparent(Some(&valid_parent)),
            valid_parent.to_str().unwrap()
        );
        for invalid in [
            "00-00000000000000000000000000000000-0020000000000001-01",
            "00-463ac35c9f6413ad48485a3953bb6124-0000000000000000-01",
            "ff-463ac35c9f6413ad48485a3953bb6124-0020000000000001-01",
            "00-secret-value",
        ] {
            let header = HeaderValue::from_str(invalid).unwrap();
            assert_eq!(trace_safe_traceparent(Some(&header)), "-");
        }
        assert_eq!(trace_safe_traceparent(None), "-");

        assert_eq!(trace_method_family(&Method::GET), "GET");
        assert_eq!(
            trace_method_family(&Method::from_bytes(b"UNTRUSTED-CUSTOM-METHOD").unwrap()),
            "OTHER"
        );
    }

    #[test]
    fn traces_use_bounded_path_families_without_untrusted_segments() {
        assert_eq!(trace_path_family("/health/ready"), "/health/ready");
        assert_eq!(trace_path_family("/metrics/"), "/metrics");
        assert_eq!(trace_path_family("/api/graphql"), "/graphql");
        assert_eq!(
            trace_path_family("/api/auth/password/reset/secret-token-in-path"),
            "/api/:request"
        );
        assert_eq!(
            trace_path_family("/admin/tenant/private-user-id"),
            "/admin/:request"
        );
        assert_eq!(
            trace_path_family("/apiary/private"),
            "/:request",
            "a similarly prefixed path must not acquire the API path family"
        );
        assert_eq!(
            trace_path_family("/hidden/secrets/aaa?password=not-logged"),
            "/:request"
        );
        for path in [
            "/api/auth/password/reset/secret-token-in-path",
            "/admin/tenant/private-user-id",
            "/hidden/secrets/aaa?password=not-logged",
        ] {
            assert!(!trace_path_family(path).contains("secret"));
            assert!(!trace_path_family(path).contains("private-user"));
            assert!(!trace_path_family(path).contains("password"));
        }
    }

    #[test]
    fn http_timeout_settings_are_bounded_before_becoming_tokio_deadlines() {
        assert_eq!(
            bounded_timeout_seconds(
                0,
                DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS,
                MAX_HTTP_REQUEST_TIMEOUT_SECONDS,
                "request",
            ),
            DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS
        );
        assert_eq!(
            bounded_timeout_seconds(
                u64::MAX,
                DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS,
                MAX_HTTP_REQUEST_TIMEOUT_SECONDS,
                "request",
            ),
            MAX_HTTP_REQUEST_TIMEOUT_SECONDS
        );
        assert_eq!(
            parse_timeout_setting(
                Some("999999"),
                "request",
                DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS,
                MAX_HTTP_REQUEST_TIMEOUT_SECONDS,
            ),
            MAX_HTTP_REQUEST_TIMEOUT_SECONDS
        );
        assert_eq!(
            parse_timeout_setting(
                Some("not-a-number"),
                "request",
                DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS,
                MAX_HTTP_REQUEST_TIMEOUT_SECONDS,
            ),
            DEFAULT_HTTP_REQUEST_TIMEOUT_SECONDS
        );
    }

    #[test]
    fn http_concurrency_limit_is_bounded_before_constructing_tower_semaphore() {
        assert_eq!(parse_http_concurrency_limit(Some("1")), 1);
        assert_eq!(
            parse_http_concurrency_limit(Some("16385")),
            MAX_HTTP_CONCURRENCY_LIMIT
        );
        assert_eq!(
            parse_http_concurrency_limit(Some("0")),
            DEFAULT_HTTP_CONCURRENCY_LIMIT
        );
        assert_eq!(
            parse_http_concurrency_limit(Some("invalid")),
            DEFAULT_HTTP_CONCURRENCY_LIMIT
        );
    }
}
