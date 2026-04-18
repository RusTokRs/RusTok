use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

pub type PaymentResult<T> = Result<T, PaymentError>;

#[derive(Debug, Error)]
pub enum PaymentError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("payment collection {0} not found")]
    PaymentCollectionNotFound(Uuid),
    #[error("payment for collection {0} not found")]
    PaymentNotFound(Uuid),
    #[error("refund {0} not found")]
    RefundNotFound(Uuid),
    #[error("invalid payment transition from `{from}` to `{to}`")]
    InvalidTransition { from: String, to: String },
    #[error(transparent)]
    Database(#[from] DbErr),
}
