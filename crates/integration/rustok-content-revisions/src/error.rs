use thiserror::Error;

/// Errors specific to content revision integration.
#[derive(Debug, Error)]
pub enum ContentRevisionError {
    #[error("Revision error: {0}")]
    Revision(#[from] rustok_revisions::RevisionError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Content type not configured: {0}")]
    ContentTypeNotConfigured(String),

    #[error("Revision tracking disabled for content type: {0}")]
    TrackingDisabled(String),

    #[error("Tenant mismatch")]
    TenantMismatch,

    #[error("Permission denied")]
    PermissionDenied,
}

impl From<sea_orm::DbErr> for ContentRevisionError {
    fn from(err: sea_orm::DbErr) -> Self {
        ContentRevisionError::Database(err.to_string())
    }
}
