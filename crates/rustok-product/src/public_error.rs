use uuid::Uuid;

use crate::CommerceError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductPublicError {
    pub message: &'static str,
    pub code: &'static str,
    pub retryable: bool,
    pub correlation_id: Uuid,
}

impl std::fmt::Display for ProductPublicError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} (code: {}; reference: {})",
            self.message, self.code, self.correlation_id
        )
    }
}

pub fn map_product_public_error(
    error: &CommerceError,
    operation: &'static str,
    boundary: &'static str,
) -> ProductPublicError {
    let (message, code, retryable) = match error {
        CommerceError::Database(_) => (
            "Product data is temporarily unavailable",
            "PRODUCT_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        CommerceError::ProductNotFound(_) => ("Product was not found", "PRODUCT_NOT_FOUND", false),
        CommerceError::DuplicateHandle { .. } => (
            "Product handle conflicts with an existing product",
            "DUPLICATE_HANDLE",
            false,
        ),
        CommerceError::DuplicateSku(_) => (
            "Product SKU conflicts with an existing product",
            "DUPLICATE_SKU",
            false,
        ),
        CommerceError::Validation(_) => ("Product request is invalid", "PRODUCT_VALIDATION", false),
        CommerceError::NoVariants => (
            "Product requires at least one variant",
            "NO_VARIANTS",
            false,
        ),
        CommerceError::CannotDeletePublished => (
            "Published products must be archived before removal",
            "CANNOT_DELETE_PUBLISHED",
            false,
        ),
        CommerceError::Core(_) => (
            "Product operation could not be completed safely",
            "PRODUCT_OPERATION_FAILED",
            false,
        ),
    };
    let correlation_id = Uuid::new_v4();

    tracing::error!(
        error = ?error,
        operation,
        public_code = code,
        retryable,
        boundary,
        %correlation_id,
        "product service operation failed"
    );

    ProductPublicError {
        message,
        code,
        retryable,
        correlation_id,
    }
}

#[cfg(test)]
mod tests {
    use super::map_product_public_error;
    use crate::CommerceError;

    #[test]
    fn database_details_are_redacted_from_public_product_errors() {
        let error = CommerceError::Database(sea_orm::DbErr::Custom(
            "password=private host=internal".to_owned(),
        ));
        let public = map_product_public_error(&error, "test", "product_test");
        let rendered = public.to_string();

        assert_eq!(public.code, "PRODUCT_TEMPORARILY_UNAVAILABLE");
        assert!(public.retryable);
        assert!(!rendered.contains("password=private"));
        assert!(!rendered.contains("host=internal"));
        assert!(rendered.contains(&public.correlation_id.to_string()));
    }
}
