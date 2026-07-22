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

    #[error("Invalid rendition purpose: {0}")]
    InvalidRenditionPurpose(String),

    #[error("Rendition is already being processed: {0}")]
    RenditionInProgress(Uuid),

    #[error("Upload session has expired: {0}")]
    UploadSessionExpired(Uuid),

    #[error("Presigned upload is unavailable for the configured storage backend")]
    PresignedUploadUnavailable,

    #[error("Image processing error: {0}")]
    ImageProcessing(#[from] crate::image::ImageProcessingError),

    #[error("Media JSON encoding error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Storage error: {0}")]
    Storage(#[from] object_store::Error),

    #[error("Invalid media object key: {0}")]
    StorageKey(#[from] rustok_storage::KeyError),

    #[error("Database error: {0}")]
    Db(#[from] sea_orm::DbErr),
}

pub type Result<T> = std::result::Result<T, MediaError>;
