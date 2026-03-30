use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

pub type ProfileResult<T> = Result<T, ProfileError>;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("profile display name must not be empty")]
    EmptyDisplayName,
    #[error("profile display name is too long")]
    DisplayNameTooLong,
    #[error("profile handle must not be empty")]
    EmptyHandle,
    #[error("profile handle contains invalid characters")]
    InvalidHandle,
    #[error("profile handle is too short")]
    HandleTooShort,
    #[error("profile handle is too long")]
    HandleTooLong,
    #[error("profile handle is reserved: {0}")]
    ReservedHandle(String),
    #[error("profile locale is invalid: {0}")]
    InvalidLocale(String),
    #[error("profile for user {0} not found")]
    ProfileNotFound(Uuid),
    #[error("profile for handle {0} not found")]
    ProfileByHandleNotFound(String),
    #[error("profile handle already exists: {0}")]
    DuplicateHandle(String),
    #[error("profile validation failed: {0}")]
    Validation(String),
    #[error(transparent)]
    Database(#[from] DbErr),
}

impl From<rustok_taxonomy::TaxonomyError> for ProfileError {
    fn from(value: rustok_taxonomy::TaxonomyError) -> Self {
        match value {
            rustok_taxonomy::TaxonomyError::Database(err) => Self::Database(err),
            rustok_taxonomy::TaxonomyError::Validation(message)
            | rustok_taxonomy::TaxonomyError::DuplicateCanonicalKey(message)
            | rustok_taxonomy::TaxonomyError::DuplicateSlug(message)
            | rustok_taxonomy::TaxonomyError::DuplicateAlias(message)
            | rustok_taxonomy::TaxonomyError::Forbidden(message) => Self::Validation(message),
            rustok_taxonomy::TaxonomyError::TermNotFound(term_id) => {
                Self::Validation(format!("taxonomy term not found: {term_id}"))
            }
        }
    }
}
