use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("Media not found: {0}")]
    NotFound(Uuid),

    #[error("Access denied")]
    Forbidden,

    #[error("Unsupported media type: {0}")]
    UnsupportedMimeType(String),

    #[error("Media content does not match declared type `{declared}`: {reason}")]
    InvalidMediaContent { declared: String, reason: String },

    #[error("File too large: {size} bytes (max {max} bytes)")]
    FileTooLarge { size: u64, max: u64 },

    #[error("Invalid locale: {0}")]
    InvalidLocale(String),

    #[error("Storage error: {0}")]
    Storage(#[from] rustok_storage::StorageError),

    #[error("Database error: {0}")]
    Db(#[from] sea_orm::DbErr),
}

pub type Result<T> = std::result::Result<T, MediaError>;