use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum BrandError {
    #[error("Brand with ID '{0}' not found")]
    NotFound(Uuid),

    #[error("Brand with slug '{0}' not found")]
    SlugNotFound(String),

    #[error("Brand with slug '{0}' already exists")]
    SlugAlreadyExists(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    #[error("Outbox error: {0}")]
    Outbox(String),
}

pub type BrandResult<T> = Result<T, BrandError>;
