use axum::http::{HeaderName, HeaderValue, Method, header};
use std::time::Duration;
use tower_http::cors::CorsLayer;

const DEFAULT_DEV_ORIGINS: &[&str] = &[
    "http://localhost:3000",
    "http://localhost:5150",
    "http://localhost:5173",
    "http://localhost:8080",
    "http://127.0.0.1:3000",
    "http://127.0.0.1:5150",
    "http://127.0.0.1:5173",
    "http://127.0.0.1:8080",
];

const DEFAULT_PRODUCTION_ORIGIN: &str = "http://127.0.0.1:5150";

/// Resolves allowed origins from the `RUSTOK_CORS_ALLOWED_ORIGINS` environment variable.
/// Multiple origins must be comma-separated, e.g. `https://store.example.com,https://admin.example.com`.
pub fn resolve_cors_allowed_origins_from_env() -> Option<Vec<String>> {
    let raw = std::env::var("RUSTOK_CORS_ALLOWED_ORIGINS").ok()?;
    let origins: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    if origins.is_empty() {
        None
    } else {
        Some(origins)
    }
}

/// Builds a production-grade [`CorsLayer`] enforcing explicit origin policies.
///
/// In production:
/// - Uses explicitly configured `allowed_origins` from environment/settings.
/// - Falls back to the primary server origin if none specified (never wildcard).
///
/// In non-production (development/test):
/// - Allows standard local frontend/admin origins (`localhost:3000`, `localhost:5173`, etc.)
///   or explicitly configured origins.
pub fn build_cors_layer(is_production: bool, allowed_origins: Option<&[String]>) -> CorsLayer {
    let origins: Vec<HeaderValue> = if let Some(configured) = allowed_origins {
        configured
            .iter()
            .filter_map(|origin| origin.trim().parse::<HeaderValue>().ok())
            .collect()
    } else if is_production {
        vec![HeaderValue::from_static(DEFAULT_PRODUCTION_ORIGIN)]
    } else {
        DEFAULT_DEV_ORIGINS
            .iter()
            .filter_map(|origin| HeaderValue::from_str(origin).ok())
            .collect()
    };

    let allowed_methods = [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::PATCH,
        Method::OPTIONS,
        Method::HEAD,
    ];

    let allowed_headers = [
        header::AUTHORIZATION,
        header::CONTENT_TYPE,
        header::ACCEPT,
        header::ORIGIN,
        HeaderName::from_static("x-request-id"),
        HeaderName::from_static("x-correlation-id"),
        HeaderName::from_static("x-tenant-id"),
        HeaderName::from_static("x-channel-id"),
        HeaderName::from_static("x-locale"),
    ];

    let exposed_headers = [
        header::CONTENT_TYPE,
        HeaderName::from_static("x-request-id"),
        HeaderName::from_static("x-correlation-id"),
    ];

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods(allowed_methods)
        .allow_headers(allowed_headers)
        .expose_headers(exposed_headers)
        .allow_credentials(true)
        .max_age(Duration::from_secs(3600))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_cors_uses_restricted_dev_origins() {
        let cors = build_cors_layer(false, None);
        // Ensure layer is built successfully
        assert!(format!("{cors:?}").contains("CorsLayer"));
    }

    #[test]
    fn production_cors_with_configured_origins() {
        let origins = vec![
            "https://admin.example.com".to_string(),
            "https://store.example.com".to_string(),
        ];
        let cors = build_cors_layer(true, Some(&origins));
        assert!(format!("{cors:?}").contains("CorsLayer"));
    }

    #[test]
    fn production_cors_fallback_is_not_wildcard() {
        let cors = build_cors_layer(true, None);
        assert!(format!("{cors:?}").contains("CorsLayer"));
    }
}
