use rust_decimal::Decimal;
use rustok_api::{Patch, TenantLocale};
use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::{Validate, ValidationError};

use super::{
    CreateVariantInput, VariantAxisConfigResponse, VariantAxisInput, VariantResponse,
};
use crate::entities::product::ProductStatus;
use crate::fulfillment::ProductFulfillmentRequirement;

fn deserialize_tenant_locale<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    TenantLocale::new(raw)
        .map(TenantLocale::into_inner)
        .map_err(serde::de::Error::custom)
}

fn validate_tenant_locale(locale: &str) -> Result<(), ValidationError> {
    match TenantLocale::new(locale) {
        Ok(canonical) if canonical.as_str() == locale => Ok(()),
        _ => Err(ValidationError::new("tenant_locale")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, Validate)]
pub struct CreateProductInput {
    #[validate(length(min = 1, message = "At least one translation required"))]
    #[validate(nested)]
    pub translations: Vec<ProductTranslationInput>,
    #[serde(default)]
    #[validate(nested)]
    pub variant_axes: Vec<VariantAxisInput>,
    #[validate(nested)]
    pub variants: Vec<CreateVariantInput>,
    #[validate(length(max = 100, message = "Seller ID must be max 100 characters"))]
    pub seller_id: Option<String>,
    #[validate(length(max = 255, message = "Vendor must be max 255 characters"))]
    pub vendor: Option<String>,
    #[validate(length(max = 255, message = "Product type must be max 255 characters"))]
    pub product_type: Option<String>,
    #[validate(length(
        min = 1,
        max = 64,
        message = "Shipping profile slug must be 1-64 characters"
    ))]
    pub shipping_profile_slug: Option<String>,
    pub primary_category_id: Option<Uuid>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub publish: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct ProductTranslationInput {
    #[serde(deserialize_with = "deserialize_tenant_locale")]
    #[validate(custom(function = "validate_tenant_locale"))]
    pub locale: String,
    #[validate(length(min = 1, max = 255, message = "Title must be 1-255 characters"))]
    pub title: String,
    #[validate(length(max = 255, message = "Handle must be max 255 characters"))]
    pub handle: Option<String>,
    pub description: Option<String>,
    #[validate(length(max = 255, message = "Meta title must be max 255 characters"))]
    pub meta_title: Option<String>,
    #[validate(length(max = 500, message = "Meta description must be max 500 characters"))]
    pub meta_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, Validate)]
pub struct UpdateProductInput {
    #[validate(nested)]
    pub translations: Option<Vec<ProductTranslationInput>>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub seller_id: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub vendor: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub product_type: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub shipping_profile_slug: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<Uuid>)]
    pub primary_category_id: Patch<Uuid>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
    pub status: Option<ProductStatus>,
    /// Predecessor revision of the product aggregate.
    ///
    /// A transport that edits the product document must send the revision it read. `None` is an
    /// explicit unconditional write and is reserved for owner-internal callers; every external
    /// transport requires the field.
    pub expected_revision: Option<i32>,
}

impl UpdateProductInput {
    /// True when the input changes the product document rather than only its lifecycle.
    ///
    /// Lifecycle-only writes (`status`) are repeatable state transitions; every other field is an
    /// editorial change and therefore needs the predecessor revision of the document that was read.
    pub fn changes_document(&self) -> bool {
        self.translations.is_some()
            || self.tags.is_some()
            || self.metadata.is_some()
            || self.seller_id.is_changed()
            || self.vendor.is_changed()
            || self.product_type.is_changed()
            || self.shipping_profile_slug.is_changed()
            || self.primary_category_id.is_changed()
    }

    /// Structured reason one externally reachable update is refused before it reaches the owner.
    pub fn missing_expected_revision(&self) -> bool {
        self.changes_document() && self.expected_revision.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProductResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub status: ProductStatus,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub fulfillment_requirement: ProductFulfillmentRequirement,
    pub shipping_profile_slug: Option<String>,
    pub primary_category_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Current editorial revision; send it back as `expected_revision` on the next update.
    pub revision: i32,
    pub translations: Vec<ProductTranslationResponse>,
    #[serde(default)]
    pub variant_axes: Vec<VariantAxisConfigResponse>,
    pub variants: Vec<VariantResponse>,
    pub images: Vec<ProductImageResponse>,
    /// Storefront-safe specifications: filled by the published storefront detail projection and
    /// empty on admin reads, which resolve attribute values through the catalog schema service.
    ///
    /// Every entry is already display-ready — localized label, ordered values, localized text or
    /// option label — so a storefront never joins `product_attribute*` tables or resolves option
    /// ids itself. Attributes that a cataloguer hid from the storefront, service attributes of
    /// other scopes and JSON payloads are absent by construction.
    #[serde(default)]
    pub storefront_attributes: Vec<StorefrontProductAttributeResponse>,
}

/// One display-ready value of a storefront attribute.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StorefrontProductAttributeValueResponse {
    /// Localized text, option label or formatted number/date. Booleans keep the `true`/`false`
    /// vocabulary so each storefront renders its own localized yes/no copy.
    pub text: String,
}

/// One storefront-safe product attribute with its resolved values.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StorefrontProductAttributeResponse {
    pub code: String,
    /// Localized attribute label (`product_attribute_translations`) with the attribute code as the
    /// last resort.
    pub label: String,
    /// Stored value type, e.g. `select`; drives storefront-side presentation of booleans.
    pub value_type: String,
    pub is_localized: bool,
    pub values: Vec<StorefrontProductAttributeValueResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProductTranslationResponse {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProductImageResponse {
    pub id: Uuid,
    pub media_id: Uuid,
    pub url: String,
    pub alt_text: Option<String>,
    pub position: i32,
    #[serde(default)]
    pub translations: Vec<ProductImageTranslationResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProductImageTranslationResponse {
    pub locale: String,
    pub alt_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AddProductImageInput {
    pub media_id: Uuid,
    pub position: Option<i32>,
    pub alt_text: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateProductImageInput {
    pub position: Option<i32>,
    pub alt_text: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PriceResponse {
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub on_sale: bool,
}

#[cfg(test)]
mod tests {
    use super::ProductTranslationInput;
    use serde_json::json;
    use validator::Validate;

    #[test]
    fn product_translation_input_uses_tenant_locale_contract() {
        let input: ProductTranslationInput = serde_json::from_value(json!({
            "locale": " pt_br ",
            "title": "Title",
            "handle": null,
            "description": null,
            "meta_title": null,
            "meta_description": null
        }))
        .expect("canonical tenant locale");

        assert_eq!(input.locale, "pt-BR");
        assert!(input.validate().is_ok());

        let invalid: Result<ProductTranslationInput, _> = serde_json::from_value(json!({
            "locale": "und",
            "title": "Title",
            "handle": null,
            "description": null,
            "meta_title": null,
            "meta_description": null
        }));
        assert!(invalid.is_err());

        let direct = ProductTranslationInput {
            locale: "und".to_string(),
            title: "Title".to_string(),
            handle: None,
            description: None,
            meta_title: None,
            meta_description: None,
        };
        assert!(direct.validate().is_err());

        let noncanonical_direct = ProductTranslationInput {
            locale: "pt_br".to_string(),
            title: "Title".to_string(),
            handle: None,
            description: None,
            meta_title: None,
            meta_description: None,
        };
        assert!(noncanonical_direct.validate().is_err());
    }
}
