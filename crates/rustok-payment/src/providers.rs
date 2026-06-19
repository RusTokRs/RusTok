use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{PaymentError, PaymentResult};

/// Stable identifier of the built-in manual payment provider.
pub const MANUAL_PAYMENT_PROVIDER_ID: &str = "manual";

/// Provider capabilities advertised to checkout orchestration and admin tooling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentProviderCapabilities {
    pub authorize: bool,
    pub capture: bool,
    pub refund: bool,
    pub cancel: bool,
    pub webhook_ingress: bool,
}

impl PaymentProviderCapabilities {
    pub const fn manual() -> Self {
        Self {
            authorize: true,
            capture: true,
            refund: true,
            cancel: true,
            webhook_ingress: false,
        }
    }
}

/// Registry entry for a payment provider implementation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentProviderDescriptor {
    pub provider_id: String,
    pub display_name: String,
    pub capabilities: PaymentProviderCapabilities,
    pub default_for_new_collections: bool,
}

impl PaymentProviderDescriptor {
    pub fn manual() -> Self {
        Self {
            provider_id: MANUAL_PAYMENT_PROVIDER_ID.to_string(),
            display_name: "Manual payment".to_string(),
            capabilities: PaymentProviderCapabilities::manual(),
            default_for_new_collections: true,
        }
    }
}

/// Transport-neutral request passed to provider adapters for payment operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentProviderOperationRequest {
    pub tenant_id: Uuid,
    pub collection_id: Uuid,
    pub amount: Decimal,
    pub currency_code: String,
    pub idempotency_key: Option<String>,
    pub metadata: Value,
}

/// Provider operation result normalized before it is persisted by `PaymentService`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentProviderOperationResult {
    pub provider_id: String,
    pub external_reference: Option<String>,
    pub authorized_amount: Decimal,
    pub captured_amount: Decimal,
    pub metadata: Value,
}

/// Transport-neutral webhook delivery passed to provider adapters before replay-safe
/// lifecycle handling is delegated back to `PaymentService`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentProviderWebhookRequest {
    pub tenant_id: Uuid,
    pub provider_id: String,
    pub delivery_id: String,
    pub idempotency_key: String,
    pub signature: Option<String>,
    pub raw_payload: Vec<u8>,
}

/// Normalized webhook facts. Adapters must not persist lifecycle state directly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentProviderWebhookResult {
    pub provider_id: String,
    pub external_reference: Option<String>,
    pub event_type: String,
    pub replay_key: String,
    pub metadata: Value,
}

/// SPI for concrete payment providers. Domain state transitions stay in `PaymentService`;
/// adapters only execute provider-side effects and return normalized facts.
#[async_trait]
pub trait PaymentProvider: Send + Sync {
    fn descriptor(&self) -> PaymentProviderDescriptor;

    async fn authorize(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult>;

    async fn capture(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult>;

    async fn cancel(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult>;

    async fn refund(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult>;

    async fn handle_webhook(
        &self,
        request: PaymentProviderWebhookRequest,
    ) -> PaymentResult<PaymentProviderWebhookResult>;
}

/// Built-in manual provider used while external gateways are not connected.
#[derive(Debug, Default, Clone, Copy)]
pub struct ManualPaymentProvider;

impl ManualPaymentProvider {
    fn result(
        request: PaymentProviderOperationRequest,
        authorized_amount: Decimal,
        captured_amount: Decimal,
    ) -> PaymentProviderOperationResult {
        PaymentProviderOperationResult {
            provider_id: MANUAL_PAYMENT_PROVIDER_ID.to_string(),
            external_reference: None,
            authorized_amount,
            captured_amount,
            metadata: request.metadata,
        }
    }
}

#[async_trait]
impl PaymentProvider for ManualPaymentProvider {
    fn descriptor(&self) -> PaymentProviderDescriptor {
        PaymentProviderDescriptor::manual()
    }

    async fn authorize(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        Ok(Self::result(request.clone(), request.amount, Decimal::ZERO))
    }

    async fn capture(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        Ok(Self::result(
            request.clone(),
            request.amount,
            request.amount,
        ))
    }

    async fn cancel(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        Ok(Self::result(request, Decimal::ZERO, Decimal::ZERO))
    }

    async fn refund(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        if request.amount <= Decimal::ZERO {
            return Err(PaymentError::Validation(
                "refund amount must be greater than zero".to_string(),
            ));
        }
        Ok(Self::result(request, Decimal::ZERO, Decimal::ZERO))
    }

    async fn handle_webhook(
        &self,
        _request: PaymentProviderWebhookRequest,
    ) -> PaymentResult<PaymentProviderWebhookResult> {
        Err(PaymentError::Validation(
            "manual payment provider does not accept webhook ingress".to_string(),
        ))
    }
}
