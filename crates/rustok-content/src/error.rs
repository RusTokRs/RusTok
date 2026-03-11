use rustok_core::error::{ErrorKind, RichError};
use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

/// Content module errors
///
/// Uses both legacy Error enum and new RichError system.
/// Gradually migrate to RichError for better context.
#[derive(Debug, Error)]
pub enum ContentError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    #[error("Core error: {0}")]
    Core(#[from] rustok_core::Error),

    #[error("Node not found: {0}")]
    NodeNotFound(Uuid),

    #[error("Translation not found for node {node_id} and locale {locale}")]
    TranslationNotFound { node_id: Uuid, locale: String },

    #[error("Slug already exists: {slug} for locale {locale}")]
    DuplicateSlug { slug: String, locale: String },

    #[error("Concurrent modification: expected version {expected}, found {actual}")]
    ConcurrentModification { expected: i32, actual: i32 },

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Rich error: {0}")]
    Rich(Box<RichError>),
}

pub type ContentResult<T> = Result<T, ContentError>;

// Conversion from ContentError to RichError for API responses
impl From<ContentError> for RichError {
    fn from(err: ContentError) -> Self {
        match err {
            ContentError::Database(db_err) => {
                RichError::new(ErrorKind::Database, "Database operation failed")
                    .with_user_message("Unable to access content data")
                    .with_source(db_err)
            }
            ContentError::Core(core_err) => core_err.into(),
            ContentError::NodeNotFound(id) => {
                RichError::new(ErrorKind::NotFound, format!("Node {} not found", id))
                    .with_user_message("The requested content does not exist")
                    .with_field("node_id", id.to_string())
                    .with_error_code("NODE_NOT_FOUND")
            }
            ContentError::TranslationNotFound { node_id, locale } => RichError::new(
                ErrorKind::NotFound,
                format!(
                    "Translation for node {} in locale {} not found",
                    node_id, locale
                ),
            )
            .with_user_message("Translation not available for this content")
            .with_field("node_id", node_id.to_string())
            .with_field("locale", locale)
            .with_error_code("TRANSLATION_NOT_FOUND"),
            ContentError::DuplicateSlug { slug, locale } => RichError::new(
                ErrorKind::Conflict,
                format!("Slug '{}' already exists for locale '{}'", slug, locale),
            )
            .with_user_message("This URL slug is already in use. Please choose a different one.")
            .with_field("slug", slug)
            .with_field("locale", locale)
            .with_error_code("DUPLICATE_SLUG"),
            ContentError::ConcurrentModification { expected, actual } => RichError::new(
                ErrorKind::Conflict,
                format!(
                    "Concurrent modification detected: expected version {}, found {}",
                    expected, actual
                ),
            )
            .with_user_message(
                "This content was modified by another user. Please refresh and try again.",
            )
            .with_field("expected_version", expected.to_string())
            .with_field("actual_version", actual.to_string())
            .with_error_code("CONCURRENT_MODIFICATION"),
            ContentError::Forbidden(msg) => RichError::new(ErrorKind::Forbidden, msg)
                .with_user_message("You do not have permission to perform this action"),
            ContentError::Validation(msg) => {
                RichError::new(ErrorKind::Validation, msg).with_user_message("Invalid input data")
            }
            ContentError::Rich(rich) => *rich,
        }
    }
}

impl From<RichError> for ContentError {
    fn from(err: RichError) -> Self {
        ContentError::Rich(Box::new(err))
    }
}

/// Helper functions for creating common content errors
impl ContentError {
    /// Create a node not found error with rich context
    pub fn node_not_found(node_id: Uuid) -> Self {
        ContentError::NodeNotFound(node_id)
    }

    /// Create a translation not found error with rich context
    pub fn translation_not_found(node_id: Uuid, locale: impl Into<String>) -> Self {
        ContentError::TranslationNotFound {
            node_id,
            locale: locale.into(),
        }
    }

    /// Create a validation error
    pub fn validation(message: impl Into<String>) -> Self {
        ContentError::Validation(message.into())
    }

    /// Create a forbidden error
    pub fn forbidden(message: impl Into<String>) -> Self {
        ContentError::Forbidden(message.into())
    }

    /// Create a duplicate slug error
    pub fn duplicate_slug(slug: impl Into<String>, locale: impl Into<String>) -> Self {
        ContentError::DuplicateSlug {
            slug: slug.into(),
            locale: locale.into(),
        }
    }

    /// Create a concurrent modification error
    pub fn concurrent_modification(expected: i32, actual: i32) -> Self {
        ContentError::ConcurrentModification { expected, actual }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_not_found_conversion() {
        let id = Uuid::new_v4();
        let err = ContentError::node_not_found(id);
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::NotFound);
        assert_eq!(rich.status_code, 404);
        assert!(rich.fields.contains_key("node_id"));
    }

    #[test]
    fn test_translation_not_found_conversion() {
        let id = Uuid::new_v4();
        let err = ContentError::translation_not_found(id, "en");
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::NotFound);
        assert_eq!(rich.fields.get("locale"), Some(&"en".to_string()));
    }

    #[test]
    fn test_duplicate_slug_conversion() {
        let err = ContentError::duplicate_slug("my-slug", "ru");
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::Conflict);
        assert_eq!(rich.error_code.as_deref(), Some("DUPLICATE_SLUG"));
        assert_eq!(rich.fields.get("slug"), Some(&"my-slug".to_string()));
        assert_eq!(rich.fields.get("locale"), Some(&"ru".to_string()));
    }

    #[test]
    fn test_concurrent_modification_conversion() {
        let err = ContentError::concurrent_modification(3, 2);
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::Conflict);
        assert_eq!(rich.error_code.as_deref(), Some("CONCURRENT_MODIFICATION"));
        assert_eq!(rich.fields.get("expected_version"), Some(&"3".to_string()));
        assert_eq!(rich.fields.get("actual_version"), Some(&"2".to_string()));
    }
}
