use axum::{
    body::{Body, to_bytes},
    http::{HeaderValue, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

const OBSERVABILITY_TOKEN_ENV: &str = "RUSTOK_OBSERVABILITY_BEARER_TOKEN";
const LEGACY_METRICS_TOKEN_ENV: &str = "RUSTOK_METRICS_BEARER_TOKEN";
const MAX_READINESS_BODY_BYTES: usize = 256 * 1024;

/// Protect Prometheus and detailed operational diagnostics without coupling
/// them to tenant resolution.
///
/// Basic liveness (`/health`, `/health/live`) remains public. Public readiness
/// exposes only the aggregate status and translates `unhealthy` into HTTP 503.
/// A valid observability bearer token receives the full dependency diagnostic
/// body. Production is fail-closed for metrics/runtime/module diagnostics when
/// no host token is configured.
pub async fn require_bearer(request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path();
    if path == "/health/ready" {
        let reveal_details = request_is_authorized(&request);
        let response = next.run(request).await;
        return normalize_readiness_response(response, reveal_details).await;
    }

    if !is_protected_observability_path(path) {
        return next.run(request).await;
    }

    let Some(expected) = configured_token() else {
        if cfg!(debug_assertions) {
            return next.run(request).await;
        }
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "observability authentication is not configured",
        )
            .into_response();
    };

    if supplied_token(&request).is_some_and(|supplied| constant_time_eq(supplied, &expected)) {
        return next.run(request).await;
    }

    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer realm=\"observability\"")],
        "observability bearer token required",
    )
        .into_response()
}

fn request_is_authorized(request: &Request<Body>) -> bool {
    match configured_token() {
        Some(expected) => {
            supplied_token(request).is_some_and(|supplied| constant_time_eq(supplied, &expected))
        }
        None => cfg!(debug_assertions),
    }
}

fn supplied_token(request: &Request<Body>) -> Option<&str> {
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_bearer_token)
}

async fn normalize_readiness_response(response: Response, reveal_details: bool) -> Response {
    let (mut parts, body) = response.into_parts();
    let bytes = match to_bytes(body, MAX_READINESS_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => return generic_readiness_response("unhealthy"),
    };
    let value = match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(value) => value,
        Err(_) => return generic_readiness_response("unhealthy"),
    };
    let status = value
        .get("status")
        .and_then(serde_json::Value::as_str)
        .filter(|status| matches!(*status, "ok" | "degraded" | "unhealthy"))
        .unwrap_or("unhealthy");
    parts.status = readiness_http_status(status);

    if reveal_details {
        return Response::from_parts(parts, Body::from(bytes));
    }

    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    let sanitized = serde_json::to_vec(&serde_json::json!({ "status": status }))
        .unwrap_or_else(|_| br#"{"status":"unhealthy"}"#.to_vec());
    Response::from_parts(parts, Body::from(sanitized))
}

fn generic_readiness_response(status: &str) -> Response {
    let status_code = readiness_http_status(status);
    (
        status_code,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        serde_json::json!({ "status": status }).to_string(),
    )
        .into_response()
}

fn readiness_http_status(status: &str) -> StatusCode {
    if status == "unhealthy" {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    }
}

fn configured_token() -> Option<String> {
    [OBSERVABILITY_TOKEN_ENV, LEGACY_METRICS_TOKEN_ENV]
        .into_iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn is_protected_observability_path(path: &str) -> bool {
    matches!(
        path,
        "/metrics" | "/metrics/" | "/api/_health/metrics" | "/health/runtime" | "/health/modules"
    )
}

fn parse_bearer_token(value: &str) -> Option<&str> {
    let (scheme, token) = value.trim().split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    (!token.is_empty()).then_some(token)
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    bool::from(left.as_bytes().ct_eq(right.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::{
        constant_time_eq, is_protected_observability_path, parse_bearer_token,
        readiness_http_status,
    };
    use axum::http::StatusCode;

    #[test]
    fn protects_metrics_and_detailed_health_only() {
        for path in [
            "/metrics",
            "/metrics/",
            "/api/_health/metrics",
            "/health/runtime",
            "/health/modules",
        ] {
            assert!(is_protected_observability_path(path), "{path}");
        }
        assert!(!is_protected_observability_path("/health"));
        assert!(!is_protected_observability_path("/health/live"));
        assert!(!is_protected_observability_path("/health/ready"));
        assert!(!is_protected_observability_path("/api/graphql"));
    }

    #[test]
    fn readiness_status_controls_http_availability() {
        assert_eq!(readiness_http_status("ok"), StatusCode::OK);
        assert_eq!(readiness_http_status("degraded"), StatusCode::OK);
        assert_eq!(
            readiness_http_status("unhealthy"),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            readiness_http_status("invalid"),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn accepts_only_non_empty_bearer_credentials() {
        assert_eq!(parse_bearer_token("Bearer secret"), Some("secret"));
        assert_eq!(parse_bearer_token("bearer secret"), Some("secret"));
        assert_eq!(parse_bearer_token("Basic secret"), None);
        assert_eq!(parse_bearer_token("Bearer   "), None);
    }

    #[test]
    fn token_comparison_is_exact() {
        assert!(constant_time_eq("metrics-secret", "metrics-secret"));
        assert!(!constant_time_eq("metrics-secret", "metrics-secret-2"));
    }
}
