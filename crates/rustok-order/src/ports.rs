use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortContext, PortError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::OrderResponse;

/// Transport-neutral owner boundary for checkout completion/result reads.
#[async_trait]
pub trait CheckoutCompletionPort: Send + Sync {
    async fn complete_checkout(
        &self,
        context: PortContext,
        request: CompleteCheckoutPortRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError>;

    async fn read_checkout_result(
        &self,
        context: PortContext,
        request: CheckoutResultRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError>;

    async fn read_order_status(
        &self,
        context: PortContext,
        request: OrderStatusRequest,
    ) -> Result<OrderStatusSnapshot, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompleteCheckoutPortRequest {
    pub cart_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub payment_collection_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub currency_code: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckoutResultRequest {
    pub cart_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderStatusRequest {
    pub order_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckoutCompletionSnapshot {
    pub order_id: Uuid,
    pub status: String,
    pub currency_code: String,
    pub total: Decimal,
    pub payment_collection_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderStatusSnapshot {
    pub order_id: Uuid,
    pub status: String,
    pub paid: bool,
    pub shipped: bool,
    pub delivered: bool,
    pub total_amount: Decimal,
}

impl OrderStatusSnapshot {
    pub fn from_response(response: &OrderResponse) -> Self {
        Self {
            order_id: response.id,
            status: response.status.clone(),
            paid: response.paid_at.is_some(),
            shipped: response.shipped_at.is_some(),
            delivered: response.delivered_at.is_some(),
            total_amount: response.total_amount,
        }
    }
}
