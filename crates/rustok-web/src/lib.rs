use axum::{Json, http::StatusCode, response::IntoResponse};
use rustok_api::{PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
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
    use super::{CspNonce, port_error_to_http_error};
    use axum::http::StatusCode;
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
}
