use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PaymentCollectionStatusKind {
    Pending,
    Authorized,
    Captured,
    Cancelled,
    Unknown,
}

impl PaymentCollectionStatusKind {
    pub fn from_raw(status: &str) -> Self {
        match status {
            "pending" => Self::Pending,
            "authorized" => Self::Authorized,
            "captured" => Self::Captured,
            "cancelled" => Self::Cancelled,
            _ => Self::Unknown,
        }
    }

    pub const fn can_authorize(self) -> bool {
        matches!(self, Self::Pending)
    }

    pub const fn can_capture(self) -> bool {
        matches!(self, Self::Authorized)
    }

    pub const fn is_authorized_or_captured(self) -> bool {
        matches!(self, Self::Authorized | Self::Captured)
    }

    pub const fn is_captured(self) -> bool {
        matches!(self, Self::Captured)
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Captured | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatusKind {
    Pending,
    Authorized,
    Captured,
    Cancelled,
    Unknown,
}

impl PaymentStatusKind {
    pub fn from_raw(status: &str) -> Self {
        match status {
            "pending" => Self::Pending,
            "authorized" => Self::Authorized,
            "captured" => Self::Captured,
            "cancelled" => Self::Cancelled,
            _ => Self::Unknown,
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Captured | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RefundStatusKind {
    Pending,
    Refunded,
    Cancelled,
    Unknown,
}

impl RefundStatusKind {
    pub fn from_raw(status: &str) -> Self {
        match status {
            "pending" => Self::Pending,
            "refunded" => Self::Refunded,
            "cancelled" => Self::Cancelled,
            _ => Self::Unknown,
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Refunded | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreatePaymentCollectionInput {
    pub cart_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    #[validate(custom(function = "validate_currency_code"))]
    pub currency_code: String,
    #[validate(custom(function = "validate_positive_decimal"))]
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
    pub order_id: Option<Uuid>,
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

impl PaymentCollectionResponse {
    pub fn status_kind(&self) -> PaymentCollectionStatusKind {
        PaymentCollectionStatusKind::from_raw(self.status.as_str())
    }
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

impl PaymentResponse {
    pub fn status_kind(&self) -> PaymentStatusKind {
        PaymentStatusKind::from_raw(self.status.as_str())
    }
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

impl RefundResponse {
    pub fn status_kind(&self) -> RefundStatusKind {
        RefundStatusKind::from_raw(self.status.as_str())
    }
}

fn validate_currency_code(value: &str) -> Result<(), ValidationError> {
    let value = value.trim();
    if value.len() == 3 && value.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Ok(())
    } else {
        Err(ValidationError::new("currency_code"))
    }
}

fn validate_positive_decimal(value: &Decimal) -> Result<(), ValidationError> {
    if *value > Decimal::ZERO {
        Ok(())
    } else {
        Err(ValidationError::new("positive_decimal"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_collection() -> CreatePaymentCollectionInput {
        CreatePaymentCollectionInput {
            cart_id: Some(Uuid::new_v4()),
            order_id: None,
            customer_id: None,
            currency_code: "USD".to_string(),
            amount: Decimal::ONE,
            metadata: Value::Null,
        }
    }

    #[test]
    fn rejects_symbolic_currency_code() {
        let mut input = valid_collection();
        input.currency_code = "12$".to_string();
        assert!(input.validate().is_err());
    }

    #[test]
    fn rejects_non_positive_collection_amount() {
        let mut input = valid_collection();
        input.amount = Decimal::ZERO;
        assert!(input.validate().is_err());
        input.amount = -Decimal::ONE;
        assert!(input.validate().is_err());
    }

    #[test]
    fn collection_status_kind_is_typed_and_fail_closed() {
        assert_eq!(
            PaymentCollectionStatusKind::from_raw("authorized"),
            PaymentCollectionStatusKind::Authorized
        );
        assert!(PaymentCollectionStatusKind::Authorized.can_capture());
        assert!(PaymentCollectionStatusKind::Captured.is_terminal());
        assert_eq!(
            PaymentCollectionStatusKind::from_raw("legacy_external_state"),
            PaymentCollectionStatusKind::Unknown
        );
    }

    #[test]
    fn payment_and_refund_status_kinds_preserve_unknown_values() {
        assert_eq!(
            PaymentStatusKind::from_raw("captured"),
            PaymentStatusKind::Captured
        );
        assert_eq!(
            RefundStatusKind::from_raw("refunded"),
            RefundStatusKind::Refunded
        );
        assert_eq!(
            RefundStatusKind::from_raw("provider_custom"),
            RefundStatusKind::Unknown
        );
    }
}
