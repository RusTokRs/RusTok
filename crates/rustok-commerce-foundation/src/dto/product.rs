use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::{CreateVariantInput, VariantResponse};
use crate::entities::product::ProductStatus;

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, Validate)]
pub struct CreateProductInput {
    #[validate(length(min = 1, message = "At least one translation required"))]
    #[validate(nested)]
    pub translations: Vec<ProductTranslationInput>,
    #[serde(default)]
    pub options: Vec<ProductOptionInput>,
    #[validate(nested)]
    pub variants: Vec<CreateVariantInput>,
    #[validate(length(max = 255, message = "Vendor must be max 255 characters"))]
    pub vendor: Option<String>,
    #[validate(length(max = 255, message = "Product type must be max 255 characters"))]
    pub product_type: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub publish: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct ProductTranslationInput {
    #[validate(length(
        min = 2,
        max = 5,
        message = "Locale must be 2-5 characters (e.g. 'en', 'en-US')"
    ))]
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
    #[validate(length(min = 1, max = 255, message = "Option name must be 1-255 characters"))]
    pub name: String,
    #[validate(length(min = 1, message = "At least one option value required"))]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, Validate)]
pub struct UpdateProductInput {
    #[validate(nested)]
    pub translations: Option<Vec<ProductTranslationInput>>,
    #[validate(length(max = 255, message = "Vendor must be max 255 characters"))]
    pub vendor: Option<String>,
    #[validate(length(max = 255, message = "Product type must be max 255 characters"))]
    pub product_type: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub status: Option<ProductStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProductResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub status: ProductStatus,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
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
