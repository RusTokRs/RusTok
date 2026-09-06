use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

pub type RegionResult<T> = Result<T, RegionError>;

#[derive(Debug, Error)]
pub enum RegionError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("region {0} not found")]
    RegionNotFound(Uuid),
    #[error("country code `{0}` is invalid")]
    InvalidCountryCode(String),
    #[error(transparent)]
    Database(#[from] DbErr),
}
