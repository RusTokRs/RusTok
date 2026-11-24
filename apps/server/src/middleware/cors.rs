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

/// Accept only actual HTTP(S) origins, never URL credentials, paths,
/// queries, fragments, or non-web schemes. The Origin request header is
/// serialized as scheme + authority, so preserving a configured path would
/// silently produce a non-matching or misleading allow-list entry.
fn validated_cors_origin(configured: &str) -> Option<HeaderValue> {
    let configured = configured.trim();
    // The general development normalization maps an unspecified listener
    // address to loopback. That is useful for local URLs but must not turn
    // an explicitly supplied bind address into credentialed CORS authority.
    if configured == "0.0.0.0" || configured.starts_with("0.0.0.0:") {
        return None;
    }
    let normalized = normalize_host_origin(configured);
    let url = url::Url::parse(&normalized).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || matches!(url.host(), Some(url::Host::Ipv4(ip)) if ip.is_unspecified())
        || matches!(url.host(), Some(url::Host::Ipv6(ip)) if ip.is_unspecified())
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    // Canonicalize scheme, host and default ports. It avoids treating
    // multiple textual variants as independent trusted origins.
    HeaderValue::from_str(&url.origin().ascii_serialization()).ok()
}

/// Resolves allowed origins from the environment:
/// 1. `RUSTOK_CORS_ALLOWED_ORIGINS` (comma-separated explicit origins)
/// 2. In development ONLY, `RUSTOK_HOST` or `APP_HOST` (local server host).
/// Production bind addresses are not proof of a browser origin's authorization.
pub fn resolve_cors_allowed_origins_from_env() -> Option<Vec<String>> {
    let explicit = std::env::var("RUSTOK_CORS_ALLOWED_ORIGINS").ok();
    let host = std::env::var("RUSTOK_HOST")
        .or_else(|_| std::env::var("APP_HOST"))
        .ok();
    cors_origins_from_configuration(
        explicit.as_deref(),
        host.as_deref(),
        crate::common::is_production_security_mode(),
    )
}

fn cors_origins_from_configuration(
    explicit: Option<&str>,
    host: Option<&str>,
    is_production: bool,
) -> Option<Vec<String>> {
    if let Some(explicit) = explicit {
        let origins: Vec<String> = explicit
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(normalize_host_origin)
            .collect();
        if !origins.is_empty() {
            return Some(origins);
        }
    }

    // A production listener commonly binds to 0.0.0.0. Treating that bind
    // address as an allow-listed localhost Origin would grant arbitrary local
    // browser pages credentialed cross-origin access to the deployed host.
    if is_production {
        return None;
    }

    host.map(str::trim)
        .filter(|host| !host.is_empty())
        .map(|host| vec![normalize_host_origin(host)])
}

/// Builds a production-grade [`CorsLayer`] enforcing explicit origin policies.
///
/// In production:
/// - Uses explicitly configured `allowed_origins` from environment/settings.
/// - Denies every cross-origin request when none are configured/valid (never wildcard or implicit loopback).
///
/// In non-production (development/test):
/// - Allows standard local frontend/admin origins (`localhost:3000`, `localhost:5173`, etc.)
///   or explicitly configured origins.
pub fn build_cors_layer(is_production: bool, allowed_origins: Option<&[String]>) -> CorsLayer {
    let mut origins: Vec<HeaderValue> = if let Some(configured) = allowed_origins {
        configured
            .iter()
            .enumerate()
            .filter_map(|(origin_index, origin)| {
                match validated_cors_origin(origin) {
                    Some(value) => Some(value),
                    None => {
                        // Never expose a possibly credential-bearing URL in
                        // logs; log only the position of the invalid setting.
                        tracing::error!(
                            origin_index,
                            error_code = "cors_origin_invalid",
                            "Configured CORS origin is not a valid HTTP(S) origin; requests from it will be denied"
                        );
                        None
                    }
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    if origins.is_empty() {
        if is_production {
            // An unspecified bind address is never permission to trust
            // loopback browser origins. Keep the origin list empty rather
            // than inventing a credential-bearing CORS allow-list entry.
            tracing::error!(
                configured = allowed_origins.map(<[String]>::len).unwrap_or(0),
                error_code = "cors_no_valid_production_origins",
                "No valid CORS origins configured in production; all cross-origin requests are denied"
            );
        } else {
            origins.extend(
                DEFAULT_DEV_ORIGINS
                    .iter()
                    .map(|&origin| HeaderValue::from_static(origin)),
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
    async fn production_cors_without_valid_origins_denies_all_cross_origin_requests() {
        for origin in [
            "http://127.0.0.1:5150",
            "http://localhost:5150",
            "https://attacker.example.com",
        ] {
            assert_eq!(
                preflight_allowed_origin(true, None, origin).await,
                None,
                "missing production origins must never grant credentialed access to {origin}"
            );
        }
        let invalid = vec![
            "https://user:password@trusted.example.com".to_string(),
            "https://trusted.example.com/admin".to_string(),
        ];
        assert_eq!(
            preflight_allowed_origin(true, Some(&invalid), "http://127.0.0.1:5150").await,
            None
        );
        assert_eq!(
            preflight_allowed_origin(true, Some(&invalid), "https://trusted.example.com").await,
            None
        );
    }

    #[test]
    fn cors_origin_validation_rejects_url_credentials_and_non_origin_parts() {
        for invalid in [
            "https://user:password@trusted.example.com",
            "https://trusted.example.com/admin",
            "https://trusted.example.com?token=secret",
            "https://trusted.example.com/#fragment",
            "ftp://trusted.example.com",
            "javascript:alert(1)",
            "0.0.0.0",
            "0.0.0.0:5150",
            "http://0.0.0.0:5150",
            "http://[::]:5150",
        ] {
            assert!(
                validated_cors_origin(invalid).is_none(),
                "invalid origin must not be trusted"
            );
        }
        assert_eq!(
            validated_cors_origin("https://TRUSTED.EXAMPLE.COM:443/")
                .as_ref()
                .and_then(|value| value.to_str().ok()),
            Some("https://trusted.example.com")
        );
        assert_eq!(
            validated_cors_origin("localhost:3000")
                .as_ref()
                .and_then(|value| value.to_str().ok()),
            Some("http://localhost:3000")
        );
    }

    #[test]
    fn production_does_not_promote_bind_host_to_cors_authorization() {
        assert_eq!(
            cors_origins_from_configuration(None, Some("0.0.0.0:5150"), true),
            None,
            "a production bind address is not an authorized browser origin"
        );
        assert_eq!(
            cors_origins_from_configuration(Some("  "), Some("localhost:5150"), true),
            None
        );
        assert_eq!(
            cors_origins_from_configuration(
                Some("https://admin.example.com"),
                Some("0.0.0.0:5150"),
                true
            ),
            Some(vec!["https://admin.example.com".to_string()])
        );
        assert_eq!(
            cors_origins_from_configuration(None, Some("0.0.0.0:5150"), false),
            Some(vec!["http://127.0.0.1:5150".to_string()]),
            "developer bind-host fallback retains its existing behavior"
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
