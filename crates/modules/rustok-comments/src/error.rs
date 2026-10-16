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

impl From<CommentsError> for rustok_core::Error {
    fn from(error: CommentsError) -> Self {
        match error {
            CommentsError::Database(err) => rustok_core::Error::Database(err),
            CommentsError::EventPublication(msg) => rustok_core::Error::Internal(msg),
            CommentsError::CommentNotFound(id) => {
                rustok_core::Error::NotFound(format!("Comment {id} not found"))
            }
            CommentsError::CommentThreadNotFound {
                target_type,
                target_id,
            } => rustok_core::Error::NotFound(format!(
                "Comment thread not found for target {target_type}:{target_id}"
            )),
            CommentsError::CommentThreadClosed {
                target_type,
                target_id,
            } => rustok_core::Error::Validation(format!(
                "Comment thread is closed for target {target_type}:{target_id}"
            )),
            CommentsError::Forbidden(msg) => rustok_core::Error::Forbidden(msg),
            CommentsError::Validation(msg) => rustok_core::Error::Validation(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_error_converts_to_core_error() {
        let validation = CommentsError::Validation("invalid comment".to_string());
        let core_err: rustok_core::Error = validation.into();
        assert!(matches!(core_err, rustok_core::Error::Validation(_)));

        let not_found = CommentsError::CommentNotFound(Uuid::new_v4());
        let core_err: rustok_core::Error = not_found.into();
        assert!(matches!(core_err, rustok_core::Error::NotFound(_)));

        let thread_not_found = CommentsError::CommentThreadNotFound {
            target_type: "post".to_string(),
            target_id: Uuid::new_v4(),
        };
        let core_err: rustok_core::Error = thread_not_found.into();
        assert!(matches!(core_err, rustok_core::Error::NotFound(_)));

        let conflict = CommentsError::CommentThreadClosed {
            target_type: "post".to_string(),
            target_id: Uuid::new_v4(),
        };
        let core_err: rustok_core::Error = conflict.into();
        assert!(matches!(core_err, rustok_core::Error::Validation(_)));

        let forbidden = CommentsError::Forbidden("access denied".to_string());
        let core_err: rustok_core::Error = forbidden.into();
        assert!(matches!(core_err, rustok_core::Error::Forbidden(_)));

        let event_pub = CommentsError::EventPublication("bus failure".to_string());
        let core_err: rustok_core::Error = event_pub.into();
        assert!(matches!(core_err, rustok_core::Error::Internal(_)));
    }
}
