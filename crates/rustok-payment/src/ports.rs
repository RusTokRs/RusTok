use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortContext, PortError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::PaymentCollectionResponse;

/// Transport-neutral owner boundary for payment collection create/reuse flows.
#[async_trait]
pub trait PaymentCollectionPort: Send + Sync {
    async fn create_or_reuse_collection(
        &self,
        context: PortContext,
        request: PaymentCollectionCreateOrReuseRequest,
    ) -> Result<PaymentCollectionResponse, PortError>;

    async fn read_collection_status(
        &self,
        context: PortContext,
        request: PaymentCollectionStatusRequest,
    ) -> Result<PaymentCollectionStatusSnapshot, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentCollectionCreateOrReuseRequest {
    pub cart_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub currency_code: String,
    pub amount: Decimal,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentCollectionStatusRequest {
    pub collection_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentCollectionStatusSnapshot {
    pub collection_id: Uuid,
    pub status: String,
    pub currency_code: String,
    pub amount: Decimal,
    pub authorized_amount: Decimal,
    pub captured_amount: Decimal,
    pub provider_id: Option<String>,
}

impl PaymentCollectionStatusSnapshot {
    pub fn from_response(response: &PaymentCollectionResponse) -> Self {
        Self {
            collection_id: response.id,
            status: response.status.clone(),
            currency_code: response.currency_code.clone(),
            amount: response.amount,
            authorized_amount: response.authorized_amount,
            captured_amount: response.captured_amount,
            provider_id: response.provider_id.clone(),
        }
    }
}
