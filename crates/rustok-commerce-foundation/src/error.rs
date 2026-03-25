use rustok_core::error::{Error as CoreError, ErrorKind, RichError};
use thiserror::Error;
use uuid::Uuid;

/// Commerce module errors
///
/// Uses both legacy Error enum and new RichError system.
/// Gradually migrate to RichError for better context.
#[derive(Error, Debug)]
pub enum CommerceError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Product not found: {0}")]
    ProductNotFound(Uuid),

    #[error("Variant not found: {0}")]
    VariantNotFound(Uuid),

    #[error("Duplicate handle: {handle} already exists for locale {locale}")]
    DuplicateHandle { handle: String, locale: String },

    #[error("Duplicate SKU: {0}")]
    DuplicateSku(String),

    #[error("Invalid price: {0}")]
    InvalidPrice(String),

    #[error("Insufficient inventory: requested {requested}, available {available}")]
    InsufficientInventory { requested: i32, available: i32 },

    #[error("Invalid option combination")]
    InvalidOptionCombination,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Product must have at least one variant")]
    NoVariants,

    #[error("Cannot delete published product")]
    CannotDeletePublished,

    #[error("Rich error: {0}")]
    Rich(#[from] RichError),

    #[error("Core error: {0}")]
    Core(#[from] CoreError),
}

pub type CommerceResult<T> = Result<T, CommerceError>;

// Conversion from CommerceError to RichError for API responses
impl From<CommerceError> for RichError {
    fn from(err: CommerceError) -> Self {
        match err {
            CommerceError::Database(db_err) => {
                RichError::new(ErrorKind::Database, "Database operation failed")
                    .with_user_message("Unable to access product data")
                    .with_source(db_err)
            }
            CommerceError::ProductNotFound(id) => {
                RichError::new(ErrorKind::NotFound, format!("Product {} not found", id))
                    .with_user_message("The requested product does not exist")
                    .with_field("product_id", id.to_string())
                    .with_error_code("PRODUCT_NOT_FOUND")
            }
            CommerceError::VariantNotFound(id) => {
                RichError::new(ErrorKind::NotFound, format!("Variant {} not found", id))
                    .with_user_message("The requested product variant does not exist")
                    .with_field("variant_id", id.to_string())
                    .with_error_code("VARIANT_NOT_FOUND")
            }
            CommerceError::DuplicateHandle { handle, locale } => RichError::new(
                ErrorKind::Conflict,
                format!("Handle '{}' already exists for locale '{}'", handle, locale),
            )
            .with_user_message("A product with this handle already exists")
            .with_field("handle", handle)
            .with_field("locale", locale)
            .with_error_code("DUPLICATE_HANDLE"),
            CommerceError::DuplicateSku(sku) => {
                RichError::new(ErrorKind::Conflict, format!("SKU '{}' already exists", sku))
                    .with_user_message("A product with this SKU already exists")
                    .with_field("sku", sku)
                    .with_error_code("DUPLICATE_SKU")
            }
            CommerceError::InvalidPrice(msg) => RichError::new(ErrorKind::Validation, msg)
                .with_user_message("Invalid price value")
                .with_error_code("INVALID_PRICE"),
            CommerceError::InsufficientInventory {
                requested,
                available,
            } => RichError::new(
                ErrorKind::BusinessLogic,
                format!(
                    "Insufficient inventory: requested {}, available {}",
                    requested, available
                ),
            )
            .with_user_message("Not enough items in stock")
            .with_field("requested", requested.to_string())
            .with_field("available", available.to_string())
            .with_error_code("INSUFFICIENT_INVENTORY"),
            CommerceError::InvalidOptionCombination => {
                RichError::new(ErrorKind::Validation, "Invalid option combination")
                    .with_user_message("The selected product options are not available")
                    .with_error_code("INVALID_OPTIONS")
            }
            CommerceError::Validation(msg) => {
                RichError::new(ErrorKind::Validation, msg).with_user_message("Invalid input data")
            }
            CommerceError::NoVariants => RichError::new(
                ErrorKind::Validation,
                "Product must have at least one variant",
            )
            .with_user_message("Product must have at least one variant")
            .with_error_code("NO_VARIANTS"),
            CommerceError::CannotDeletePublished => {
                RichError::new(ErrorKind::BusinessLogic, "Cannot delete published product")
                    .with_user_message(
                        "Published products cannot be deleted. Archive them instead.",
                    )
                    .with_error_code("CANNOT_DELETE_PUBLISHED")
            }
            CommerceError::Rich(rich) => rich,
            CommerceError::Core(core) => core.into(),
        }
    }
}

/// Helper functions for creating common commerce errors
impl CommerceError {
    /// Create a product not found error
    pub fn product_not_found(product_id: Uuid) -> Self {
        CommerceError::ProductNotFound(product_id)
    }

    /// Create a variant not found error
    pub fn variant_not_found(variant_id: Uuid) -> Self {
        CommerceError::VariantNotFound(variant_id)
    }

    /// Create a duplicate handle error
    pub fn duplicate_handle(handle: impl Into<String>, locale: impl Into<String>) -> Self {
        CommerceError::DuplicateHandle {
            handle: handle.into(),
            locale: locale.into(),
        }
    }

    /// Create a duplicate SKU error
    pub fn duplicate_sku(sku: impl Into<String>) -> Self {
        CommerceError::DuplicateSku(sku.into())
    }

    /// Create an insufficient inventory error
    pub fn insufficient_inventory(requested: i32, available: i32) -> Self {
        CommerceError::InsufficientInventory {
            requested,
            available,
        }
    }

    /// Create a validation error
    pub fn validation(message: impl Into<String>) -> Self {
        CommerceError::Validation(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_product_not_found_conversion() {
        let id = Uuid::new_v4();
        let err = CommerceError::product_not_found(id);
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::NotFound);
        assert_eq!(rich.status_code, 404);
        assert!(rich.fields.contains_key("product_id"));
    }

    #[test]
    fn test_duplicate_handle_conversion() {
        let err = CommerceError::duplicate_handle("my-product", "en");
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::Conflict);
        assert_eq!(rich.status_code, 409);
        assert_eq!(rich.fields.get("handle"), Some(&"my-product".to_string()));
    }

    #[test]
    fn test_insufficient_inventory_conversion() {
        let err = CommerceError::insufficient_inventory(10, 5);
        let rich: RichError = err.into();

        assert_eq!(rich.kind, ErrorKind::BusinessLogic);
        assert_eq!(rich.fields.get("requested"), Some(&"10".to_string()));
        assert_eq!(rich.fields.get("available"), Some(&"5".to_string()));
    }
}
