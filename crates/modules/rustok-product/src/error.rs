use rustok_core::error::Error as CoreError;
use thiserror::Error;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Bounded diagnostic facts for Product owner errors
// ---------------------------------------------------------------------------

/// Structured diagnostic summary of a `CommerceError` that is safe to log and
/// propagate to observability without leaking internal DB details.
///
/// This is the single canonical implementation used by both the read port
/// (`ports.rs`), the command port (`catalog_command_port.rs`), and the public
/// error layer (`public_error.rs`).
pub(crate) struct ProductOwnerErrorFacts {
    pub error_variant: &'static str,
    pub text_field_count: usize,
    pub text_total_length: usize,
    pub uuid_field_count: usize,
    pub uuid_non_nil_count: usize,
    pub opaque_payload_present: bool,
}

impl ProductOwnerErrorFacts {
    pub fn empty(error_variant: &'static str) -> Self {
        Self {
            error_variant,
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        }
    }

    pub fn text(error_variant: &'static str, values: &[&str]) -> Self {
        Self {
            text_field_count: values.len(),
            text_total_length: values.iter().map(|value| value.chars().count()).sum(),
            ..Self::empty(error_variant)
        }
    }

    pub fn uuids(error_variant: &'static str, values: &[Uuid]) -> Self {
        Self {
            uuid_field_count: values.len(),
            uuid_non_nil_count: values.iter().filter(|value| !value.is_nil()).count(),
            ..Self::empty(error_variant)
        }
    }

    pub fn opaque(error_variant: &'static str) -> Self {
        Self {
            opaque_payload_present: true,
            ..Self::empty(error_variant)
        }
    }
}

pub(crate) fn product_owner_error_facts(error: &CommerceError) -> ProductOwnerErrorFacts {
    match error {
        CommerceError::Database(_) => ProductOwnerErrorFacts::opaque("database"),
        CommerceError::ProductNotFound(value) => {
            ProductOwnerErrorFacts::uuids("product_not_found", &[*value])
        }
        CommerceError::DuplicateHandle { handle, locale } => {
            ProductOwnerErrorFacts::text("duplicate_handle", &[handle.as_str(), locale.as_str()])
        }
        CommerceError::DuplicateSku(value) => {
            ProductOwnerErrorFacts::text("duplicate_sku", &[value.as_str()])
        }
        CommerceError::Validation(value) => {
            ProductOwnerErrorFacts::text("validation", &[value.as_str()])
        }
        CommerceError::NoVariants => ProductOwnerErrorFacts::empty("no_variants"),
        CommerceError::VariantNotFound(value) => {
            ProductOwnerErrorFacts::uuids("variant_not_found", &[*value])
        }
        CommerceError::ImageNotFound(value) => {
            ProductOwnerErrorFacts::uuids("image_not_found", &[*value])
        }
        CommerceError::CannotDeleteOnlyVariant => {
            ProductOwnerErrorFacts::empty("cannot_delete_only_variant")
        }
        CommerceError::CannotDeletePublished => {
            ProductOwnerErrorFacts::empty("cannot_delete_published")
        }
        CommerceError::Core(_) => ProductOwnerErrorFacts::opaque("core"),
    }
}

/// Canonical Product owner error code for diagnostic logging.
///
/// Every `CommerceError` variant has an explicit code — no wildcard fallback.
pub(crate) fn product_error_code(error: &CommerceError) -> &'static str {
    match error {
        CommerceError::Database(_) => "product.database_unavailable",
        CommerceError::ProductNotFound(_) => "product.product_not_found",
        CommerceError::VariantNotFound(_) => "product.variant_not_found",
        CommerceError::ImageNotFound(_) => "product.image_not_found",
        CommerceError::CannotDeleteOnlyVariant => "product.cannot_delete_only_variant",
        CommerceError::DuplicateHandle { .. } => "product.duplicate_handle",
        CommerceError::DuplicateSku(_) => "product.duplicate_sku",
        CommerceError::Validation(_) => "product.validation",
        CommerceError::NoVariants => "product.no_variants",
        CommerceError::CannotDeletePublished => "product.cannot_delete_published",
        CommerceError::Core(_) => "product.invariant_violation",
    }
}

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
