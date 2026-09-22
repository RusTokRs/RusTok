use thiserror::Error;
use uuid::Uuid;

pub type MarketplaceAllocationResult<T> = Result<T, MarketplaceAllocationError>;

#[derive(Debug, Error)]
pub enum MarketplaceAllocationError {
    #[error("marketplace allocation for order line {0} was not found")]
    AllocationNotFound(Uuid),
    #[error("order line {0} is already allocated")]
    LineAlreadyAllocated(Uuid),
    #[error("order line {0} appears more than once in the allocation command")]
    DuplicateLine(Uuid),
    #[error("marketplace allocation idempotency key is already bound to another request")]
    IdempotencyConflict,
    #[error("marketplace allocation receipt is incomplete or corrupt")]
    ReceiptCorrupt,
    #[error("marketplace allocation validation failed: {0}")]
    Validation(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}
