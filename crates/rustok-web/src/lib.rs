use std::fmt::Write as _;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

impl ErrorBody {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{status}: {code}: {message}")]
pub struct HttpError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
}

impl HttpError {
    pub fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
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

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
    }

    pub fn body(&self) -> ErrorBody {
        ErrorBody::new(self.code.clone(), self.message.clone())
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(self.body())).into_response()
    }
}

pub type HttpResult<T> = Result<T, HttpError>;

/// Build a same-origin immutable-input asset response with content-derived revalidation.
pub fn embedded_asset_response(
    headers: &HeaderMap,
    bytes: &'static [u8],
    content_type: &'static str,
    cache_control: &'static str,
    context: &'static str,
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
            .unwrap_or_else(|error| panic!("{context} headers are invalid: {error}"));
    }
    builder
        .header(CONTENT_TYPE, content_type)
        .status(StatusCode::OK)
        .body(Body::from(bytes))
        .unwrap_or_else(|error| panic!("{context} headers are invalid: {error}"))
}

fn content_etag(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2 + 2);
    encoded.push('"');
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
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
/// Retryable infrastructure failures intentionally receive stable public messages instead of
/// exposing storage or connector details carried by the internal port error.
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

    HttpError::new(status, error.code, message)
}

pub fn json_response<T>(value: T) -> axum::response::Response
where
    T: Serialize,
{
    Json(value).into_response()
}

#[cfg(test)]
mod tests {
    use super::{CspNonce, embedded_asset_response, port_error_to_http_error};
    use axum::http::{HeaderMap, HeaderValue, StatusCode, header::IF_NONE_MATCH};
    use rustok_api::PortError;

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
}
