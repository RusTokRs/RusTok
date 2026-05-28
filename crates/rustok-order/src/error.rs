use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

pub type OrderResult<T> = Result<T, OrderError>;

#[derive(Debug, Error)]
pub enum OrderError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("order {0} not found")]
    OrderNotFound(Uuid),
    #[error("order return {0} not found")]
    OrderReturnNotFound(Uuid),
    #[error("invalid order status transition: {from} -> {to}")]
    InvalidTransition { from: String, to: String },
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    Core(#[from] rustok_core::Error),
}
