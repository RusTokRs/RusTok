use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOrderInput {
    pub customer_id: Option<Uuid>,
    #[validate(custom(function = "validate_currency_code"))]
    pub currency_code: String,
    #[serde(default)]
    #[validate(custom(function = "validate_non_negative_decimal"))]
    pub shipping_total: Decimal,
    #[validate(length(min = 1))]
    #[validate(nested)]
    pub line_items: Vec<CreateOrderLineItemInput>,
    #[serde(default)]
    #[validate(nested)]
    pub adjustments: Vec<CreateOrderAdjustmentInput>,
    #[serde(default)]
    #[validate(nested)]
    pub tax_lines: Vec<CreateOrderTaxLineInput>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOrderLineItemInput {
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    #[validate(length(min = 1, max = 100))]
    pub shipping_profile_slug: String,
    #[validate(length(max = 100))]
    pub seller_id: Option<String>,
    #[validate(length(max = 100))]
    pub sku: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub title: String,
    #[validate(range(min = 1))]
    pub quantity: i32,
    #[validate(custom(function = "validate_non_negative_decimal"))]
    pub unit_price: Decimal,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOrderAdjustmentInput {
    pub line_item_index: Option<usize>,
    #[validate(length(min = 1, max = 64))]
    pub source_type: String,
    #[validate(length(max = 191))]
    pub source_id: Option<String>,
    #[validate(custom(function = "validate_non_negative_decimal"))]
    pub amount: Decimal,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOrderTaxLineInput {
    pub line_item_index: Option<usize>,
    pub shipping_option_id: Option<Uuid>,
    #[validate(length(max = 255))]
    pub description: Option<String>,
    #[validate(length(min = 1, max = 64))]
    pub provider_id: String,
    #[validate(custom(function = "validate_non_negative_decimal"))]
    pub rate: Decimal,
    #[validate(custom(function = "validate_non_negative_decimal"))]
    pub amount: Decimal,
    #[validate(custom(function = "validate_currency_code"))]
    pub currency_code: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOrderChangeInput {
    #[validate(length(min = 1, max = 64))]
    pub change_type: String,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    pub preview: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ApplyOrderChangeInput {
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CancelOrderChangeInput {
    #[validate(length(max = 255))]
    pub reason: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListOrderChangesInput {
    pub page: u64,
    pub per_page: u64,
    pub order_id: Option<Uuid>,
    pub status: Option<String>,
    pub change_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOrderReturnInput {
    #[validate(length(max = 255))]
    pub reason: Option<String>,
    #[validate(length(max = 2000))]
    pub note: Option<String>,
    #[serde(default)]
    #[validate(nested)]
    pub items: Vec<CreateOrderReturnItemInput>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOrderReturnItemInput {
    pub line_item_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
    #[validate(length(max = 255))]
    pub reason: Option<String>,
    #[validate(length(max = 2000))]
    pub note: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CompleteOrderReturnInput {
    #[validate(length(max = 64))]
    pub resolution_type: Option<String>,
    pub refund_id: Option<Uuid>,
    pub order_change_id: Option<Uuid>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CancelOrderReturnInput {
    #[validate(length(max = 255))]
    pub reason: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListOrderReturnsInput {
    pub page: u64,
    pub per_page: u64,
    pub order_id: Option<Uuid>,
    pub status: Option<String>,
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
    pub subtotal_amount: Decimal,
    pub adjustment_total: Decimal,
    pub shipping_total: Decimal,
    pub total_amount: Decimal,
    pub tax_total: Decimal,
    pub tax_included: bool,
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
    pub adjustments: Vec<OrderAdjustmentResponse>,
    pub tax_lines: Vec<OrderTaxLineResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderLineItemResponse {
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub sku: Option<String>,
    pub title: String,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub total_price: Decimal,
    pub currency_code: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderAdjustmentResponse {
    pub id: Uuid,
    pub order_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub amount: Decimal,
    pub currency_code: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderTaxLineResponse {
    pub id: Uuid,
    pub order_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub description: Option<String>,
    pub provider_id: String,
    pub rate: Decimal,
    pub amount: Decimal,
    pub currency_code: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderChangeResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub created_by: Uuid,
    pub change_type: String,
    pub status: String,
    pub description: Option<String>,
    pub preview: Value,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderReturnResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub reason: Option<String>,
    pub note: Option<String>,
    pub status: String,
    pub resolution_type: Option<String>,
    pub refund_id: Option<Uuid>,
    pub order_change_id: Option<Uuid>,
    pub metadata: Value,
    pub items: Vec<OrderReturnItemResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderReturnItemResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub return_id: Uuid,
    pub order_id: Uuid,
    pub line_item_id: Uuid,
    pub quantity: i32,
    pub reason: Option<String>,
    pub note: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn validate_currency_code(value: &str) -> Result<(), ValidationError> {
    let value = value.trim();
    if value.len() == 3 && value.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Ok(())
    } else {
        Err(ValidationError::new("currency_code"))
    }
}

fn validate_non_negative_decimal(value: &Decimal) -> Result<(), ValidationError> {
    if *value >= Decimal::ZERO {
        Ok(())
    } else {
        Err(ValidationError::new("non_negative_decimal"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_order() -> CreateOrderInput {
        CreateOrderInput {
            customer_id: None,
            currency_code: "USD".to_string(),
            shipping_total: Decimal::ZERO,
            line_items: vec![CreateOrderLineItemInput {
                product_id: None,
                variant_id: None,
                shipping_profile_slug: "default".to_string(),
                seller_id: None,
                sku: None,
                title: "Item".to_string(),
                quantity: 1,
                unit_price: Decimal::ONE,
                metadata: Value::Null,
            }],
            adjustments: Vec::new(),
            tax_lines: Vec::new(),
            metadata: Value::Null,
        }
    }

    #[test]
    fn rejects_non_alphabetic_currency() {
        let mut input = valid_order();
        input.currency_code = "12$".to_string();
        assert!(input.validate().is_err());
    }

    #[test]
    fn validates_nested_line_items() {
        let mut input = valid_order();
        input.line_items[0].quantity = 0;
        assert!(input.validate().is_err());
    }

    #[test]
    fn rejects_negative_money() {
        let mut input = valid_order();
        input.shipping_total = -Decimal::ONE;
        assert!(input.validate().is_err());
    }
}
