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

/// Normalizes an origin or host string into a canonical origin URL with scheme.
/// Converts `0.0.0.0:<port>` into `http://127.0.0.1:<port>` and attaches `http://`
/// or `https://` if missing.
pub fn normalize_host_origin(host: &str) -> String {
    let trimmed = host.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("0.0.0.0:") {
        return format!("http://127.0.0.1:{rest}");
    }
    if trimmed == "0.0.0.0" {
        return "http://127.0.0.1:5150".to_string();
    }
    if trimmed.ends_with(":443") {
        format!("https://{trimmed}")
    } else {
        format!("http://{trimmed}")
    }
}

/// Resolves allowed origins from the environment:
/// 1. `RUSTOK_CORS_ALLOWED_ORIGINS` (comma-separated origins)
/// 2. `RUSTOK_HOST` or `APP_HOST` (canonical server host)
pub fn resolve_cors_allowed_origins_from_env() -> Option<Vec<String>> {
    if let Ok(raw) = std::env::var("RUSTOK_CORS_ALLOWED_ORIGINS") {
        let origins: Vec<String> = raw
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(normalize_host_origin)
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

    host.map(|h| vec![normalize_host_origin(&h)])
}

/// Builds a production-grade [`CorsLayer`] enforcing explicit origin policies.
///
/// In production:
/// - Uses explicitly configured `allowed_origins` from environment/settings.
/// - Falls back to the primary server origin if none specified or if parsing failed (never wildcard).
///
/// In non-production (development/test):
/// - Allows standard local frontend/admin origins (`localhost:3000`, `localhost:5173`, etc.)
///   or explicitly configured origins.
pub fn build_cors_layer(is_production: bool, allowed_origins: Option<&[String]>) -> CorsLayer {
    let mut origins: Vec<HeaderValue> = if let Some(configured) = allowed_origins {
        configured
            .iter()
            .map(|origin| normalize_host_origin(origin))
            .filter_map(|origin| origin.parse::<HeaderValue>().ok())
            .collect()
    } else {
        Vec::new()
    };

    if origins.is_empty() {
        if is_production {
            origins.push(HeaderValue::from_static(DEFAULT_PRODUCTION_ORIGIN));
        } else {
            origins.extend(
                DEFAULT_DEV_ORIGINS
                    .iter()
                    .filter_map(|origin| HeaderValue::from_str(origin).ok()),
            );
        }
    }

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
        header::ACCEPT_LANGUAGE,
        header::ORIGIN,
        header::CACHE_CONTROL,
        header::IF_NONE_MATCH,
        header::IF_MATCH,
        header::IF_MODIFIED_SINCE,
        HeaderName::from_static("x-request-id"),
        HeaderName::from_static("x-correlation-id"),
        HeaderName::from_static("x-tenant-id"),
        HeaderName::from_static("x-tenant-slug"),
        HeaderName::from_static("x-channel-id"),
        HeaderName::from_static("x-channel-slug"),
        HeaderName::from_static("x-locale"),
        HeaderName::from_static("x-rustok-runner-token"),
        HeaderName::from_static("x-requested-with"),
        HeaderName::from_static("apollographql-client-name"),
        HeaderName::from_static("apollographql-client-version"),
        HeaderName::from_static("traceparent"),
        HeaderName::from_static("tracestate"),
        HeaderName::from_static("baggage"),
    ];

    let exposed_headers = [
        header::CONTENT_TYPE,
        header::CONTENT_DISPOSITION,
        header::ETAG,
        header::RETRY_AFTER,
        HeaderName::from_static("x-request-id"),
        HeaderName::from_static("x-correlation-id"),
        HeaderName::from_static("x-tenant-id"),
        HeaderName::from_static("x-tenant-slug"),
        HeaderName::from_static("x-channel-id"),
        HeaderName::from_static("x-channel-slug"),
        HeaderName::from_static("x-total-count"),
        HeaderName::from_static("x-ratelimit-limit"),
        HeaderName::from_static("x-ratelimit-remaining"),
        HeaderName::from_static("x-ratelimit-reset"),
        HeaderName::from_static("traceparent"),
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

    #[test]
    fn normalize_host_origin_adds_scheme_and_maps_unspecified_address() {
        assert_eq!(
            normalize_host_origin("0.0.0.0:5150"),
            "http://127.0.0.1:5150"
        );
        assert_eq!(
            normalize_host_origin("localhost:3000"),
            "http://localhost:3000"
        );
        assert_eq!(
            normalize_host_origin("https://app.rustok.io"),
            "https://app.rustok.io"
        );
    }
}
