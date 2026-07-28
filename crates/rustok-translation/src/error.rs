use rustok_api::PortError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TranslationError {
    #[error("translation provider `{owner_slug}/{resource_kind}` is not registered")]
    ProviderNotFound {
        owner_slug: String,
        resource_kind: String,
    },
    #[error("translation provider does not expose a change cursor")]
    ChangeCursorUnavailable,
    #[error("translation provider returned a change outside its registered identity")]
    ProviderIdentityMismatch,
    #[error("translation provider returned changes without a checkpoint cursor")]
    MissingCheckpointCursor,
    #[error("translation provider change cursor did not advance")]
    CursorDidNotAdvance,
    #[error("invalid translation inventory request: {0}")]
    InvalidRequest(String),
    #[error("translation inventory permission denied")]
    Forbidden,
    #[error("invalid translation tenant id")]
    InvalidTenantId,
    #[error("translation inventory checkpoint changed concurrently")]
    CheckpointConflict,
    #[error("translation provider error {code}: {message}")]
    Provider {
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("translation database error: {0}")]
    Database(#[from] sea_orm::DbErr),
}

pub type TranslationResult<T> = Result<T, TranslationError>;

impl From<PortError> for TranslationError {
    fn from(error: PortError) -> Self {
        Self::Provider {
            code: error.code,
            message: error.message,
            retryable: error.retryable,
        }
    }
}
