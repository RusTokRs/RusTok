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

    #[error("Invalid media translation revision: {0}")]
    InvalidTranslationRevision(i64),

    #[error("Exact source translation `{locale}` was not found for media {media_id}")]
    TranslationSourceNotFound { media_id: Uuid, locale: String },

    #[error("Translation revision conflict for media {media_id} locale `{locale}`")]
    TranslationRevisionConflict { media_id: Uuid, locale: String },

    #[error("Resource revision conflict for media {media_id}")]
    ResourceRevisionConflict { media_id: Uuid },

    #[error("Translation revision exhausted for media {media_id} locale `{locale}`")]
    TranslationRevisionExhausted { media_id: Uuid, locale: String },

    #[error("Media translation event persistence failed: {0}")]
    TranslationEvent(String),

    #[error("Invalid rendition purpose: {0}")]
    InvalidRenditionPurpose(String),

    #[error("Invalid owner module: {0}")]
    InvalidOwnerModule(String),

    #[error("Media asset {0} cannot currently accept an owner reference")]
    AssetReferenceNotAdmissible(Uuid),

    #[error("Media asset {0} is still retained by an owner reference")]
    AssetReferenced(Uuid),

    #[error("Media asset reference identity is invalid")]
    InvalidAssetReferenceId,

    #[error("Media asset reference identity {0} is already bound to another asset or owner")]
    AssetReferenceConflict(Uuid),

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

impl From<MediaError> for rustok_core::Error {
    fn from(err: MediaError) -> Self {
        match err {
            MediaError::NotFound(_) | MediaError::TranslationSourceNotFound { .. } => {
                Self::NotFound(err.to_string())
            }
            MediaError::Forbidden => Self::Forbidden(err.to_string()),
            MediaError::TranslationRevisionConflict { .. }
            | MediaError::ResourceRevisionConflict { .. }
            | MediaError::AssetReferenceConflict(_) => Self::Validation(err.to_string()),
            MediaError::UnsupportedMimeType(_)
            | MediaError::InvalidMediaContent { .. }
            | MediaError::FileTooLarge { .. }
            | MediaError::InvalidLocale(_)
            | MediaError::InvalidTranslationRevision(_)
            | MediaError::InvalidRenditionPurpose(_)
            | MediaError::InvalidOwnerModule(_)
            | MediaError::AssetReferenceNotAdmissible(_)
            | MediaError::AssetReferenced(_)
            | MediaError::InvalidAssetReferenceId
            | MediaError::RenditionInProgress(_)
            | MediaError::UploadSessionExpired(_) => Self::Validation(err.to_string()),
            MediaError::Db(db_err) => Self::Database(db_err),
            MediaError::Json(json_err) => Self::Serialization(json_err),
            MediaError::TranslationRevisionExhausted { .. }
            | MediaError::TranslationEvent(_)
            | MediaError::PresignedUploadUnavailable
            | MediaError::ImageProcessing(_)
            | MediaError::Storage(_)
            | MediaError::StorageKey(_) => Self::Internal(err.to_string()),
        }
    }
}

pub type Result<T> = std::result::Result<T, MediaError>;

#[cfg(test)]
mod tests {
    use super::MediaError;
    use uuid::Uuid;

    #[test]
    fn test_media_error_to_core_error() {
        let not_found = MediaError::NotFound(Uuid::new_v4());
        let core_err: rustok_core::Error = not_found.into();
        assert!(matches!(core_err, rustok_core::Error::NotFound(_)));

        let forbidden = MediaError::Forbidden;
        let core_err: rustok_core::Error = forbidden.into();
        assert!(matches!(core_err, rustok_core::Error::Forbidden(_)));

        let conflict = MediaError::ResourceRevisionConflict {
            media_id: Uuid::new_v4(),
        };
        let core_err: rustok_core::Error = conflict.into();
        assert!(matches!(core_err, rustok_core::Error::Validation(_)));

        let invalid = MediaError::InvalidAssetReferenceId;
        let core_err: rustok_core::Error = invalid.into();
        assert!(matches!(core_err, rustok_core::Error::Validation(_)));
    }
}
