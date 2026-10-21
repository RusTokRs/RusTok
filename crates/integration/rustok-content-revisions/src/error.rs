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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ContentRevisionError::TenantMismatch;
        assert_eq!(err.to_string(), "Tenant mismatch");

        let err = ContentRevisionError::PermissionDenied;
        assert_eq!(err.to_string(), "Permission denied");

        let err = ContentRevisionError::ContentTypeNotConfigured("custom".to_string());
        assert!(err.to_string().contains("custom"));

        let db_err = sea_orm::DbErr::Custom("db error".to_string());
        let converted: ContentRevisionError = db_err.into();
        assert!(converted.to_string().contains("db error"));
    }
}

