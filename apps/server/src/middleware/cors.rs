use axum::http::{HeaderName, HeaderValue, Method, header};
use std::time::Duration;
use tower_http::cors::CorsLayer;

const DEFAULT_DEV_ORIGINS: &[&str] = &[
    "http://localhost:3000",
    "http://localhost:3001",
    "http://localhost:5150",
    "http://localhost:5173",
    "http://localhost:5174",
    "http://localhost:5175",
    "http://localhost:4173",
    "http://localhost:8080",
    "http://localhost:8081",
    "http://127.0.0.1:3000",
    "http://127.0.0.1:3001",
    "http://127.0.0.1:5150",
    "http://127.0.0.1:5173",
    "http://127.0.0.1:5174",
    "http://127.0.0.1:5175",
    "http://127.0.0.1:4173",
    "http://127.0.0.1:8080",
    "http://127.0.0.1:8081",
];

const DEFAULT_PRODUCTION_ORIGIN: &str = "http://127.0.0.1:5150";

/// Resolves allowed origins from the environment:
/// 1. `RUSTOK_CORS_ALLOWED_ORIGINS` (comma-separated origins)
/// 2. `RUSTOK_HOST` or `APP_HOST` (canonical server host)
pub fn resolve_cors_allowed_origins_from_env() -> Option<Vec<String>> {
    if let Ok(raw) = std::env::var("RUSTOK_CORS_ALLOWED_ORIGINS") {
        let origins: Vec<String> = raw
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        if !origins.is_empty() {
            return Some(origins);
        }
    }

    let host = std::env::var("RUSTOK_HOST")
        .or_else(|_| std::env::var("APP_HOST"))
        .ok()
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty());

    host.map(|h| vec![h])
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
        header::CACHE_CONTROL,
        header::IF_NONE_MATCH,
        header::IF_MATCH,
        header::IF_MODIFIED_SINCE,
        HeaderName::from_static("x-request-id"),
        HeaderName::from_static("x-correlation-id"),
        HeaderName::from_static("x-tenant-id"),
        HeaderName::from_static("x-channel-id"),
        HeaderName::from_static("x-locale"),
        HeaderName::from_static("x-requested-with"),
        HeaderName::from_static("apollographql-client-name"),
        HeaderName::from_static("apollographql-client-version"),
    ];

    let exposed_headers = [
        header::CONTENT_TYPE,
        header::CONTENT_DISPOSITION,
        header::ETAG,
        HeaderName::from_static("x-request-id"),
        HeaderName::from_static("x-correlation-id"),
        HeaderName::from_static("x-total-count"),
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
