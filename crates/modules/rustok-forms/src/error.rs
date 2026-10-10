use rustok_core::error::{ErrorKind, RichError};
use thiserror::Error;

pub const FORM_SUBMIT_RATE_LIMITED: &str = "FORM_SUBMIT_RATE_LIMITED";
pub const FORM_SUBMIT_PAYLOAD_INVALID: &str = "FORM_SUBMIT_PAYLOAD_INVALID";
pub const FORM_SUBMISSION_NOT_FOUND: &str = "FORM_SUBMISSION_NOT_FOUND";
pub const FORM_SUBMISSION_STATE_INVALID: &str = "FORM_SUBMISSION_STATE_INVALID";

pub type FormsResult<T> = Result<T, FormsError>;

#[derive(Debug, Error)]
pub enum FormsError {
    #[error("Database operation failed")]
    Database(#[source] sea_orm::DbErr),
    #[error(transparent)]
    Rich(Box<RichError>),
}

impl FormsError {
    /// Machine-readable error code when the error carries one.
    pub fn error_code(&self) -> Option<String> {
        match self {
            Self::Rich(error) => error.error_code.clone(),
            Self::Database(_) => None,
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Rich(Box::new(RichError::new(
            ErrorKind::Validation,
            message.into(),
        )))
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::Rich(Box::new(RichError::new(
            ErrorKind::NotFound,
            message.into(),
        )))
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Rich(Box::new(RichError::new(
            ErrorKind::Forbidden,
            message.into(),
        )))
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Rich(Box::new(RichError::new(
            ErrorKind::Internal,
            message.into(),
        )))
    }
}

impl From<sea_orm::DbErr> for FormsError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Database(error)
    }
}

impl From<rustok_core::Error> for FormsError {
    fn from(error: rustok_core::Error) -> Self {
        Self::Rich(Box::new(error.into()))
    }
}

impl From<FormsError> for RichError {
    fn from(error: FormsError) -> Self {
        match error {
            FormsError::Database(source) => {
                RichError::new(ErrorKind::Database, "Database operation failed").with_source(source)
            }
            FormsError::Rich(error) => *error,
        }
    }
}

impl From<FormsError> for rustok_web::HttpError {
    fn from(error: FormsError) -> Self {
        let rich: RichError = error.into();
        rustok_web::HttpError::new(
            axum::http::StatusCode::from_u16(rich.status_code)
                .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
            rich.error_code
                .unwrap_or_else(|| rich.kind.error_code().to_string()),
            rich.message,
        )
    }
}
