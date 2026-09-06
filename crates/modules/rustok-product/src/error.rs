use rustok_core::error::Error as CoreError;
use thiserror::Error;
use uuid::Uuid;

/// Errors owned by the Product domain.
#[derive(Error, Debug)]
pub enum CommerceError {
    #[error("Product storage error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Product not found: {0}")]
    ProductNotFound(Uuid),

    #[error("Duplicate handle: {handle} already exists for locale {locale}")]
    DuplicateHandle { handle: String, locale: String },

    #[error("Duplicate SKU: {0}")]
    DuplicateSku(String),

    #[error("Product validation error: {0}")]
    Validation(String),

    #[error("Product must have at least one variant")]
    NoVariants,

    #[error("Cannot delete published product")]
    CannotDeletePublished,

    #[error("Product core operation failed: {0}")]
    Core(#[from] CoreError),
}

pub type CommerceResult<T> = Result<T, CommerceError>;
