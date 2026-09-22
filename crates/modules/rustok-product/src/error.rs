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

impl From<rustok_core::field_schema::FlexError> for CommerceError {
    fn from(error: rustok_core::field_schema::FlexError) -> Self {
        match error {
            rustok_core::field_schema::FlexError::Database(message) => {
                CommerceError::Database(sea_orm::DbErr::Custom(message))
            }
            rustok_core::field_schema::FlexError::NotFound(id) => {
                CommerceError::Validation(format!("Custom field definition `{id}` was not found"))
            }
            rustok_core::field_schema::FlexError::DuplicateFieldKey(key) => {
                CommerceError::Validation(format!("Custom field key `{key}` already exists"))
            }
            rustok_core::field_schema::FlexError::InvalidLocale(locale) => {
                CommerceError::Validation(format!("Invalid custom field locale: `{locale}`"))
            }
            other => CommerceError::Validation(other.to_string()),
        }
    }
}
