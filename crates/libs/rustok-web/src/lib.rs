//! # RusToK Web
//!
//! Axum HTTP boundary helpers for RusToK server and module backend adapters.
//!
//! Provides the neutral import surface for HTTP error response mapping, typed status codes,
//! per-response Content Security Policy (CSP) nonces, and release-qualified browser asset delivery.
//!
//! ## Entry points
//!
//! - [`HttpError`], [`HttpResult`]: Typed HTTP error model mapping to standard status codes.
//! - [`ErrorBody`]: Standard JSON error payload response structure.
//! - [`CspNonce`]: Per-response Content Security Policy nonce generator.
//! - [`BrowserAssetRegistry`]: Release-qualified content-addressed asset serving with strong ETags.

use std::fmt::Write as _;

#[cfg(test)]
mod contract_tests;

use axum::{
    Json,
    body::Body,
    http::{
        HeaderMap, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
    },
    response::{IntoResponse, Response},
};
use rustok_api::{PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

/// Per-response Content Security Policy nonce shared by HTTP hosts and trusted UI renderers.
///
/// The value is generated from UUIDv4 randomness and encoded as lowercase hexadecimal, which is a
/// valid subset of the CSP `base64-value` grammar and safe to place in HTML attributes and headers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CspNonce(String);

impl CspNonce {
    pub fn generate() -> Self {
        Self(Uuid::new_v4().simple().to_string())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn source_expression(&self) -> String {
        format!("'nonce-{}'", self.0)
    }
}

/// Canonical public HTTP error body. Retry advice is independent of HTTP status and outcome.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    /// Exact owner advice, when supplied. Absence means unknown, not true or false.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_retryable"
    )]
    #[schema(nullable = false)]
    pub retryable: Option<bool>,
}

// Missing is accepted via serde(default); a present null or non-boolean is not an owner decision.
fn deserialize_retryable<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    bool::deserialize(deserializer).map(Some)
}

impl ErrorBody {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: None,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{status}: {code}: {message}")]
pub struct HttpError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
    /// Exact owner retry advice, if known. This does not certify a durable write outcome.
    pub retryable: Option<bool>,
    retry_after_seconds: Option<u64>,
    extra_headers: Vec<(String, String)>,
}

impl HttpError {
    pub fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
            retryable: None,
            retry_after_seconds: None,
            extra_headers: Vec::new(),
        }
    }

    /// Attach an owner's decision without deriving it from status, code, or message.
    ///
    /// This is advice, not permission to replay a mutation or replace its idempotency key.
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = Some(retryable);
        self
    }

    pub fn with_retry_after(mut self, seconds: u64) -> Self {
        self.retry_after_seconds = Some(seconds);
        self
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.push((name.into(), value.into()));
        self
    }

    pub fn bad_request(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn unauthorized(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, code, message)
    }

    pub fn forbidden(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, code, message)
    }

    pub fn not_found(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    pub fn conflict(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    pub fn service_unavailable(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, code, message)
    }

    pub fn gateway_timeout(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(StatusCode::GATEWAY_TIMEOUT, code, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
    }

    pub fn body(&self) -> ErrorBody {
        ErrorBody {
            code: self.code.clone(),
            message: self.message.clone(),
            retryable: self.retryable,
        }
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        let mut response = (self.status, Json(self.body())).into_response();
        if let Some(seconds) = self.retry_after_seconds {
            if let Ok(value) = axum::http::HeaderValue::from_str(&seconds.to_string()) {
                response.headers_mut().insert(
                    axum::http::header::RETRY_AFTER,
                    value,
                );
            }
        }
        for (name, value) in self.extra_headers {
            if let (Ok(header_name), Ok(header_value)) = (
                axum::http::HeaderName::from_bytes(name.as_bytes()),
                axum::http::HeaderValue::from_str(&value),
            ) {
                response.headers_mut().insert(header_name, header_value);
            }
        }
        response
    }
}

pub type HttpResult<T> = Result<T, HttpError>;

/// Build a same-origin immutable-input asset response with content-derived revalidation.
pub fn embedded_asset_response(
    headers: &HeaderMap,
    bytes: &'static [u8],
    content_type: &'static str,
    cache_control: &'static str,
    _context: &'static str,
) -> Response {
    let etag = content_etag(bytes);
    let not_modified = headers
        .get(IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| if_none_match_matches(value, etag.as_str()));
    let builder = Response::builder()
        .header(CACHE_CONTROL, cache_control)
        .header(ETAG, etag)
        .header("cross-origin-resource-policy", "same-origin");
    if not_modified {
        return builder
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::NOT_MODIFIED)
                    .body(Body::empty())
                    .unwrap_or_default()
            });
    }
    builder
        .header(CONTENT_TYPE, content_type)
        .status(StatusCode::OK)
        .body(Body::from(bytes))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Internal Server Error"))
                .unwrap_or_default()
        })
}

fn content_etag(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2 + 2);
    encoded.push('"');
    // INVARIANT: Formatting fixed Sha256 byte digest into in-memory String cannot fail.
    for byte in digest {
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded.push('"');
    encoded
}

fn if_none_match_matches(value: &str, etag: &str) -> bool {
    value.split(',').map(str::trim).any(|candidate| {
        candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == etag
    })
}

/// Preserve the typed semantics of a module port failure at an HTTP boundary.
///
/// Infrastructure failures intentionally receive stable public messages instead of exposing
/// storage or connector details. The exact owner retry decision is forwarded independently of
/// kind-to-status mapping and never establishes a committed or rejected write outcome.
pub fn port_error_to_http_error(error: PortError) -> HttpError {
    let status = match error.kind {
        PortErrorKind::Validation => StatusCode::BAD_REQUEST,
        PortErrorKind::NotFound => StatusCode::NOT_FOUND,
        PortErrorKind::Conflict => StatusCode::CONFLICT,
        PortErrorKind::Forbidden => StatusCode::FORBIDDEN,
        PortErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        PortErrorKind::Timeout => StatusCode::GATEWAY_TIMEOUT,
        PortErrorKind::InvariantViolation => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let message = match error.kind {
        PortErrorKind::Unavailable => {
            "The requested service is temporarily unavailable".to_string()
        }
        PortErrorKind::Timeout => "The requested service timed out".to_string(),
        PortErrorKind::InvariantViolation => {
            "The requested operation could not be completed".to_string()
        }
        PortErrorKind::Validation
        | PortErrorKind::NotFound
        | PortErrorKind::Conflict
        | PortErrorKind::Forbidden => error.message,
    };

    HttpError::new(status, error.code, message).with_retryable(error.retryable)
}

pub fn json_response<T>(value: T) -> axum::response::Response
where
    T: Serialize,
{
    Json(value).into_response()
}

pub mod browser_assets;

pub use browser_assets::{
    BrowserAssetError, BrowserAssetRegistry, IMMUTABLE_ASSET_CACHE_CONTROL,
    MISSING_ASSET_CACHE_CONTROL, ReleaseAssetSet, ReleaseQualifiedAsset,
};

#[cfg(test)]
mod tests {
    use super::{
        BrowserAssetRegistry, CspNonce, ErrorBody, HttpError, IMMUTABLE_ASSET_CACHE_CONTROL,
        MISSING_ASSET_CACHE_CONTROL, ReleaseQualifiedAsset, embedded_asset_response,
        port_error_to_http_error,
    };
    use axum::http::{HeaderMap, HeaderValue, StatusCode, header::IF_NONE_MATCH};
    use chrono::{Duration, Utc};
    use rustok_api::{PortError, PortErrorKind};
    use serde_json::json;

    #[test]
    fn generated_csp_nonce_is_attribute_and_header_safe() {
        let nonce = CspNonce::generate();

        assert_eq!(nonce.as_str().len(), 32);
        assert!(
            nonce
                .as_str()
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        );
        assert_eq!(
            nonce.source_expression(),
            format!("'nonce-{}'", nonce.as_str())
        );
    }

    #[test]
    fn port_errors_preserve_transport_status_and_safe_domain_evidence() {
        let not_found =
            port_error_to_http_error(PortError::not_found("cart.not_found", "cart was not found"));
        assert_eq!(not_found.status, StatusCode::NOT_FOUND);
        assert_eq!(not_found.code, "cart.not_found");
        assert_eq!(not_found.message, "cart was not found");

        let unavailable = port_error_to_http_error(PortError::unavailable(
            "cart.database_unavailable",
            "secret database endpoint failed",
        ));
        assert_eq!(unavailable.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(unavailable.code, "cart.database_unavailable");
        assert_eq!(
            unavailable.message,
            "The requested service is temporarily unavailable"
        );
    }

    #[test]
    fn unknown_retry_advice_is_absent_and_never_derived_from_status() {
        for status in [
            StatusCode::BAD_REQUEST,
            StatusCode::CONFLICT,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::SERVICE_UNAVAILABLE,
            StatusCode::GATEWAY_TIMEOUT,
        ] {
            let error = HttpError::new(status, "owner.failure", "Public refusal");
            assert_eq!(error.retryable, None);
            assert_eq!(error.body().retryable, None);
            let wire = serde_json::to_value(error.body()).unwrap();
            assert_eq!(
                wire,
                json!({ "code": "owner.failure", "message": "Public refusal" })
            );
            let decoded: ErrorBody = serde_json::from_value(wire).unwrap();
            assert_eq!(decoded.retryable, None);
        }
    }

    #[test]
    fn explicit_true_and_false_survive_body_and_json_round_trip() {
        for retryable in [false, true] {
            for status in [StatusCode::CONFLICT, StatusCode::SERVICE_UNAVAILABLE] {
                let error = HttpError::new(status, "owner.failure", "Public refusal")
                    .with_retryable(retryable);
                assert_eq!(error.retryable, Some(retryable));
                let wire = serde_json::to_value(error.body()).unwrap();
                assert_eq!(wire["retryable"], json!(retryable));
                let decoded: ErrorBody = serde_json::from_value(wire).unwrap();
                assert_eq!(decoded.retryable, Some(retryable));
                assert_eq!(decoded.code, "owner.failure");
                assert_eq!(decoded.message, "Public refusal");
            }
        }
    }

    #[test]
    fn malformed_present_retry_advice_is_not_an_absent_or_coerced_decision() {
        for value in [
            json!(null),
            json!("false"),
            json!("true"),
            json!(0),
            json!(1),
            json!([]),
            json!({}),
        ] {
            let wire =
                json!({ "code": "owner.failure", "message": "Public refusal", "retryable": value });
            assert!(serde_json::from_value::<ErrorBody>(wire).is_err());
        }
        let body = ErrorBody::new("owner.failure", "Public refusal");
        assert_eq!(body.retryable, None);
    }

    #[test]
    fn every_port_error_kind_preserves_both_owner_bits_and_existing_public_policy() {
        let cases = [
            (
                PortErrorKind::Validation,
                StatusCode::BAD_REQUEST,
                "Safe domain refusal",
            ),
            (
                PortErrorKind::NotFound,
                StatusCode::NOT_FOUND,
                "Safe domain refusal",
            ),
            (
                PortErrorKind::Conflict,
                StatusCode::CONFLICT,
                "Safe domain refusal",
            ),
            (
                PortErrorKind::Forbidden,
                StatusCode::FORBIDDEN,
                "Safe domain refusal",
            ),
            (
                PortErrorKind::Unavailable,
                StatusCode::SERVICE_UNAVAILABLE,
                "The requested service is temporarily unavailable",
            ),
            (
                PortErrorKind::Timeout,
                StatusCode::GATEWAY_TIMEOUT,
                "The requested service timed out",
            ),
            (
                PortErrorKind::InvariantViolation,
                StatusCode::INTERNAL_SERVER_ERROR,
                "The requested operation could not be completed",
            ),
        ];
        for (kind, status, message) in cases {
            for retryable in [false, true] {
                let error = port_error_to_http_error(PortError::new(
                    kind.clone(),
                    "owner.failure",
                    "Safe domain refusal",
                    retryable,
                ));
                assert_eq!(error.status, status);
                assert_eq!(error.code, "owner.failure");
                assert_eq!(error.message, message);
                assert_eq!(error.retryable, Some(retryable));
                assert_eq!(error.body().retryable, Some(retryable));
                assert_eq!(
                    serde_json::to_value(error.body()).unwrap()["retryable"],
                    json!(retryable)
                );
            }
        }
    }

    #[test]
    fn error_schema_declares_optional_non_nullable_boolean_retry_advice() {
        let schema = serde_json::to_value(<ErrorBody as utoipa::PartialSchema>::schema()).unwrap();
        assert_eq!(schema["properties"]["retryable"]["type"], "boolean");
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("code")));
        assert!(required.contains(&json!("message")));
        assert!(!required.contains(&json!("retryable")));
    }

    #[test]
    fn embedded_assets_support_strong_and_weak_revalidation() {
        let first = embedded_asset_response(
            &HeaderMap::new(),
            b"asset",
            "text/plain",
            "public, max-age=0, must-revalidate",
            "test asset response",
        );
        let etag = first.headers().get("etag").expect("etag").clone();
        let mut headers = HeaderMap::new();
        headers.insert(
            IF_NONE_MATCH,
            HeaderValue::from_str(format!("W/{}", etag.to_str().expect("etag text")).as_str())
                .expect("weak etag"),
        );
        let revalidated = embedded_asset_response(
            &headers,
            b"asset",
            "text/plain",
            "public, max-age=0, must-revalidate",
            "test asset response",
        );
        assert_eq!(revalidated.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(
            revalidated.headers()["cross-origin-resource-policy"],
            "same-origin"
        );
    }

    #[test]
    fn test_content_addressed_browser_asset_resolution_and_conditional_requests() {
        let mut registry = BrowserAssetRegistry::new(Duration::hours(24));
        let now = Utc::now();
        let release_id = "sha256:rel_11111111111111111111111111111111";

        let js_asset = ReleaseQualifiedAsset::new(
            "pkg/rustok_admin.js",
            "application/javascript",
            b"console.log('rustok admin');".to_vec(),
        );
        let css_asset = ReleaseQualifiedAsset::new(
            "pkg/rustok_admin.css",
            "text/css",
            b"body { margin: 0; }".to_vec(),
        );

        registry.register_release(release_id, vec![js_asset.clone(), css_asset], now);
        registry.activate_release(release_id, now).unwrap();

        // 1. Resolve asset: returns 200 OK with immutable cache control
        let empty_headers = HeaderMap::new();
        let res = registry.resolve_asset(&empty_headers, release_id, "pkg/rustok_admin.js", now);
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get("cache-control").unwrap(),
            IMMUTABLE_ASSET_CACHE_CONTROL
        );
        assert_eq!(
            res.headers().get("content-type").unwrap(),
            "application/javascript"
        );
        assert_eq!(res.headers().get("etag").unwrap(), &js_asset.etag);

        // 2. Conditional request with matching If-None-Match: returns 304 Not Modified
        let mut if_match_headers = HeaderMap::new();
        if_match_headers.insert(
            IF_NONE_MATCH,
            HeaderValue::from_str(&js_asset.etag).unwrap(),
        );
        let res_304 =
            registry.resolve_asset(&if_match_headers, release_id, "pkg/rustok_admin.js", now);
        assert_eq!(res_304.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(
            res_304.headers().get("cache-control").unwrap(),
            IMMUTABLE_ASSET_CACHE_CONTROL
        );
    }

    #[test]
    fn test_dual_n_and_n_plus_1_retention_across_rollout_and_rollback() {
        let mut registry = BrowserAssetRegistry::new(Duration::hours(24));
        let now = Utc::now();
        let release_n = "sha256:release_n_stable";
        let release_n_plus_1 = "sha256:release_n_plus_1_candidate";

        let asset_n = ReleaseQualifiedAsset::new(
            "pkg/app.js",
            "application/javascript",
            b"console.log('v1');".to_vec(),
        );
        let asset_n1 = ReleaseQualifiedAsset::new(
            "pkg/app.js",
            "application/javascript",
            b"console.log('v2');".to_vec(),
        );

        // 1. Initial State: Release N is active
        registry.register_release(release_n, vec![asset_n.clone()], now);
        registry.activate_release(release_n, now).unwrap();

        // 2. Pre-stage candidate N+1 before mutation
        registry.register_release(release_n_plus_1, vec![asset_n1.clone()], now);

        let headers = HeaderMap::new();

        // 3. Both N and N+1 assets resolve successfully during preparation
        assert_eq!(
            registry
                .resolve_asset(&headers, release_n, "pkg/app.js", now)
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            registry
                .resolve_asset(&headers, release_n_plus_1, "pkg/app.js", now)
                .status(),
            StatusCode::OK
        );

        // 4. Activate N+1 (promoting to active release)
        let later = now + Duration::minutes(5);
        registry.activate_release(release_n_plus_1, later).unwrap();
        assert_eq!(registry.active_release_id(), Some(release_n_plus_1));

        // Clients that still hold release N HTML can STILL resolve release N assets!
        assert_eq!(
            registry
                .resolve_asset(&headers, release_n, "pkg/app.js", later)
                .status(),
            StatusCode::OK
        );
        // Clients that received release N+1 HTML resolve release N+1 assets!
        assert_eq!(
            registry
                .resolve_asset(&headers, release_n_plus_1, "pkg/app.js", later)
                .status(),
            StatusCode::OK
        );

        // 5. Instant Rollback to release N
        let rollback_time = later + Duration::minutes(10);
        registry.activate_release(release_n, rollback_time).unwrap();
        assert_eq!(registry.active_release_id(), Some(release_n));

        // After rollback, clients with N+1 HTML can STILL resolve N+1 assets during retention window!
        assert_eq!(
            registry
                .resolve_asset(&headers, release_n_plus_1, "pkg/app.js", rollback_time)
                .status(),
            StatusCode::OK
        );
    }

    #[test]
    fn test_strict_not_found_for_missing_or_expired_immutable_asset() {
        let mut registry = BrowserAssetRegistry::new(Duration::hours(24));
        let now = Utc::now();
        let release_id = "sha256:release_known";

        let asset = ReleaseQualifiedAsset::new(
            "pkg/app.js",
            "application/javascript",
            b"console.log('app');".to_vec(),
        );
        registry.register_release(release_id, vec![asset], now);
        registry.activate_release(release_id, now).unwrap();

        let headers = HeaderMap::new();

        // 1. Missing asset in valid release: returns strict 404 NOT_FOUND (never HTML!)
        let res_missing = registry.resolve_asset(&headers, release_id, "pkg/missing.png", now);
        assert_eq!(res_missing.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            res_missing.headers().get("cache-control").unwrap(),
            MISSING_ASSET_CACHE_CONTROL
        );
        assert_eq!(
            res_missing.headers().get("content-type").unwrap(),
            "text/plain; charset=utf-8"
        );

        // 2. Unknown release: returns strict 404 NOT_FOUND
        let res_unknown_rel = registry.resolve_asset(&headers, "sha256:unknown", "pkg/app.js", now);
        assert_eq!(res_unknown_rel.status(), StatusCode::NOT_FOUND);

        // 3. Expiration: after retention window exceeds client_cache_lifetime
        let future_time = now + Duration::hours(25);
        let mut registry2 = BrowserAssetRegistry::new(Duration::hours(24));
        let old_release = "sha256:old_rel";
        let new_release = "sha256:new_rel";

        registry2.register_release(
            old_release,
            vec![ReleaseQualifiedAsset::new(
                "pkg/old.js",
                "application/javascript",
                b"old".to_vec(),
            )],
            now,
        );
        registry2.activate_release(old_release, now).unwrap();

        registry2.register_release(
            new_release,
            vec![ReleaseQualifiedAsset::new(
                "pkg/new.js",
                "application/javascript",
                b"new".to_vec(),
            )],
            now,
        );
        registry2.activate_release(new_release, now).unwrap();

        // At future_time (25 hours later): old_release is expired!
        let res_expired = registry2.resolve_asset(&headers, old_release, "pkg/old.js", future_time);
        assert_eq!(res_expired.status(), StatusCode::NOT_FOUND);

        // New active release is NOT expired
        let res_active = registry2.resolve_asset(&headers, new_release, "pkg/new.js", future_time);
        assert_eq!(res_active.status(), StatusCode::OK);

        // Pruning removes expired release
        let pruned = registry2.prune_expired_releases(future_time);
        assert_eq!(pruned, 1);
        assert!(!registry2.is_release_retained(old_release, future_time));
        assert!(registry2.is_release_retained(new_release, future_time));
    }
}
