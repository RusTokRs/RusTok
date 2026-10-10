//! Error types for revision operations.

use thiserror::Error;

/// Errors that can occur during revision operations.
#[derive(Debug, Error)]
pub enum RevisionError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Revision not found: {0}")]
    NotFound(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Revision already exists: {0}")]
    AlreadyExists(String),

    #[error("Retention policy violation: {0}")]
    RetentionPolicyViolation(String),

    #[error("Diff computation failed: {0}")]
    DiffError(String),

    #[error("Restore failed: {0}")]
    RestoreFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

impl From<serde_json::Error> for RevisionError {
    fn from(err: serde_json::Error) -> Self {
        RevisionError::Serialization(err.to_string())
    }
}

#[cfg(feature = "seaorm")]
impl From<sea_orm::DbErr> for RevisionError {
    fn from(err: sea_orm::DbErr) -> Self {
        RevisionError::Database(err.to_string())
    }
}

pub type RevisionResult<T> = Result<T, RevisionError>;
