use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

pub type ProfileResult<T> = Result<T, ProfileError>;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("profile display name must not be empty")]
    EmptyDisplayName,
    #[error("profile display name is too long")]
    DisplayNameTooLong,
    #[error("profile handle must not be empty")]
    EmptyHandle,
    #[error("profile handle contains invalid characters")]
    InvalidHandle,
    #[error("profile handle is too short")]
    HandleTooShort,
    #[error("profile handle is too long")]
    HandleTooLong,
    #[error("profile handle is reserved: {0}")]
    ReservedHandle(String),
    #[error("profile locale is invalid: {0}")]
    InvalidLocale(String),
    #[error("profile for user {0} not found")]
    ProfileNotFound(Uuid),
    #[error("profile for handle {0} not found")]
    ProfileByHandleNotFound(String),
    #[error("localized profile copy for user {0} was not found")]
    LocalizedCopyNotFound(Uuid),
    #[error("profile handle already exists: {0}")]
    DuplicateHandle(String),
    #[error("profile validation failed: {0}")]
    Validation(String),
    #[error("profile presentation is temporarily unavailable")]
    PresentationUnavailable,
    #[error("profile event publication is temporarily unavailable")]
    EventPublishUnavailable,
    #[error(transparent)]
    Database(#[from] DbErr),
}

impl ProfileError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::EmptyDisplayName => "profiles.display_name_empty",
            Self::DisplayNameTooLong => "profiles.display_name_too_long",
            Self::EmptyHandle => "profiles.handle_empty",
            Self::InvalidHandle => "profiles.handle_invalid",
            Self::HandleTooShort => "profiles.handle_too_short",
            Self::HandleTooLong => "profiles.handle_too_long",
            Self::ReservedHandle(_) => "profiles.handle_reserved",
            Self::InvalidLocale(_) => "profiles.locale_invalid",
            Self::ProfileNotFound(_) => "profiles.profile_not_found",
            Self::ProfileByHandleNotFound(_) => "profiles.profile_by_handle_not_found",
            Self::LocalizedCopyNotFound(_) => "profiles.localized_copy_not_found",
            Self::DuplicateHandle(_) => "profiles.handle_duplicate",
            Self::Validation(_) => "profiles.validation_failed",
            Self::PresentationUnavailable => "profiles.presentation_unavailable",
            Self::EventPublishUnavailable => "profiles.event_publish_unavailable",
            Self::Database(_) => "profiles.storage_unavailable",
        }
    }

    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::PresentationUnavailable | Self::EventPublishUnavailable | Self::Database(_)
        )
    }
}

impl From<rustok_taxonomy::TaxonomyError> for ProfileError {
    fn from(value: rustok_taxonomy::TaxonomyError) -> Self {
        match value {
            rustok_taxonomy::TaxonomyError::Database(err) => Self::Database(err),
            rustok_taxonomy::TaxonomyError::Validation(message)
            | rustok_taxonomy::TaxonomyError::DuplicateCanonicalKey(message)
            | rustok_taxonomy::TaxonomyError::DuplicateSlug(message)
            | rustok_taxonomy::TaxonomyError::DuplicateAlias(message)
            | rustok_taxonomy::TaxonomyError::Forbidden(message)
            | rustok_taxonomy::TaxonomyError::Conflict(message) => Self::Validation(message),
            rustok_taxonomy::TaxonomyError::TermNotFound(term_id) => {
                Self::Validation(format!("taxonomy term not found: {term_id}"))
            }
            rustok_taxonomy::TaxonomyError::TranslationRevisionExhausted { term_id, locale } => {
                Self::Validation(format!(
                    "taxonomy translation revision is exhausted for term {term_id} and locale {locale}"
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ProfileError;

    #[test]
    fn public_error_codes_do_not_include_sensitive_values() {
        assert_eq!(
            ProfileError::ReservedHandle("private-handle".into()).code(),
            "profiles.handle_reserved"
        );
        assert_eq!(
            ProfileError::InvalidLocale("private-locale".into()).code(),
            "profiles.locale_invalid"
        );
    }

    #[test]
    fn only_availability_failures_are_retryable() {
        assert!(ProfileError::PresentationUnavailable.is_retryable());
        assert!(ProfileError::EventPublishUnavailable.is_retryable());
        assert!(!ProfileError::InvalidHandle.is_retryable());
    }
}
