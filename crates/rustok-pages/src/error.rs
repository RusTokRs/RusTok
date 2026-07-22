use rustok_core::error::{Error as CoreError, ErrorKind, RichError};
use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum PagesError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    #[error("Core error: {0}")]
    Core(#[from] CoreError),

    #[error("Page not found: {0}")]
    PageNotFound(Uuid),

    #[error("Menu not found: {0}")]
    MenuNotFound(Uuid),

    #[error("Duplicate slug: {slug} already exists for locale {locale}")]
    DuplicateSlug { slug: String, locale: String },

    #[error("Page version conflict: expected {expected_version}, found {actual_version}")]
    VersionConflict {
        expected_version: i32,
        actual_version: i32,
    },

    #[error("Cannot delete published page")]
    CannotDeletePublished,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Static landing artifact integrity error: {0}")]
    ArtifactIntegrity(String),

    #[error("Page Builder publish runtime review invalid: {0}")]
    PublishRuntimeReviewInvalid(String),

    #[error("Page Builder publish sanitization failed: {0}")]
    PublishSanitize(String),

    #[error("Page Builder publish runtime materialization mismatch: {0}")]
    PublishRuntimeMaterializationMismatch(String),

    #[error("Page publish idempotency conflict: {0}")]
    PublishIdempotencyConflict(String),

    #[error("Page publish operation integrity error: {0}")]
    PublishOperationIntegrity(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Feature disabled: {feature}")]
    FeatureDisabled { feature: String },

    #[error("Content error: {0}")]
    Content(#[from] rustok_content::ContentError),

    #[error("Tenant contract error: {0}")]
    Tenant(#[from] rustok_tenant::TenantError),

    #[error("Rich error: {0}")]
    Rich(#[from] Box<RichError>),
}

pub type PagesResult<T> = Result<T, PagesError>;

pub const FEATURE_BUILDER_ENABLED: &str = "builder.enabled";
pub const FEATURE_BUILDER_PREVIEW_ENABLED: &str = "builder.preview.enabled";
pub const FEATURE_BUILDER_PROPERTIES_ENABLED: &str = "builder.properties.enabled";
pub const FEATURE_BUILDER_PUBLISH_ENABLED: &str = "builder.publish.enabled";
pub const BUILDER_FEATURE_DISABLED_ERROR_CODE: &str = "FEATURE_DISABLED";
pub const CANNOT_DELETE_PUBLISHED_ERROR_CODE: &str = "CANNOT_DELETE_PUBLISHED";
pub const PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID: &str =
    "PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID";
pub const PAGE_BUILDER_PUBLISH_SANITIZE_FAILED: &str = "PAGE_BUILDER_PUBLISH_SANITIZE_FAILED";
pub const PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH: &str =
    "PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH";
pub const PAGE_PUBLISH_IDEMPOTENCY_CONFLICT: &str = "PAGE_PUBLISH_IDEMPOTENCY_CONFLICT";
pub const PAGE_PUBLISH_OPERATION_INTEGRITY: &str = "PAGE_PUBLISH_OPERATION_INTEGRITY";

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

impl From<PagesError> for RichError {
    fn from(error: PagesError) -> Self {
        match error {
            PagesError::Database(source) => {
                RichError::new(ErrorKind::Database, "Database operation failed")
                    .with_user_message("Unable to access pages data")
                    .with_source(source)
            }
            PagesError::Core(source) => source.into(),
            PagesError::PageNotFound(id) => {
                RichError::new(ErrorKind::NotFound, format!("Page {id} not found"))
                    .with_user_message("The requested page does not exist")
                    .with_field("page_id", id.to_string())
                    .with_error_code("PAGE_NOT_FOUND")
            }
            PagesError::MenuNotFound(id) => {
                RichError::new(ErrorKind::NotFound, format!("Menu {id} not found"))
                    .with_user_message("The requested menu does not exist")
                    .with_field("menu_id", id.to_string())
                    .with_error_code("MENU_NOT_FOUND")
            }
            PagesError::DuplicateSlug { slug, locale } => RichError::new(
                ErrorKind::Conflict,
                format!("Slug '{slug}' already exists for locale '{locale}'"),
            )
            .with_user_message("This URL slug is already in use. Please choose a different one.")
            .with_field("slug", slug)
            .with_field("locale", locale)
            .with_error_code("DUPLICATE_SLUG"),
            PagesError::VersionConflict {
                expected_version,
                actual_version,
            } => RichError::new(
                ErrorKind::Conflict,
                format!(
                    "Page metadata changed concurrently: expected version {expected_version}, found {actual_version}"
                ),
            )
            .with_user_message(
                "The page metadata changed while you were editing it. Reload and try again.",
            )
            .with_field("expected_version", expected_version.to_string())
            .with_field("actual_version", actual_version.to_string())
            .with_error_code("PAGE_METADATA_VERSION_CONFLICT"),
            PagesError::CannotDeletePublished => {
                RichError::new(ErrorKind::BusinessLogic, "Cannot delete published page")
                    .with_user_message("Published pages cannot be deleted. Unpublish them first.")
                    .with_error_code(CANNOT_DELETE_PUBLISHED_ERROR_CODE)
            }
            PagesError::Validation(message) => RichError::new(ErrorKind::Validation, message)
                .with_user_message("Invalid input data"),
            PagesError::ArtifactIntegrity(message) => RichError::new(ErrorKind::Internal, message)
                .with_user_message("The published page artifact is unavailable")
                .with_error_code("PAGE_ARTIFACT_INTEGRITY"),
            PagesError::PublishRuntimeReviewInvalid(message) => {
                RichError::new(ErrorKind::Validation, message)
                    .with_user_message(
                        "The selected Page Builder runtime must be reviewed again before publish.",
                    )
                    .with_error_code(PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID)
            }
            PagesError::PublishSanitize(message) => RichError::new(ErrorKind::Validation, message)
                .with_user_message(
                    "The Page Builder document did not pass the public publish security policy.",
                )
                .with_error_code(PAGE_BUILDER_PUBLISH_SANITIZE_FAILED),
            PagesError::PublishRuntimeMaterializationMismatch(message) => {
                RichError::new(ErrorKind::Conflict, message)
                    .with_user_message(
                        "The reviewed Page Builder runtime no longer matches the publish artifact. Review and publish again.",
                    )
                    .with_error_code(PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH)
            }
            PagesError::PublishIdempotencyConflict(message) => {
                RichError::new(ErrorKind::Conflict, message)
                    .with_user_message(
                        "This publish idempotency key is already bound to a different request.",
                    )
                    .with_error_code(PAGE_PUBLISH_IDEMPOTENCY_CONFLICT)
            }
            PagesError::PublishOperationIntegrity(message) => {
                RichError::new(ErrorKind::Internal, message)
                    .with_user_message("The stored page publish receipt failed integrity validation.")
                    .with_error_code(PAGE_PUBLISH_OPERATION_INTEGRITY)
            }
            PagesError::Forbidden(message) => RichError::new(ErrorKind::Forbidden, message)
                .with_user_message("You do not have permission to perform this action"),
            PagesError::FeatureDisabled { feature } => RichError::new(
                ErrorKind::BusinessLogic,
                format!("Feature '{feature}' is disabled for this tenant"),
            )
            .with_user_message("This feature is disabled for the current tenant")
            .with_field("feature", feature)
            .with_error_code(BUILDER_FEATURE_DISABLED_ERROR_CODE),
            PagesError::Content(source) => source.into(),
            PagesError::Tenant(source) => RichError::new(
                ErrorKind::Database,
                "Unable to read tenant module configuration",
            )
            .with_user_message("Unable to resolve the current feature configuration")
            .with_source(source),
            PagesError::Rich(error) => *error,
        }
    }
}

impl PagesError {
    pub fn page_not_found(page_id: Uuid) -> Self {
        Self::PageNotFound(page_id)
    }

    pub fn menu_not_found(menu_id: Uuid) -> Self {
        Self::MenuNotFound(menu_id)
    }

    pub fn duplicate_slug(slug: impl Into<String>, locale: impl Into<String>) -> Self {
        Self::DuplicateSlug {
            slug: slug.into(),
            locale: locale.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn artifact_integrity(message: impl Into<String>) -> Self {
        Self::ArtifactIntegrity(message.into())
    }

    pub fn publish_runtime_review_invalid(message: impl Into<String>) -> Self {
        Self::PublishRuntimeReviewInvalid(message.into())
    }

    pub fn publish_sanitize(message: impl Into<String>) -> Self {
        Self::PublishSanitize(message.into())
    }

    pub fn publish_runtime_materialization_mismatch(message: impl Into<String>) -> Self {
        Self::PublishRuntimeMaterializationMismatch(message.into())
    }

    pub fn publish_idempotency_conflict(message: impl Into<String>) -> Self {
        Self::PublishIdempotencyConflict(message.into())
    }

    pub fn publish_operation_integrity(message: impl Into<String>) -> Self {
        Self::PublishOperationIntegrity(message.into())
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden(message.into())
    }

    pub fn cannot_delete_published() -> Self {
        Self::CannotDeletePublished
    }

    pub fn feature_disabled(feature: impl Into<String>) -> Self {
        Self::FeatureDisabled {
            feature: feature.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_not_found_has_stable_code() {
        let error: RichError = PagesError::page_not_found(Uuid::new_v4()).into();
        assert_eq!(error.kind, ErrorKind::NotFound);
        assert_eq!(error.error_code.as_deref(), Some("PAGE_NOT_FOUND"));
    }

    #[test]
    fn metadata_conflict_has_stable_code() {
        let error: RichError = PagesError::VersionConflict {
            expected_version: 2,
            actual_version: 3,
        }
        .into();
        assert_eq!(
            error.error_code.as_deref(),
            Some("PAGE_METADATA_VERSION_CONFLICT")
        );
    }

    #[test]
    fn cannot_delete_published_has_stable_code() {
        let error: RichError = PagesError::cannot_delete_published().into();
        assert_eq!(error.kind, ErrorKind::BusinessLogic);
        assert_eq!(
            error.error_code.as_deref(),
            Some(CANNOT_DELETE_PUBLISHED_ERROR_CODE)
        );
    }

    #[test]
    fn reviewed_publish_errors_have_stable_codes() {
        let review: RichError = PagesError::publish_runtime_review_invalid("invalid").into();
        assert_eq!(review.kind, ErrorKind::Validation);
        assert_eq!(
            review.error_code.as_deref(),
            Some(PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID)
        );

        let sanitize: RichError = PagesError::publish_sanitize("blocked").into();
        assert_eq!(sanitize.kind, ErrorKind::Validation);
        assert_eq!(
            sanitize.error_code.as_deref(),
            Some(PAGE_BUILDER_PUBLISH_SANITIZE_FAILED)
        );

        let mismatch: RichError =
            PagesError::publish_runtime_materialization_mismatch("mismatch").into();
        assert_eq!(mismatch.kind, ErrorKind::Conflict);
        assert_eq!(
            mismatch.error_code.as_deref(),
            Some(PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH)
        );
    }

    #[test]
    fn publish_receipt_errors_have_stable_codes() {
        let conflict: RichError = PagesError::publish_idempotency_conflict("reused").into();
        assert_eq!(conflict.kind, ErrorKind::Conflict);
        assert_eq!(
            conflict.error_code.as_deref(),
            Some(PAGE_PUBLISH_IDEMPOTENCY_CONFLICT)
        );

        let integrity: RichError = PagesError::publish_operation_integrity("invalid").into();
        assert_eq!(integrity.kind, ErrorKind::Internal);
        assert_eq!(
            integrity.error_code.as_deref(),
            Some(PAGE_PUBLISH_OPERATION_INTEGRITY)
        );
    }

    #[test]
    fn builder_feature_keys_are_stable() {
        assert_eq!(FEATURE_BUILDER_ENABLED, "builder.enabled");
        assert_eq!(FEATURE_BUILDER_PREVIEW_ENABLED, "builder.preview.enabled");
        assert_eq!(
            FEATURE_BUILDER_PROPERTIES_ENABLED,
            "builder.properties.enabled"
        );
        assert_eq!(FEATURE_BUILDER_PUBLISH_ENABLED, "builder.publish.enabled");
    }
}
