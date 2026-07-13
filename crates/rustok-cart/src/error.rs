use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

pub type CartResult<T> = Result<T, CartError>;

#[derive(Debug, Error)]
pub enum CartError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("cart {0} not found")]
    CartNotFound(Uuid),
    #[error("cart line item {0} not found")]
    CartLineItemNotFound(Uuid),
    #[error("invalid cart status transition: {from} -> {to}")]
    InvalidTransition { from: String, to: String },
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error("tax boundary failed with `{code}` ({kind:?}, retryable={retryable}): {message}")]
    TaxBoundary {
        kind: rustok_api::PortErrorKind,
        code: String,
        message: String,
        retryable: bool,
    },
}
