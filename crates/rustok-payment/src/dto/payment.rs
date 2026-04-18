use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreatePaymentCollectionInput {
    pub cart_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    #[validate(length(equal = 3))]
    pub currency_code: String,
    pub amount: Decimal,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListPaymentCollectionsInput {
    pub page: u64,
    pub per_page: u64,
    pub status: Option<String>,
    pub order_id: Option<Uuid>,
    pub cart_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListRefundsInput {
    pub page: u64,
    pub per_page: u64,
    pub payment_collection_id: Option<Uuid>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct AuthorizePaymentInput {
    #[validate(length(min = 1, max = 100))]
    pub provider_id: Option<String>,
    #[validate(length(min = 1, max = 191))]
    pub provider_payment_id: Option<String>,
    pub amount: Option<Decimal>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CapturePaymentInput {
    pub amount: Option<Decimal>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CancelPaymentInput {
    pub reason: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateRefundInput {
    pub amount: Decimal,
    pub reason: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteRefundInput {
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CancelRefundInput {
    pub reason: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaymentCollectionResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub cart_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub status: String,
    pub currency_code: String,
    pub amount: Decimal,
    pub authorized_amount: Decimal,
    pub captured_amount: Decimal,
    pub refunded_amount: Decimal,
    pub provider_id: Option<String>,
    pub cancellation_reason: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub authorized_at: Option<DateTime<Utc>>,
    pub captured_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub payments: Vec<PaymentResponse>,
    pub refunds: Vec<RefundResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaymentResponse {
    pub id: Uuid,
    pub payment_collection_id: Uuid,
    pub provider_id: String,
    pub provider_payment_id: String,
    pub status: String,
    pub currency_code: String,
    pub amount: Decimal,
    pub captured_amount: Decimal,
    pub error_message: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub authorized_at: Option<DateTime<Utc>>,
    pub captured_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RefundResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub payment_collection_id: Uuid,
    pub status: String,
    pub currency_code: String,
    pub amount: Decimal,
    pub reason: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub refunded_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
}
