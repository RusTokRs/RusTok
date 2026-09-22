use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("Bundle with ID '{0}' not found")]
    NotFound(Uuid),

    #[error("Bundle with slug '{0}' not found")]
    SlugNotFound(String),

    #[error("Bundle with slug '{0}' already exists")]
    SlugAlreadyExists(String),

    #[error("Bundle item with ID '{0}' not found")]
    ItemNotFound(Uuid),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    #[error("Outbox error: {0}")]
    Outbox(String),
}

pub type BundleResult<T> = Result<T, BundleError>;
