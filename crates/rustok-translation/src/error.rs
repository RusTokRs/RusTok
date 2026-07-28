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
    #[error("translation provider does not expose bounded resource listing")]
    FullRescanUnavailable,
    #[error("translation full-rescan change drain did not converge")]
    FullRescanChangeDrainLimit,
    #[error("translation full-rescan cursor did not advance")]
    FullRescanCursorDidNotAdvance,
    #[error("translation full-rescan provider page exceeded its requested bound")]
    FullRescanPageOverflow,
    #[error("translation full-rescan exceeded the resource safety bound")]
    FullRescanResourceLimit,
    #[error("invalid translation inventory request: {0}")]
    InvalidRequest(String),
    #[error("translation workflow idempotency key was reused with another request")]
    IdempotencyConflict,
    #[error("translation workflow revision changed concurrently")]
    WorkflowRevisionConflict,
    #[error("translation job was not found")]
    JobNotFound,
    #[error("translation job does not accept new items in state `{0}`")]
    JobNotWritable(String),
    #[error("translation job item was not found")]
    ItemNotFound,
    #[error("translation job item does not accept this transition in state `{0}`")]
    ItemNotWritable(String),
    #[error("translation proposal was not found")]
    ProposalNotFound,
    #[error("translation proposal is not the current item proposal")]
    ProposalNotCurrent,
    #[error("translation proposal failed owner validation")]
    ProposalValidationFailed,
    #[error("translation proposal creator cannot approve their own proposal")]
    ReviewerSeparationRequired,
    #[error("translation workflow serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
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
