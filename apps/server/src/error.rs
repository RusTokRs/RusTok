//! Server-owned error boundary for Axum handlers and host services.

use axum::{
    Json,
    http::{StatusCode, header::InvalidHeaderValue},
    response::{IntoResponse, Response},
};
use thiserror::Error;

use rustok_web::{ErrorBody, HttpError};

/// Error type shared by server services and Axum transport handlers.
///
/// Domain modules expose their own typed errors. This type only maps host
/// orchestration and transport failures to the stable HTTP error envelope.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    Template(#[from] tera::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    #[error(transparent)]
    Http(#[from] HttpError),
    #[error(transparent)]
    Core(#[from] rustok_core::Error),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    BadRequest(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("cache operation failed: {0}")]
    Cache(String),
    #[error("internal server error")]
    InternalServerError,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Self::Http(error) => error.into_response(),
            Self::Unauthorized(message) => {
                error_response(StatusCode::UNAUTHORIZED, "unauthorized", message)
            }
            Self::Forbidden(message) => error_response(StatusCode::FORBIDDEN, "forbidden", message),
            Self::NotFound => error_response(StatusCode::NOT_FOUND, "not_found", "Not found"),
            Self::BadRequest(message) => {
                error_response(StatusCode::BAD_REQUEST, "bad_request", message)
            }
            Self::Validation(message) => {
                error_response(StatusCode::BAD_REQUEST, "validation_error", message)
            }
            Self::Conflict(message) => {
                error_response(StatusCode::CONFLICT, "conflict", message)
            }
            Self::InvalidHeaderValue(error) => {
                error_response(StatusCode::BAD_REQUEST, "invalid_header", error.to_string())
            }
            Self::Core(core_err) => match core_err {
                rustok_core::Error::NotFound(msg) => {
                    error_response(StatusCode::NOT_FOUND, "not_found", msg)
                }
                rustok_core::Error::Forbidden(msg) => {
                    error_response(StatusCode::FORBIDDEN, "forbidden", msg)
                }
                rustok_core::Error::Validation(msg) => {
                    error_response(StatusCode::BAD_REQUEST, "validation_error", msg)
                }
                rustok_core::Error::InvalidIdFormat(msg) => {
                    error_response(StatusCode::BAD_REQUEST, "invalid_id_format", msg)
                }
                rustok_core::Error::Auth(msg) => {
                    error_response(StatusCode::UNAUTHORIZED, "unauthorized", msg)
                }
                other => {
                    tracing::error!(error = %other, "server core operation failed");
                    error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal_error",
                        "Internal server error",
                    )
                }
            },
            Self::Message(error) => {
                tracing::error!(error = %error, "server host operation failed");
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Internal server error",
                )
            }
            error => {
                tracing::error!(error = %error, "server request failed");
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Internal server error",
                )
            }
        }
    }
}

pub fn http_error(error: HttpError) -> Error {
    error.into()
}

fn error_response(
    status: StatusCode,
    code: impl Into<String>,
    message: impl Into<String>,
) -> Response {
    (status, Json(ErrorBody::new(code, message))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn parse_error_response(response: Response) -> (StatusCode, ErrorBody) {
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 64)
            .await
            .expect("read body");
        let body: ErrorBody = serde_json::from_slice(&bytes).expect("parse json");
        (status, body)
    }

    #[tokio::test]
    async fn error_into_response_maps_unauthorized() {
        let err = Error::Unauthorized("token expired".into());
        let (status, body) = parse_error_response(err.into_response()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body.code, "unauthorized");
        assert_eq!(body.message, "token expired");
    }

    #[tokio::test]
    async fn error_into_response_maps_forbidden() {
        let err = Error::Forbidden("access denied".into());
        let (status, body) = parse_error_response(err.into_response()).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body.code, "forbidden");
        assert_eq!(body.message, "access denied");
    }

    #[tokio::test]
    async fn error_into_response_maps_not_found() {
        let err = Error::NotFound;
        let (status, body) = parse_error_response(err.into_response()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.code, "not_found");
        assert_eq!(body.message, "Not found");
    }

    #[tokio::test]
    async fn error_into_response_maps_bad_request() {
        let err = Error::BadRequest("invalid query param".into());
        let (status, body) = parse_error_response(err.into_response()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.code, "bad_request");
        assert_eq!(body.message, "invalid query param");
    }

    #[tokio::test]
    async fn error_into_response_maps_validation() {
        let err = Error::Validation("slug cannot be empty".into());
        let (status, body) = parse_error_response(err.into_response()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.code, "validation_error");
        assert_eq!(body.message, "slug cannot be empty");
    }

    #[tokio::test]
    async fn error_into_response_maps_conflict() {
        let err = Error::Conflict("slug already exists".into());
        let (status, body) = parse_error_response(err.into_response()).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body.code, "conflict");
        assert_eq!(body.message, "slug already exists");
    }

    #[tokio::test]
    async fn error_into_response_maps_invalid_header_value() {
        let invalid = axum::http::HeaderValue::from_str("invalid\nvalue").unwrap_err();
        let err = Error::InvalidHeaderValue(invalid);
        let (status, body) = parse_error_response(err.into_response()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.code, "invalid_header");
    }

    #[tokio::test]
    async fn error_into_response_sanitizes_internal_errors_without_leakage() {
        let err = Error::Message("sensitive secret connection string postgres://admin:pass@host/db".into());
        let (status, body) = parse_error_response(err.into_response()).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.code, "internal_error");
        assert_eq!(body.message, "Internal server error");
        assert!(!body.message.contains("pass"));
        assert!(!body.message.contains("postgres"));
    }

    #[tokio::test]
    async fn error_into_response_sanitizes_io_errors_without_path_leakage() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "/etc/secret/token.key not found");
        let err = Error::Io(io_err);
        let (status, body) = parse_error_response(err.into_response()).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.code, "internal_error");
        assert_eq!(body.message, "Internal server error");
        assert!(!body.message.contains("/etc/secret"));
    }
}
