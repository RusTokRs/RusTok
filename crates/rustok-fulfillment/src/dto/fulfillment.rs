use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateShippingOptionInput {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(length(equal = 3))]
    pub currency_code: String,
    pub amount: Decimal,
    #[validate(length(min = 1, max = 100))]
    pub provider_id: Option<String>,
    pub allowed_shipping_profile_slugs: Option<Vec<String>>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateShippingOptionInput {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    #[validate(length(equal = 3))]
    pub currency_code: Option<String>,
    pub amount: Option<Decimal>,
    #[validate(length(min = 1, max = 100))]
    pub provider_id: Option<String>,
    pub allowed_shipping_profile_slugs: Option<Vec<String>>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListFulfillmentsInput {
    pub page: u64,
    pub per_page: u64,
    pub status: Option<String>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateFulfillmentInput {
    pub order_id: Uuid,
    pub shipping_option_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    #[validate(length(max = 100))]
    pub carrier: Option<String>,
    #[validate(length(max = 100))]
    pub tracking_number: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ShipFulfillmentInput {
    #[validate(length(min = 1, max = 100))]
    pub carrier: String,
    #[validate(length(min = 1, max = 100))]
    pub tracking_number: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DeliverFulfillmentInput {
    pub delivered_note: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CancelFulfillmentInput {
    pub reason: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ShippingOptionResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub currency_code: String,
    pub amount: Decimal,
    pub provider_id: String,
    pub active: bool,
    pub allowed_shipping_profile_slugs: Option<Vec<String>>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FulfillmentResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub shipping_option_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub status: String,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub delivered_note: Option<String>,
    pub cancellation_reason: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
}
