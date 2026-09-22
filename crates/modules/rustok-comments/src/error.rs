use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum CommentsError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Event publication failed: {0}")]
    EventPublication(String),

    #[error("Comment not found: {0}")]
    CommentNotFound(Uuid),

    #[error("Comment thread not found for target {target_type}:{target_id}")]
    CommentThreadNotFound {
        target_type: String,
        target_id: Uuid,
    },

    #[error("Comment thread is closed for target {target_type}:{target_id}")]
    CommentThreadClosed {
        target_type: String,
        target_id: Uuid,
    },

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

pub type CommentsResult<T> = Result<T, CommentsError>;

impl CommentsError {
    pub fn kind(&self) -> &'static str {
        match self {
            CommentsError::Database(_) => "database",
            CommentsError::EventPublication(_) => "unavailable",
            CommentsError::CommentNotFound(_) => "not_found",
            CommentsError::CommentThreadNotFound { .. } => "not_found",
            CommentsError::CommentThreadClosed { .. } => "conflict",
            CommentsError::Forbidden(_) => "forbidden",
            CommentsError::Validation(_) => "validation",
        }
    }

    pub fn severity(&self) -> &'static str {
        match self {
            CommentsError::Database(_) => "error",
            CommentsError::EventPublication(_) => "error",
            CommentsError::CommentNotFound(_) => "warning",
            CommentsError::CommentThreadNotFound { .. } => "warning",
            CommentsError::CommentThreadClosed { .. } => "warning",
            CommentsError::Forbidden(_) => "warning",
            CommentsError::Validation(_) => "warning",
        }
    }
}
