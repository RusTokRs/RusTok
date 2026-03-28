use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ForumError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

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

    #[error("Topic is closed")]
    TopicClosed,

    #[error("Topic is archived")]
    TopicArchived,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("{0}")]
    InvalidTopicTransition(#[from] crate::state_machine::InvalidTopicTransition),

    #[error("{0}")]
    InvalidReplyTransition(#[from] crate::state_machine::InvalidReplyTransition),
}

pub type ForumResult<T> = Result<T, ForumError>;
