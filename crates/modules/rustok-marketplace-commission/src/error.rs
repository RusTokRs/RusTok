use thiserror::Error;
use uuid::Uuid;

pub type MarketplaceCommissionResult<T> = Result<T, MarketplaceCommissionError>;

#[derive(Debug, Error)]
pub enum MarketplaceCommissionError {
    #[error("commission rule {0} was not found")]
    RuleNotFound(Uuid),
    #[error("commission assessment for allocation {0} was not found")]
    AssessmentNotFound(Uuid),
    #[error("no active commission rule matches allocation {0}")]
    RuleNotMatched(Uuid),
    #[error("allocation {0} is already assessed")]
    AllocationAlreadyAssessed(Uuid),
    #[error("commission idempotency key is already bound to another request")]
    IdempotencyConflict,
    #[error("commission receipt is incomplete or corrupt")]
    ReceiptCorrupt,
    #[error("commission validation failed: {0}")]
    Validation(String),
    #[error("allocation boundary `{code}` failed: {message}")]
    AllocationBoundary {
        code: String,
        message: String,
        retryable: bool,
    },
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}
