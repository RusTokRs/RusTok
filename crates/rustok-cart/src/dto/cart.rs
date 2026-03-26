use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateCartInput {
    pub customer_id: Option<Uuid>,
    #[validate(length(max = 255))]
    pub email: Option<String>,
    pub region_id: Option<Uuid>,
    #[validate(length(equal = 2))]
    pub country_code: Option<String>,
    #[validate(length(min = 2, max = 10))]
    pub locale_code: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
    #[validate(length(equal = 3))]
    pub currency_code: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct AddCartLineItemInput {
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    #[validate(length(max = 100))]
    pub sku: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub title: String,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub unit_price: Decimal,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateCartContextInput {
    #[validate(length(max = 255))]
    pub email: Option<String>,
    pub region_id: Option<Uuid>,
    #[validate(length(equal = 2))]
    pub country_code: Option<String>,
    #[validate(length(min = 2, max = 10))]
    pub locale_code: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CartResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub email: Option<String>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale_code: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
    pub status: String,
    pub currency_code: String,
    pub total_amount: Decimal,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub line_items: Vec<CartLineItemResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CartLineItemResponse {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    pub sku: Option<String>,
    pub title: String,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub total_price: Decimal,
    pub currency_code: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
