use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    CartResponse, FulfillmentResponse, OrderResponse, PaymentCollectionResponse,
    StoreContextResponse,
};

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CompleteCheckoutInput {
    pub cart_id: Uuid,
    pub shipping_option_id: Option<Uuid>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    #[serde(default = "default_true")]
    pub create_fulfillment: bool,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteCheckoutResponse {
    pub cart: CartResponse,
    pub order: OrderResponse,
    pub payment_collection: PaymentCollectionResponse,
    pub fulfillment: Option<FulfillmentResponse>,
    pub context: StoreContextResponse,
}

const fn default_true() -> bool {
    true
}
