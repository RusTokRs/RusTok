use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOrderInput {
    pub customer_id: Option<Uuid>,
    #[validate(length(equal = 3))]
    pub currency_code: String,
    #[validate(length(min = 1))]
    pub line_items: Vec<CreateOrderLineItemInput>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOrderLineItemInput {
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    pub shipping_profile_slug: String,
    #[validate(length(max = 100))]
    pub sku: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub title: String,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub unit_price: Decimal,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListOrdersInput {
    pub page: u64,
    pub per_page: u64,
    pub status: Option<String>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct MarkPaidOrderInput {
    #[validate(length(min = 1, max = 191))]
    pub payment_id: String,
    #[validate(length(min = 1, max = 100))]
    pub payment_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ShipOrderInput {
    #[validate(length(min = 1, max = 100))]
    pub tracking_number: String,
    #[validate(length(min = 1, max = 100))]
    pub carrier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DeliverOrderInput {
    pub delivered_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CancelOrderInput {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub customer_id: Option<Uuid>,
    pub status: String,
    pub currency_code: String,
    pub total_amount: Decimal,
    pub metadata: Value,
    pub payment_id: Option<String>,
    pub payment_method: Option<String>,
    pub tracking_number: Option<String>,
    pub carrier: Option<String>,
    pub cancellation_reason: Option<String>,
    pub delivered_signature: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub paid_at: Option<DateTime<Utc>>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub line_items: Vec<OrderLineItemResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderLineItemResponse {
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    pub shipping_profile_slug: String,
    pub sku: Option<String>,
    pub title: String,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub total_price: Decimal,
    pub currency_code: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}
