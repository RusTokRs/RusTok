use sea_orm::DbErr;
use thiserror::Error;

pub type SeoResult<T> = Result<T, SeoError>;

#[derive(Debug, Error)]
pub enum SeoError {
    #[error("{0}")]
    Validation(String),
    #[error("SEO target not found")]
    NotFound,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
}

impl SeoError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
}
