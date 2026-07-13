use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

const METRICS_TOKEN_ENV: &str = "RUSTOK_METRICS_BEARER_TOKEN";

/// Protect the Prometheus endpoint without coupling it to tenant resolution.
///
/// Production is fail-closed when no token is configured. Debug builds retain
/// unauthenticated local scraping unless the token environment variable is
/// explicitly set, in which case the same bearer check is enforced.
pub async fn require_bearer(request: Request<Body>, next: Next) -> Response {
    if !is_metrics_path(request.uri().path()) {
        return next.run(request).await;
    }

    let expected = std::env::var(METRICS_TOKEN_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let Some(expected) = expected else {
        if cfg!(debug_assertions) {
            return next.run(request).await;
        }
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "metrics authentication is not configured",
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
        [(header::WWW_AUTHENTICATE, "Bearer realm=\"metrics\"")],
        "metrics bearer token required",
    )
        .into_response()
}

fn is_metrics_path(path: &str) -> bool {
    matches!(path, "/metrics" | "/metrics/" | "/api/_health/metrics")
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
    use super::{constant_time_eq, is_metrics_path, parse_bearer_token};

    #[test]
    fn matches_only_metrics_paths() {
        assert!(is_metrics_path("/metrics"));
        assert!(is_metrics_path("/metrics/"));
        assert!(is_metrics_path("/api/_health/metrics"));
        assert!(!is_metrics_path("/api/graphql"));
        assert!(!is_metrics_path("/metrics/debug"));
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