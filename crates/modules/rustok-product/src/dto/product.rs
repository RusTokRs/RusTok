use rust_decimal::Decimal;
use rustok_api::{Patch, TenantLocale};
use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::{Validate, ValidationError};

use super::{
    CreateVariantInput, VariantAxisConfigResponse, VariantAxisInput, VariantResponse,
};
use crate::domain::ProductStatus;

fn validate_patch_string(value: &Patch<String>, min: usize, max: usize, field: &'static str) -> Result<(), ValidationError> {
    if let Patch::Set(value) = value {
        let len = value.chars().count();
        if len < min || len > max {
            let mut error = ValidationError::new(field);
            error.add_param("min", &min);
            error.add_param("max", &max);
            return Err(error);
        }
    }
    Ok(())
}

fn validate_patch_seller(value: &Patch<String>) -> Result<(), ValidationError> {
    validate_patch_string(value, 0, 100, "seller_id")
}

fn validate_patch_vendor(value: &Patch<String>) -> Result<(), ValidationError> {
    validate_patch_string(value, 0, 255, "vendor")
}

fn validate_patch_product_type(value: &Patch<String>) -> Result<(), ValidationError> {
    validate_patch_string(value, 0, 255, "product_type")
}

fn validate_patch_shipping_profile(value: &Patch<String>) -> Result<(), ValidationError> {
    validate_patch_string(value, 1, 64, "shipping_profile_slug")
}

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
    #[validate(custom(function = "validate_patch_seller"))]
    pub seller_id: Option<String>,
    #[validate(custom(function = "validate_patch_vendor"))]
    pub vendor: Option<String>,
    #[validate(custom(function = "validate_patch_product_type"))]
    pub product_type: Option<String>,
    #[validate(custom(function = "validate_patch_shipping_profile"))]
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
    #[validate(length(max = 100, message = "Seller ID must be max 100 characters"))]
    pub seller_id: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[validate(length(max = 255, message = "Vendor must be max 255 characters"))]
    pub vendor: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[validate(length(max = 255, message = "Product type must be max 255 characters"))]
    pub product_type: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[validate(length(
        min = 1,
        max = 64,
        message = "Shipping profile slug must be 1-64 characters"
    ))]
    pub shipping_profile_slug: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    pub primary_category_id: Patch<Uuid>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
    pub status: Option<ProductStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProductResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub status: ProductStatus,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub primary_category_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub translations: Vec<ProductTranslationResponse>,
    #[serde(default)]
    pub variant_axes: Vec<VariantAxisConfigResponse>,
    pub variants: Vec<VariantResponse>,
    pub images: Vec<ProductImageResponse>,
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
    fn update_product_nullable_fields_use_explicit_patch_semantics() {
        let keep: UpdateProductInput = serde_json::from_value(json!({})).expect("keep");
        assert!(keep.seller_id.is_keep());
        assert!(keep.vendor.is_keep());
        assert!(keep.product_type.is_keep());
        assert!(keep.shipping_profile_slug.is_keep());
        assert!(keep.primary_category_id.is_keep());

        let clear: UpdateProductInput = serde_json::from_value(json!({
            "seller_id": null,
            "vendor": null,
            "product_type": null,
            "shipping_profile_slug": null,
            "primary_category_id": null
        })).expect("clear");
        assert!(matches!(clear.seller_id, Patch::Clear));
        assert!(matches!(clear.vendor, Patch::Clear));
        assert!(matches!(clear.product_type, Patch::Clear));
        assert!(matches!(clear.shipping_profile_slug, Patch::Clear));
        assert!(matches!(clear.primary_category_id, Patch::Clear));
    }

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
