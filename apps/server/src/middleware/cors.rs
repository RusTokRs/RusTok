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
            .filter_map(|origin| match origin.parse::<HeaderValue>() {
                Ok(value) => Some(value),
                Err(error) => {
                    // Dropping an origin silently would narrow the allow-list
                    // without any operator-visible signal.
                    tracing::error!(
                        %origin,
                        %error,
                        "Configured CORS origin is not a valid header value; \
                         requests from it will be denied"
                    );
                    None
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    if origins.is_empty() {
        if is_production {
            // Falling back to a loopback origin in production means the
            // deployment supplied no usable allow-list. Serving traffic with
            // this silently would look healthy while every real browser origin
            // is rejected, and it would also be trusted by mistake if a proxy
            // ever fronted the listener on 127.0.0.1.
            tracing::error!(
                fallback_origin = DEFAULT_PRODUCTION_ORIGIN,
                configured = allowed_origins.map(<[String]>::len).unwrap_or(0),
                "No usable CORS origin configured in production; the allow-list \
                 falls back to a loopback origin and browser clients will be rejected. \
                 Set the allowed origins explicitly."
            );
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
        HeaderName::from_static("x-rustok-session-request"),
        HeaderName::from_static("idempotency-key"),
        HeaderName::from_static("x-rustok-host-token"),
        HeaderName::from_static("x-rustok-setup-token"),
        HeaderName::from_static("x-rustok-observability-token"),
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

    #[tokio::test]
    async fn preflight_allows_session_idempotency_and_host_credential_headers() {
        use axum::body::Body;
        use axum::http::{Method, Request, StatusCode, header};
        use tower::ServiceExt;

        let origins = vec!["http://localhost:3000".to_string()];
        let app = axum::Router::new()
            .route(
                "/api/auth/session/refresh",
                axum::routing::post(|| async { "" }),
            )
            .layer(build_cors_layer(false, Some(&origins)));

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/api/auth/session/refresh")
                    .header(header::ORIGIN, "http://localhost:3000")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                    .header(
                        header::ACCESS_CONTROL_REQUEST_HEADERS,
                        "authorization, content-type, x-rustok-session-request, idempotency-key, x-rustok-host-token, x-rustok-setup-token, x-rustok-observability-token",
                    )
                    .body(Body::empty())
                    .expect("preflight request"),
            )
            .await
            .expect("preflight response");

        assert_eq!(response.status(), StatusCode::OK);
        let allowed = response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
            .expect("allow-headers")
            .to_str()
            .expect("allow-headers utf8")
            .to_ascii_lowercase();
        for required in [
            "x-rustok-session-request",
            "idempotency-key",
            "x-rustok-host-token",
            "x-rustok-setup-token",
            "x-rustok-observability-token",
        ] {
            assert!(
                allowed.contains(required),
                "missing {required} in {allowed}"
            );
        }
    }

    /// Sends a preflight and returns the `access-control-allow-origin` header the
    /// layer produced, if any. This is the only assertion surface that reflects
    /// what a browser will actually be permitted to do: a `Debug` rendering of
    /// the layer proves nothing about the resolved policy.
    async fn preflight_allowed_origin(
        is_production: bool,
        configured: Option<&[String]>,
        origin: &str,
    ) -> Option<String> {
        use axum::body::Body;
        use axum::http::{Method, Request, header};
        use tower::ServiceExt;

        let app = axum::Router::new()
            .route("/probe", axum::routing::get(|| async { "" }))
            .layer(build_cors_layer(is_production, configured));

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/probe")
                    .header(header::ORIGIN, origin)
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .body(Body::empty())
                    .expect("preflight request"),
            )
            .await
            .expect("preflight response");

        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(|value| value.to_str().expect("allow-origin utf8").to_string())
    }

    #[tokio::test]
    async fn development_cors_allows_configured_local_origin_and_denies_others() {
        // The development policy is a fixed local allow-list. It must admit a
        // listed admin origin and reject an unrelated public site.
        assert_eq!(
            preflight_allowed_origin(false, None, "http://localhost:5173").await,
            Some("http://localhost:5173".to_string())
        );
        assert_eq!(
            preflight_allowed_origin(false, None, "https://attacker.example.com").await,
            None,
            "development mode must not admit arbitrary origins"
        );
    }

    #[tokio::test]
    async fn production_cors_with_configured_origins() {
        let origins = vec![
            "https://admin.example.com".to_string(),
            "https://store.example.com".to_string(),
        ];
        for origin in ["https://admin.example.com", "https://store.example.com"] {
            assert_eq!(
                preflight_allowed_origin(true, Some(&origins), origin).await,
                Some(origin.to_string()),
                "{origin} is configured and must be allowed"
            );
        }
        assert_eq!(
            preflight_allowed_origin(true, Some(&origins), "https://unconfigured.example.com")
                .await,
            None,
            "production must deny origins that are not explicitly configured"
        );
    }

    #[tokio::test]
    async fn production_cors_fallback_is_not_wildcard() {
        // With nothing configured, production falls back to the single primary
        // server origin. The fallback must never become `*` (which combined with
        // credentials would let any site drive an authenticated session).
        let allowed = preflight_allowed_origin(true, None, DEFAULT_PRODUCTION_ORIGIN).await;
        assert_eq!(
            allowed,
            Some(DEFAULT_PRODUCTION_ORIGIN.to_string()),
            "the operator-visible fallback origin must stay usable"
        );
        assert_ne!(allowed.as_deref(), Some("*"));

        assert_eq!(
            preflight_allowed_origin(true, None, "https://attacker.example.com").await,
            None,
            "the production fallback must not admit an arbitrary origin"
        );
    }

    #[test]
    fn unparseable_configured_origins_are_dropped_not_trusted() {
        // A CR/LF cannot appear in a header value. The configured origin must be
        // rejected; before the explicit logging branch it was silently dropped,
        // which made a misconfiguration indistinguishable from "no origins".
        let origins = vec![
            "https://good.example.com".to_string(),
            "https://evil.example.com\r\nX-Injected: 1".to_string(),
        ];
        let cors = build_cors_layer(true, Some(&origins));
        let debug = format!("{cors:?}");
        assert!(
            debug.contains("good.example.com"),
            "the valid origin must be kept: {debug}"
        );
        assert!(
            !debug.contains("evil.example.com"),
            "the unparseable origin must never enter the allow-list: {debug}"
        );
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
