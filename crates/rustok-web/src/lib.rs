use axum::{http::StatusCode, response::IntoResponse, Json};
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

pub fn json_response<T>(value: T) -> axum::response::Response
where
    T: Serialize,
{
    Json(value).into_response()
}

#[cfg(test)]
mod tests {
    use super::CspNonce;

    #[test]
    fn generated_csp_nonce_is_attribute_and_header_safe() {
        let nonce = CspNonce::generate();

        assert_eq!(nonce.as_str().len(), 32);
        assert!(nonce
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit()));
        assert_eq!(
            nonce.source_expression(),
            format!("'nonce-{}'", nonce.as_str())
        );
    }
}
