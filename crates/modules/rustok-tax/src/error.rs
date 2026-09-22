use thiserror::Error;

pub type TaxResult<T> = Result<T, TaxError>;

#[derive(Debug, Error)]
pub enum TaxError {
    #[error("validation failed: {0}")]
    Validation(String),
}
