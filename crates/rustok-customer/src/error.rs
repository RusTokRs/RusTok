use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

pub type CustomerResult<T> = Result<T, CustomerError>;

#[derive(Debug, Error)]
pub enum CustomerError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("customer {0} not found")]
    CustomerNotFound(Uuid),
    #[error("customer for user {0} not found")]
    CustomerByUserNotFound(Uuid),
    #[error("customer email already exists: {0}")]
    DuplicateEmail(String),
    #[error("customer already linked to user {0}")]
    DuplicateUserLink(Uuid),
    #[error(transparent)]
    Profile(#[from] rustok_profiles::ProfileError),
    #[error(transparent)]
    Database(#[from] DbErr),
}
