use rust_decimal::Decimal;
use rustok_api::TenantLocale;
use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::{Validate, ValidationError};

use super::{CreateVariantInput, VariantResponse};
use crate::entities::product::ProductStatus;

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
    TenantLocale::new(locale)
        .map(|_| ())
        .map_err(|_| ValidationError::new("tenant_locale"))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, Validate)]
pub struct CreateProductInput {
    #[validate(length(min = 1, message = "At least one translation required"))]
    #[validate(nested)]
    pub translations: Vec<ProductTranslationInput>,
    #[serde(default)]
    pub options: Vec<ProductOptionInput>,
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct ProductOptionInput {
    #[validate(length(min = 1, message = "At least one option translation required"))]
    #[validate(nested)]
    pub translations: Vec<ProductOptionTranslationInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, Validate)]
pub struct UpdateProductInput {
    #[validate(nested)]
    pub translations: Option<Vec<ProductTranslationInput>>,
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
    pub options: Vec<ProductOptionResponse>,
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
pub struct ProductOptionResponse {
    pub id: Uuid,
    pub name: String,
    pub values: Vec<String>,
    pub position: i32,
    #[serde(default)]
    pub translations: Vec<ProductOptionTranslationResponse>,
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
pub struct ProductOptionTranslationResponse {
    pub locale: String,
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct ProductOptionTranslationInput {
    #[serde(deserialize_with = "deserialize_tenant_locale")]
    #[validate(custom(function = "validate_tenant_locale"))]
    pub locale: String,
    #[validate(length(min = 1, max = 255, message = "Option name must be 1-255 characters"))]
    pub name: String,
    #[validate(length(min = 1, message = "At least one option value required"))]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProductImageTranslationResponse {
    pub locale: String,
    pub alt_text: Option<String>,
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
    use super::{ProductOptionTranslationInput, ProductTranslationInput};
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
    }

    #[test]
    fn product_option_translation_input_uses_tenant_locale_contract() {
        let input: ProductOptionTranslationInput = serde_json::from_value(json!({
            "locale": "zh_hant_tw",
            "name": "Size",
            "values": ["Small"]
        }))
        .expect("canonical tenant locale");

        assert_eq!(input.locale, "zh-Hant-TW");
        assert!(input.validate().is_ok());

        let invalid: Result<ProductOptionTranslationInput, _> = serde_json::from_value(json!({
            "locale": "und",
            "name": "Size",
            "values": ["Small"]
        }));
        assert!(invalid.is_err());
    }
}
