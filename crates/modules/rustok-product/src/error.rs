use rustok_core::error::Error as CoreError;
use thiserror::Error;
use uuid::Uuid;

/// Errors owned by the Product domain.
#[derive(Error, Debug)]
pub enum CommerceError {
    #[error("Product storage error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Product not found: {0}")]
    ProductNotFound(Uuid),

    #[error("Duplicate handle: {handle} already exists for locale {locale}")]
    DuplicateHandle { handle: String, locale: String },

    #[error("Duplicate SKU: {0}")]
    DuplicateSku(String),

    #[error("Product validation error: {0}")]
    Validation(String),

    #[error("Product state conflict: {0}")]
    Conflict(String),

    #[error("Product must have at least one variant")]
    NoVariants,

    #[error("Product variant not found: {0}")]
    VariantNotFound(Uuid),

    #[error("Product image not found: {0}")]
    ImageNotFound(Uuid),

    #[error("Cannot delete the only variant of a product")]
    CannotDeleteOnlyVariant,

    #[error("Cannot delete published product")]
    CannotDeletePublished,

    #[error("Product core operation failed: {0}")]
    Core(#[from] CoreError),
}

pub type CommerceResult<T> = Result<T, CommerceError>;

/// Flex attached values are an owner dependency, not a source of public error copy.
/// Keep this match exhaustive so additions to Flex require an explicit Product policy.
impl From<rustok_core::field_schema::FlexError> for CommerceError {
    fn from(error: rustok_core::field_schema::FlexError) -> Self {
        use rustok_core::field_schema::FlexError;
        match error {
            FlexError::Database(message) => Self::Database(sea_orm::DbErr::Custom(message)),
            // Product supplies a fixed attached entity type. Its rejection is owner/configuration
            // failure, not invalid user metadata, and must never become a terminal validation.
            FlexError::UnknownEntityType(_) => Self::Core(CoreError::Internal(
                "Product custom field entity registration is invalid".into(),
            )),
            // These describe the submitted custom-field payload/definition selection, not a
            // missing Product aggregate. Preserve the existing validation class, without data.
            FlexError::NotFound(_) => {
                Self::Validation("Custom field definition was not found".into())
            }
            FlexError::DuplicateFieldKey(_) => {
                Self::Validation("Custom field key already exists".into())
            }
            FlexError::InvalidLocale(_) => {
                Self::Validation("Custom field locale is invalid".into())
            }
            FlexError::InvalidFieldKey(_) => Self::Validation("Custom field key is invalid".into()),
            FlexError::TooManyFields { .. } => {
                Self::Validation("Custom field definition limit exceeded".into())
            }
            FlexError::ValidationFailed(_) => {
                Self::Validation("Custom field validation failed".into())
            }
        }
    }
}

/// Canonical classification of nested Core failures at Product boundaries.
/// No dependency message, source/debug rendering or submitted value is public copy.
fn core_error_policy(
    error: &CoreError,
) -> (rustok_api::PortErrorKind, &'static str, &'static str, bool) {
    use rustok_api::PortErrorKind;
    match error {
        CoreError::Database(_) => (
            PortErrorKind::Unavailable,
            "product.database_unavailable",
            "product storage is temporarily unavailable",
            true,
        ),
        CoreError::Cache(_) | CoreError::External(_) => (
            PortErrorKind::Unavailable,
            "product.dependency_unavailable",
            "product dependency is temporarily unavailable",
            true,
        ),
        CoreError::Auth(_) | CoreError::Forbidden(_) => (
            PortErrorKind::Forbidden,
            "product.forbidden",
            "product access is denied",
            false,
        ),
        CoreError::InvalidIdFormat(_) | CoreError::Validation(_) => (
            PortErrorKind::Validation,
            "product.validation",
            "product request is invalid",
            false,
        ),
        CoreError::NotFound(_) => (
            PortErrorKind::NotFound,
            "product.dependency_not_found",
            "referenced product resource was not found",
            false,
        ),
        CoreError::Serialization(_) | CoreError::Scripting(_) | CoreError::Internal(_) => (
            PortErrorKind::InvariantViolation,
            "product.invariant_violation",
            "product operation could not be completed safely",
            false,
        ),
    }
}

pub fn core_port_error(error: &CoreError) -> rustok_api::PortError {
    let (kind, code, message, retryable) = core_error_policy(error);
    rustok_api::PortError::new(kind, code, message, retryable)
}

pub fn core_error_code(error: &CoreError) -> &'static str {
    core_error_policy(error).1
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_core::field_schema::FlexError;

    #[test]
    fn nested_core_classes_are_explicit_and_never_render_dependency_details() {
        use rustok_api::PortErrorKind as K;
        let serialization = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let cases = [
            (
                CoreError::Database(sea_orm::DbErr::Custom("private-marker".into())),
                K::Unavailable,
                true,
            ),
            (
                CoreError::Serialization(serialization),
                K::InvariantViolation,
                false,
            ),
            (
                CoreError::Auth("private-marker".into()),
                K::Forbidden,
                false,
            ),
            (
                CoreError::Forbidden("private-marker".into()),
                K::Forbidden,
                false,
            ),
            (
                CoreError::NotFound("private-marker".into()),
                K::NotFound,
                false,
            ),
            (
                CoreError::Cache("private-marker".into()),
                K::Unavailable,
                true,
            ),
            (
                CoreError::External("private-marker".into()),
                K::Unavailable,
                true,
            ),
            (
                CoreError::InvalidIdFormat("private-marker".into()),
                K::Validation,
                false,
            ),
            (
                CoreError::Validation("private-marker".into()),
                K::Validation,
                false,
            ),
            (
                CoreError::Scripting("private-marker".into()),
                K::InvariantViolation,
                false,
            ),
            (
                CoreError::Internal("private-marker".into()),
                K::InvariantViolation,
                false,
            ),
        ];
        for (error, kind, retryable) in cases {
            let port = core_port_error(&error);
            assert_eq!(port.kind, kind);
            assert_eq!(port.retryable, retryable);
            assert_eq!(port.code, core_error_code(&error));
            assert!(!port.message.contains("private-marker"));
            let public = crate::map_product_public_error(
                &CommerceError::Core(error),
                "test",
                "product_core",
            );
            assert_eq!(public.retryable, retryable);
            assert!(!public.to_string().contains("private-marker"));
        }
    }

    #[test]
    fn flex_storage_and_fixed_entity_failures_are_not_client_validation() {
        assert!(matches!(
            CommerceError::from(FlexError::Database("private database detail".into())),
            CommerceError::Database(_)
        ));
        let registration =
            CommerceError::from(FlexError::UnknownEntityType("private entity detail".into()));
        assert!(matches!(
            &registration,
            CommerceError::Core(CoreError::Internal(_))
        ));
        assert!(!registration.to_string().contains("private entity detail"));
    }

    #[test]
    fn flex_payload_errors_have_explicit_static_copy_without_tenant_text() {
        let id = Uuid::new_v4();
        let cases = [
            FlexError::NotFound(id),
            FlexError::DuplicateFieldKey("private field detail".into()),
            FlexError::InvalidLocale("private locale detail".into()),
            FlexError::InvalidFieldKey("private field detail".into()),
            FlexError::TooManyFields {
                entity_type: "private entity detail".into(),
                max: 99,
            },
            FlexError::ValidationFailed(vec![]),
        ];
        for error in cases {
            let mapped = CommerceError::from(error);
            assert!(matches!(&mapped, CommerceError::Validation(_)));
            let rendered = mapped.to_string();
            assert!(!rendered.contains("private"));
            assert!(!rendered.contains(&id.to_string()));
        }
    }
}
