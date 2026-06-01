use rustok_core::error::{Error as CoreError, ErrorKind, RichError};
use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

/// Pages module errors
///
/// Uses both legacy Error enum and new RichError system.
/// Gradually migrate to RichError for better context.
#[derive(Debug, Error)]
pub enum PagesError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    #[error("Core error: {0}")]
    Core(#[from] CoreError),

    #[error("Page not found: {0}")]
    PageNotFound(Uuid),

    #[error("Block not found: {0}")]
    BlockNotFound(Uuid),

    #[error("Menu not found: {0}")]
    MenuNotFound(Uuid),

    #[error("Duplicate slug: {slug} already exists for locale {locale}")]
    DuplicateSlug { slug: String, locale: String },

    #[error("Cannot delete published page")]
    CannotDeletePublished,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Feature disabled: {feature}")]
    FeatureDisabled { feature: String },

    #[error("Content error: {0}")]
    Content(#[from] rustok_content::ContentError),

    #[error("Rich error: {0}")]
    Rich(#[from] Box<RichError>),
}

pub type PagesResult<T> = Result<T, PagesError>;

pub const FEATURE_BUILDER_ENABLED: &str = "builder.enabled";
pub const FEATURE_BUILDER_PREVIEW_ENABLED: &str = "builder.preview.enabled";
pub const FEATURE_BUILDER_PROPERTIES_ENABLED: &str = "builder.properties.enabled";
pub const FEATURE_BUILDER_PUBLISH_ENABLED: &str = "builder.publish.enabled";
pub const BUILDER_FEATURE_DISABLED_ERROR_CODE: &str = "FEATURE_DISABLED";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuilderRuntimeErrorCatalogEntry {
    pub semantic: &'static str,
    pub adapter_key: &'static str,
    pub rich_error_code: Option<&'static str>,
}

pub const BUILDER_RUNTIME_ERROR_CATALOG: [BuilderRuntimeErrorCatalogEntry; 4] = [
    BuilderRuntimeErrorCatalogEntry {
        semantic: "validation",
        adapter_key: "validation",
        rich_error_code: None,
    },
    BuilderRuntimeErrorCatalogEntry {
        semantic: "sanitize",
        adapter_key: "sanitize",
        rich_error_code: None,
    },
    BuilderRuntimeErrorCatalogEntry {
        semantic: "runtime",
        adapter_key: "runtime",
        rich_error_code: None,
    },
    BuilderRuntimeErrorCatalogEntry {
        semantic: "feature_disabled",
        adapter_key: "feature-disabled",
        rich_error_code: Some(BUILDER_FEATURE_DISABLED_ERROR_CODE),
    },
];

pub fn builder_runtime_error_catalog() -> &'static [BuilderRuntimeErrorCatalogEntry] {
    &BUILDER_RUNTIME_ERROR_CATALOG
}

// Conversion from PagesError to RichError for API responses
impl From<PagesError> for RichError {
    fn from(err: PagesError) -> Self {
        match err {
            PagesError::Database(db_err) => {
                RichError::new(ErrorKind::Database, "Database operation failed")
                    .with_user_message("Unable to access pages data")
                    .with_source(db_err)
            }
            PagesError::Core(core_err) => core_err.into(),
            PagesError::PageNotFound(id) => {
                RichError::new(ErrorKind::NotFound, format!("Page {} not found", id))
                    .with_user_message("The requested page does not exist")
                    .with_field("page_id", id.to_string())
                    .with_error_code("PAGE_NOT_FOUND")
            }
            PagesError::BlockNotFound(id) => {
                RichError::new(ErrorKind::NotFound, format!("Block {} not found", id))
                    .with_user_message("The requested block does not exist")
                    .with_field("block_id", id.to_string())
                    .with_error_code("BLOCK_NOT_FOUND")
            }
            PagesError::MenuNotFound(id) => {
                RichError::new(ErrorKind::NotFound, format!("Menu {} not found", id))
                    .with_user_message("The requested menu does not exist")
                    .with_field("menu_id", id.to_string())
                    .with_error_code("MENU_NOT_FOUND")
            }
            PagesError::DuplicateSlug { slug, locale } => RichError::new(
                ErrorKind::Conflict,
                format!("Slug '{}' already exists for locale '{}'", slug, locale),
            )
            .with_user_message("This URL slug is already in use. Please choose a different one.")
            .with_field("slug", slug)
            .with_field("locale", locale)
            .with_error_code("DUPLICATE_SLUG"),
            PagesError::CannotDeletePublished => {
                RichError::new(ErrorKind::BusinessLogic, "Cannot delete published page")
                    .with_user_message("Published pages cannot be deleted. Unpublish them first.")
                    .with_error_code("CANNOT_DELETE_PUBLISHED")
            }
            PagesError::Validation(msg) => {
                RichError::new(ErrorKind::Validation, msg).with_user_message("Invalid input data")
            }
            PagesError::Forbidden(msg) => RichError::new(ErrorKind::Forbidden, msg)
                .with_user_message("You do not have permission to perform this action"),
            PagesError::FeatureDisabled { feature } => RichError::new(
                ErrorKind::BusinessLogic,
                format!("Feature '{feature}' is disabled for this tenant"),
            )
            .with_user_message("This feature is disabled for the current tenant")
            .with_field("feature", feature)
            .with_error_code(BUILDER_FEATURE_DISABLED_ERROR_CODE),
            PagesError::Content(content_err) => content_err.into(),
            PagesError::Rich(rich) => *rich,
        }
    }
}

/// Helper functions for creating common pages errors
impl PagesError {
    /// Create a page not found error
    pub fn page_not_found(page_id: Uuid) -> Self {
        PagesError::PageNotFound(page_id)
    }

    /// Create a block not found error
    pub fn block_not_found(block_id: Uuid) -> Self {
        PagesError::BlockNotFound(block_id)
    }

    /// Create a menu not found error
    pub fn menu_not_found(menu_id: Uuid) -> Self {
        PagesError::MenuNotFound(menu_id)
    }

    /// Create a duplicate slug error
    pub fn duplicate_slug(slug: impl Into<String>, locale: impl Into<String>) -> Self {
        PagesError::DuplicateSlug {
            slug: slug.into(),
            locale: locale.into(),
        }
    }

    /// Create a validation error
    pub fn validation(message: impl Into<String>) -> Self {
        PagesError::Validation(message.into())
    }

    /// Create a forbidden error
    pub fn forbidden(message: impl Into<String>) -> Self {
        PagesError::Forbidden(message.into())
    }

    /// Create a cannot delete published error
    pub fn cannot_delete_published() -> Self {
        PagesError::CannotDeletePublished
    }

    /// Create a feature disabled error
    pub fn feature_disabled(feature: impl Into<String>) -> Self {
        PagesError::FeatureDisabled {
            feature: feature.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_not_found_conversion() {
        let id = Uuid::new_v4();
        let err = PagesError::page_not_found(id);
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::NotFound);
        assert_eq!(rich.status_code, 404);
        assert!(rich.fields.contains_key("page_id"));
    }

    #[test]
    fn test_block_not_found_conversion() {
        let id = Uuid::new_v4();
        let err = PagesError::block_not_found(id);
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::NotFound);
        assert_eq!(rich.status_code, 404);
        assert!(rich.fields.contains_key("block_id"));
    }

    #[test]
    fn test_menu_not_found_conversion() {
        let id = Uuid::new_v4();
        let err = PagesError::menu_not_found(id);
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::NotFound);
        assert_eq!(rich.status_code, 404);
        assert!(rich.fields.contains_key("menu_id"));
    }

    #[test]
    fn test_duplicate_slug_conversion() {
        let err = PagesError::duplicate_slug("my-page", "en");
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::Conflict);
        assert_eq!(rich.status_code, 409);
        assert_eq!(rich.fields.get("slug"), Some(&"my-page".to_string()));
    }

    #[test]
    fn test_cannot_delete_published_conversion() {
        let err = PagesError::cannot_delete_published();
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::BusinessLogic);
        assert_eq!(rich.error_code, Some("CANNOT_DELETE_PUBLISHED".to_string()));
    }

    #[test]
    fn test_feature_disabled_conversion() {
        let err = PagesError::feature_disabled(FEATURE_BUILDER_PUBLISH_ENABLED);
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::BusinessLogic);
        assert_eq!(
            rich.error_code,
            Some(BUILDER_FEATURE_DISABLED_ERROR_CODE.to_string())
        );
        assert_eq!(
            rich.fields.get("feature"),
            Some(&FEATURE_BUILDER_PUBLISH_ENABLED.to_string())
        );
    }

    #[test]
    fn test_builder_runtime_error_catalog_is_stable() {
        let catalog = builder_runtime_error_catalog();
        let semantics: Vec<_> = catalog.iter().map(|entry| entry.semantic).collect();
        assert_eq!(
            semantics,
            vec!["validation", "sanitize", "runtime", "feature_disabled"]
        );

        let feature_disabled = catalog
            .iter()
            .find(|entry| entry.semantic == "feature_disabled")
            .expect("feature-disabled catalog entry must exist");
        assert_eq!(feature_disabled.adapter_key, "feature-disabled");
        assert_eq!(
            feature_disabled.rich_error_code,
            Some(BUILDER_FEATURE_DISABLED_ERROR_CODE)
        );
    }

    #[test]
    fn test_builder_feature_keys_are_stable() {
        assert_eq!(FEATURE_BUILDER_ENABLED, "builder.enabled");
        assert_eq!(FEATURE_BUILDER_PREVIEW_ENABLED, "builder.preview.enabled");
        assert_eq!(
            FEATURE_BUILDER_PROPERTIES_ENABLED,
            "builder.properties.enabled"
        );
        assert_eq!(FEATURE_BUILDER_PUBLISH_ENABLED, "builder.publish.enabled");
    }
}
