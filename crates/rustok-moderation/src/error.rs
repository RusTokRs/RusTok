use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ModerationError {
    #[error("moderation report {0} not found")]
    ReportNotFound(Uuid),
    #[error("moderation case {0} not found")]
    CaseNotFound(Uuid),
    #[error("moderation decision {0} not found")]
    DecisionNotFound(Uuid),
    #[error("moderation application operation for decision {0} not found")]
    ApplicationOperationNotFound(Uuid),
    #[error("moderation application lease conflict for decision {0}")]
    ApplicationLeaseConflict(Uuid),
    #[error("moderation application recovery conflict for decision {0}")]
    ApplicationRecoveryConflict(Uuid),
    #[error("moderation application evidence does not match decision {0}")]
    ApplicationEvidenceMismatch(Uuid),
    #[error("moderation command validation failed: {0}")]
    Validation(String),
    #[error("moderation case revision conflict")]
    RevisionConflict,
    #[error("moderation case lifecycle conflict from `{from}` to `{to}`")]
    LifecycleConflict { from: String, to: String },
    #[error("moderation idempotency key is already bound to another command")]
    IdempotencyConflict,
    #[error("moderation command receipt requires operator review")]
    CommandReceiptCorrupt,
    #[error("moderation invariant failed: {0}")]
    Invariant(String),
    #[error("moderation storage unavailable: {0}")]
    Database(#[from] sea_orm::DbErr),
}

pub type ModerationResult<T> = Result<T, ModerationError>;
