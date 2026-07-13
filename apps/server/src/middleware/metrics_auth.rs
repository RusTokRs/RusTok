use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

const OBSERVABILITY_TOKEN_ENV: &str = "RUSTOK_OBSERVABILITY_BEARER_TOKEN";
const LEGACY_METRICS_TOKEN_ENV: &str = "RUSTOK_METRICS_BEARER_TOKEN";

/// Protect Prometheus and detailed operational diagnostics without coupling
/// them to tenant resolution.
///
/// Basic liveness (`/health`, `/health/live`) remains public. Production is
/// fail-closed for protected observability paths when no host token is
/// configured. Debug builds retain unauthenticated local inspection unless a
/// token environment variable is explicitly set.
pub async fn require_bearer(request: Request<Body>, next: Next) -> Response {
    if !is_protected_observability_path(request.uri().path()) {
        return next.run(request).await;
    }

    let expected = configured_token();
    let Some(expected) = expected else {
        if cfg!(debug_assertions) {
            return next.run(request).await;
        }
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "observability authentication is not configured",
        )
            .into_response();
    };

    let supplied = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_bearer_token);

    if supplied.is_some_and(|supplied| constant_time_eq(supplied, &expected)) {
        return next.run(request).await;
    }

    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer realm=\"observability\"")],
        "observability bearer token required",
    )
        .into_response()
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
        "/metrics"
            | "/metrics/"
            | "/api/_health/metrics"
            | "/health/runtime"
            | "/health/modules"
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
    use super::{constant_time_eq, is_protected_observability_path, parse_bearer_token};

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
