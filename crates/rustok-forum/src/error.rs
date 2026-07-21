use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ForumError {
    #[error("Database error: {0}")]
    Database(sea_orm::DbErr),

    #[error("Content error: {0}")]
    Content(#[from] rustok_content::ContentError),

    #[error("Internal error: {0}")]
    Internal(#[from] rustok_core::Error),

    #[error("Category not found: {0}")]
    CategoryNotFound(Uuid),

    #[error("Topic not found: {0}")]
    TopicNotFound(Uuid),

    #[error("Reply not found: {0}")]
    ReplyNotFound(Uuid),

    #[error("Topic solution not found for topic: {0}")]
    SolutionNotFound(Uuid),

    #[error("Topic is closed")]
    TopicClosed,

    #[error("Topic is archived")]
    TopicArchived,

    #[error("Topic is locked")]
    TopicLocked,

    #[error("Topic is deleted")]
    TopicDeleted,

    #[error("Reply is deleted")]
    ReplyDeleted,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("{0}")]
    InvalidTopicTransition(#[from] crate::state_machine::InvalidTopicTransition),

    #[error("{0}")]
    InvalidReplyTransition(#[from] crate::state_machine::InvalidReplyTransition),
}

pub type ForumResult<T> = Result<T, ForumError>;

impl ForumError {
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden(message.into())
    }
}

impl From<sea_orm::DbErr> for ForumError {
    fn from(error: sea_orm::DbErr) -> Self {
        let message = error.to_string();
        if message.contains("forum category does not allow topic creation") {
            return Self::Validation(
                "Forum category does not allow topic creation".to_string(),
            );
        }
        if message.contains("active forum category cannot have archived parent")
            || message.contains("archived forum category cannot have active child")
            || message.contains("forum category lifecycle")
        {
            return Self::Validation("Forum category archive hierarchy violation".to_string());
        }
        if message.contains("Forum category icon") {
            return Self::Validation(
                "Forum category icon must be a bounded kebab-case design token".to_string(),
            );
        }
        if message.contains("Forum category color") {
            return Self::Validation(
                "Forum category color must be a safe bounded hexadecimal color".to_string(),
            );
        }
        Self::Database(error)
    }
}

impl From<rustok_taxonomy::TaxonomyError> for ForumError {
    fn from(value: rustok_taxonomy::TaxonomyError) -> Self {
        match value {
            rustok_taxonomy::TaxonomyError::Database(err) => Self::from(err),
            rustok_taxonomy::TaxonomyError::Forbidden(message) => Self::Forbidden(message),
            rustok_taxonomy::TaxonomyError::Validation(message)
            | rustok_taxonomy::TaxonomyError::DuplicateCanonicalKey(message)
            | rustok_taxonomy::TaxonomyError::DuplicateSlug(message)
            | rustok_taxonomy::TaxonomyError::DuplicateAlias(message) => Self::Validation(message),
            rustok_taxonomy::TaxonomyError::TermNotFound(term_id) => {
                Self::Validation(format!("Taxonomy term not found: {term_id}"))
            }
        }
    }
}
