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
    #[error("translation workflow note was not found")]
    WorkflowNoteNotFound,
    #[error("translation interchange artifact was not found")]
    InterchangeArtifactNotFound,
    #[error("translation interchange artifact has expired")]
    InterchangeArtifactExpired,
    #[error("translation interchange artifact is not ready")]
    InterchangeArtifactNotReady,
    #[error("translation interchange artifact import is currently being processed")]
    InterchangeArtifactInProgress,
    #[error("translation interchange artifact was already processed")]
    InterchangeArtifactAlreadyProcessed,
    #[error("translation job item does not accept this transition in state `{0}`")]
    ItemNotWritable(String),
    #[error("translation proposal was not found")]
    ProposalNotFound,
    #[error("translation proposal is not the current item proposal")]
    ProposalNotCurrent,
    #[error("translation proposal failed owner validation")]
    ProposalValidationFailed,
    #[error("machine translation returned invalid execution evidence or output")]
    InvalidMachineTranslationResult,
    #[error("machine translation operation was not found")]
    MachineOperationNotFound,
    #[error("machine translation operation was cancelled")]
    MachineOperationCancelled,
    #[error("machine translation operation is terminal in state `{0}`")]
    MachineOperationTerminal(String),
    #[error("machine translation memory projection is unavailable")]
    MachineMemoryProjectionUnavailable,
    #[error("invalid machine translation cancellation reason")]
    InvalidMachineCancellationReason,
    #[error("invalid machine translation recovery reason")]
    InvalidMachineRecoveryReason,
    #[error("machine translation recovery observed a different operation revision")]
    MachineRecoveryRevisionMismatch,
    #[error("machine translation recovery was already requested")]
    MachineRecoveryAlreadyRequested,
    #[error("machine translation provider has no completed result for recovery")]
    MachineRecoveryResultUnavailable,
    #[error("translation owner returned invalid patch-validation evidence: {0}")]
    InvalidProviderValidation(String),
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
    #[error("translation policy revision conflict: expected {expected}, actual {actual}")]
    TranslationPolicyConflict { expected: i64, actual: i64 },
    #[error("translation policy invariant failed: {0}")]
    TranslationPolicyInvariant(String),
    #[error("translation policy is stale against tenant locale policy: {0}")]
    TranslationPolicyStale(String),
    #[error("translation required target locale `{0}` is not enabled for the tenant")]
    RequiredTargetLocaleDisabled(String),
    #[error("translation required target locale is duplicated")]
    DuplicateRequiredTargetLocale,
    #[error("translation job {role} locale `{locale}` is not enabled for the tenant")]
    DisabledJobLocale { role: &'static str, locale: String },
    #[error("translation glossary was not found")]
    GlossaryNotFound,
    #[error("translation glossary name is already in use")]
    GlossaryNameConflict,
    #[error("translation glossary revision conflict: expected {expected}, actual {actual}")]
    GlossaryRevisionConflict { expected: i64, actual: i64 },
    #[error(
        "translation glossary revision {requested} is unavailable; current revision is {current}"
    )]
    GlossaryRevisionUnavailable { requested: i64, current: i64 },
    #[error("translation glossary is inactive")]
    GlossaryInactive,
    #[error("translation glossary active state is unchanged")]
    GlossaryActiveStateUnchanged,
    #[error("translation glossary locale pair does not match the job")]
    GlossaryLocaleMismatch,
    #[error("translation glossary term conflict: {0}")]
    GlossaryTermConflict(String),
    #[error("translation glossary invariant failed: {0}")]
    GlossaryInvariant(String),
    #[error("translation memory entry was not found")]
    MemoryEntryNotFound,
    #[error("translation memory revision conflict: expected {expected}, actual {actual}")]
    MemoryRevisionConflict { expected: i64, actual: i64 },
    #[error("translation memory lifecycle does not allow this operation: {0}")]
    MemoryLifecycleConflict(String),
    #[error("translation memory retention policy is invalid: {0}")]
    MemoryRetentionConflict(String),
    #[error("translation memory invariant failed: {0}")]
    MemoryInvariant(String),
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
