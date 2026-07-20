use thiserror::Error;
use uuid::Uuid;

pub type MarketplaceLedgerResult<T> = Result<T, MarketplaceLedgerError>;

#[derive(Debug, Error)]
pub enum MarketplaceLedgerError {
    #[error("marketplace ledger transaction for order {0} was not found")]
    TransactionNotFound(Uuid),
    #[error("marketplace ledger transaction for order {0} is already posted")]
    OrderAlreadyPosted(Uuid),
    #[error("commission assessment {0} is already posted")]
    AssessmentAlreadyPosted(Uuid),
    #[error("marketplace ledger idempotency key is already bound to another request")]
    IdempotencyConflict,
    #[error("marketplace ledger receipt is incomplete or corrupt")]
    ReceiptCorrupt,
    #[error("marketplace ledger validation failed: {0}")]
    Validation(String),
    #[error("commission boundary `{code}` failed: {message}")]
    CommissionBoundary {
        code: String,
        message: String,
        retryable: bool,
    },
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}
