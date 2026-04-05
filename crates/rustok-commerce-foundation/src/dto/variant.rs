use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::PriceResponse;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct CreateVariantInput {
    #[validate(length(max = 100, message = "SKU must be max 100 characters"))]
    pub sku: Option<String>,
    #[validate(length(max = 100, message = "Barcode must be max 100 characters"))]
    pub barcode: Option<String>,
    #[validate(length(
        max = 100,
        message = "Shipping profile slug must be max 100 characters"
    ))]
    pub shipping_profile_slug: Option<String>,
    #[validate(length(max = 255, message = "Option value must be max 255 characters"))]
    pub option1: Option<String>,
    #[validate(length(max = 255, message = "Option value must be max 255 characters"))]
    pub option2: Option<String>,
    #[validate(length(max = 255, message = "Option value must be max 255 characters"))]
    pub option3: Option<String>,
    #[validate(nested)]
    pub prices: Vec<PriceInput>,
    #[serde(default)]
    pub inventory_quantity: i32,
    #[serde(default = "default_inventory_policy")]
    #[validate(length(
        min = 1,
        max = 32,
        message = "Inventory policy must be 1-32 characters"
    ))]
    pub inventory_policy: String,
    pub weight: Option<Decimal>,
    #[validate(length(max = 16, message = "Weight unit must be max 16 characters"))]
    pub weight_unit: Option<String>,
}

fn default_inventory_policy() -> String {
    "deny".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct PriceInput {
    #[validate(length(
        min = 3,
        max = 3,
        message = "Currency code must be exactly 3 characters (ISO 4217)"
    ))]
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct UpdateVariantInput {
    #[validate(length(max = 100, message = "SKU must be max 100 characters"))]
    pub sku: Option<String>,
    #[validate(length(max = 100, message = "Barcode must be max 100 characters"))]
    pub barcode: Option<String>,
    #[validate(length(
        max = 100,
        message = "Shipping profile slug must be max 100 characters"
    ))]
    pub shipping_profile_slug: Option<String>,
    #[validate(nested)]
    pub prices: Option<Vec<PriceInput>>,
    pub inventory_quantity: Option<i32>,
    #[validate(length(max = 32, message = "Inventory policy must be max 32 characters"))]
    pub inventory_policy: Option<String>,
    pub weight: Option<Decimal>,
    #[validate(length(max = 16, message = "Weight unit must be max 16 characters"))]
    pub weight_unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VariantResponse {
    pub id: Uuid,
    pub product_id: Uuid,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    #[serde(default)]
    pub translations: Vec<VariantTranslationResponse>,
    pub option1: Option<String>,
    pub option2: Option<String>,
    pub option3: Option<String>,
    pub prices: Vec<PriceResponse>,
    pub inventory_quantity: i32,
    pub inventory_policy: String,
    pub in_stock: bool,
    pub weight: Option<Decimal>,
    pub weight_unit: Option<String>,
    pub position: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VariantTranslationResponse {
    pub locale: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct AdjustInventoryInput {
    pub variant_id: Uuid,
    pub adjustment: i32,
    pub reason: Option<String>,
}
