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
    #[error("{0}")]
    Unauthorized(String),
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    BadRequest(String),
    #[error("validation failed: {0}")]
    Validation(String),
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
            Self::NotFound => error_response(StatusCode::NOT_FOUND, "not_found", "Not found"),
            Self::BadRequest(message) | Self::Validation(message) => {
                error_response(StatusCode::BAD_REQUEST, "bad_request", message)
            }
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
