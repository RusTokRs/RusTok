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
    #[error("translation provider does not expose aggregate progress")]
    AggregateProgressUnavailable,
    #[error("translation provider returned invalid aggregate progress: {0}")]
    InvalidProviderProgress(String),
    #[error("translation provider checkpoint is invalid: {0}")]
    InvalidProviderCheckpoint(String),
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
    #[error("translation workflow idempotency key belongs to another actor")]
    IdempotencyActorMismatch,
    #[error("translation apply operation ended in state `{status}` with owner error `{code}`")]
    ApplyOperationTerminal { status: String, code: String },
    #[error("translation owner returned an invalid application receipt: {0}")]
    InvalidProviderReceipt(String),
    #[error("translation owner returned a different receipt for the same apply operation")]
    ProviderReceiptMismatch,
    #[error("translation apply operation is currently leased by another executor")]
    ApplyInProgress,
    #[error("translation apply recovery observed a different attempt count")]
    ApplyRecoveryAttemptMismatch,
    #[error("invalid translation apply recovery reason")]
    InvalidRecoveryReason,
    #[error("translation item assignment is unchanged")]
    AssignmentUnchanged,
    #[error("translation job item is assigned to another actor")]
    ItemAssignedToAnotherActor,
    #[error("translation job cannot be cancelled while an owner apply is in progress")]
    JobCancellationInProgress,
    #[error("translation job does not accept cancellation in state `{0}`")]
    JobNotCancellable(String),
    #[error("invalid translation workflow actor")]
    InvalidWorkflowActor,
    #[error("invalid translation job cancellation reason")]
    InvalidCancellationReason,
    #[error("translation item does not accept retry in state `{0}`")]
    ItemNotRetryable(String),
    #[error("translation item retry requires a current approved proposal")]
    RetryProposalNotApproved,
    #[error("invalid translation item retry reason")]
    InvalidRetryReason,
    #[error("translation job progress was not found")]
    JobProgressNotFound,
    #[error("translation job progress source is invalid: {0}")]
    InvalidProgressSource(String),
    #[error("translation job progress count overflow")]
    ProgressOverflow,
    #[error("translation job progress changed concurrently")]
    ProgressRevisionConflict,
    #[error("translation workflow event error: {0}")]
    Event(#[from] rustok_core::Error),
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
