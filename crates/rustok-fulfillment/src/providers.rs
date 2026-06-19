use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{FulfillmentError, FulfillmentResult};

/// Stable identifier of the built-in manual fulfillment provider.
pub const MANUAL_FULFILLMENT_PROVIDER_ID: &str = "manual";

/// Provider capabilities advertised to checkout orchestration and admin tooling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FulfillmentProviderCapabilities {
    pub rate_quote: bool,
    pub create_label: bool,
    pub ship: bool,
    pub cancel: bool,
    pub tracking_webhook_ingress: bool,
}

impl FulfillmentProviderCapabilities {
    pub const fn manual() -> Self {
        Self {
            rate_quote: false,
            create_label: false,
            ship: true,
            cancel: true,
            tracking_webhook_ingress: false,
        }
    }
}

/// Registry entry for a fulfillment/carrier provider implementation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FulfillmentProviderDescriptor {
    pub provider_id: String,
    pub display_name: String,
    pub capabilities: FulfillmentProviderCapabilities,
    pub default_for_manual_options: bool,
}

impl FulfillmentProviderDescriptor {
    pub fn manual() -> Self {
        Self {
            provider_id: MANUAL_FULFILLMENT_PROVIDER_ID.to_string(),
            display_name: "Manual fulfillment".to_string(),
            capabilities: FulfillmentProviderCapabilities::manual(),
            default_for_manual_options: true,
        }
    }
}

/// Transport-neutral rate quote request for carrier/provider adapters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FulfillmentRateQuoteRequest {
    pub tenant_id: Uuid,
    pub cart_id: Option<Uuid>,
    pub seller_id: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub currency_code: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FulfillmentRateQuote {
    pub provider_id: String,
    pub service_code: String,
    pub amount: Decimal,
    pub currency_code: String,
    pub metadata: Value,
}

/// Transport-neutral request passed to provider adapters for shipment operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FulfillmentProviderOperationRequest {
    pub tenant_id: Uuid,
    pub fulfillment_id: Uuid,
    pub idempotency_key: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FulfillmentProviderOperationResult {
    pub provider_id: String,
    pub external_reference: Option<String>,
    pub tracking_number: Option<String>,
    pub metadata: Value,
}

/// Transport-neutral carrier webhook delivery passed to provider adapters before
/// replay-safe lifecycle handling is delegated back to `FulfillmentService`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FulfillmentProviderWebhookRequest {
    pub tenant_id: Uuid,
    pub provider_id: String,
    pub delivery_id: String,
    pub idempotency_key: String,
    pub signature: Option<String>,
    pub raw_payload: Vec<u8>,
}

/// Normalized carrier webhook facts. Adapters must not persist fulfillment state directly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FulfillmentProviderWebhookResult {
    pub provider_id: String,
    pub external_reference: Option<String>,
    pub event_type: String,
    pub replay_key: String,
    pub tracking_number: Option<String>,
    pub metadata: Value,
}

/// SPI for concrete fulfillment providers. Fulfillment lifecycle state remains owned by
/// `FulfillmentService`; adapters only execute provider/carrier side effects.
#[async_trait]
pub trait FulfillmentProvider: Send + Sync {
    fn descriptor(&self) -> FulfillmentProviderDescriptor;

    async fn quote_rates(
        &self,
        request: FulfillmentRateQuoteRequest,
    ) -> FulfillmentResult<Vec<FulfillmentRateQuote>>;

    async fn create_label(
        &self,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult>;

    async fn cancel(
        &self,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult>;

    async fn handle_tracking_webhook(
        &self,
        request: FulfillmentProviderWebhookRequest,
    ) -> FulfillmentResult<FulfillmentProviderWebhookResult>;
}

/// Built-in manual provider used while external carrier integrations are not connected.
#[derive(Debug, Default, Clone, Copy)]
pub struct ManualFulfillmentProvider;

impl ManualFulfillmentProvider {
    fn result(request: FulfillmentProviderOperationRequest) -> FulfillmentProviderOperationResult {
        FulfillmentProviderOperationResult {
            provider_id: MANUAL_FULFILLMENT_PROVIDER_ID.to_string(),
            external_reference: None,
            tracking_number: None,
            metadata: request.metadata,
        }
    }
}

#[async_trait]
impl FulfillmentProvider for ManualFulfillmentProvider {
    fn descriptor(&self) -> FulfillmentProviderDescriptor {
        FulfillmentProviderDescriptor::manual()
    }

    async fn quote_rates(
        &self,
        _request: FulfillmentRateQuoteRequest,
    ) -> FulfillmentResult<Vec<FulfillmentRateQuote>> {
        Err(FulfillmentError::Validation(
            "manual fulfillment provider does not quote dynamic rates".to_string(),
        ))
    }

    async fn create_label(
        &self,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        Ok(Self::result(request))
    }

    async fn cancel(
        &self,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        Ok(Self::result(request))
    }

    async fn handle_tracking_webhook(
        &self,
        _request: FulfillmentProviderWebhookRequest,
    ) -> FulfillmentResult<FulfillmentProviderWebhookResult> {
        Err(FulfillmentError::Validation(
            "manual fulfillment provider does not accept tracking webhook ingress".to_string(),
        ))
    }
}
