use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum TaxonomyError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Taxonomy term not found: {0}")]
    TermNotFound(Uuid),

    #[error("Canonical key already exists in this scope: {0}")]
    DuplicateCanonicalKey(String),

    #[error("Localized slug already exists in this scope: {0}")]
    DuplicateSlug(String),

    #[error("Alias already exists in this scope: {0}")]
    DuplicateAlias(String),

    #[error("Taxonomy revision conflict: {0}")]
    Conflict(String),

    #[error("Translation revision is exhausted for term {term_id} and locale {locale}")]
    TranslationRevisionExhausted { term_id: Uuid, locale: String },

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

pub type TaxonomyResult<T> = Result<T, TaxonomyError>;

impl TaxonomyError {
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden(message.into())
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }
}
