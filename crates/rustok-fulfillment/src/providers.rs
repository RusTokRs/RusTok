use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
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

/// Health state reported by external carrier registration. Runtime orchestration
/// maps non-ready states to degraded fulfillment modes before invoking providers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FulfillmentProviderHealth {
    Ready,
    Degraded,
    Unavailable,
}

/// Registration-time degraded mode used by hosts to keep fulfillment policy explicit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FulfillmentProviderDegradedMode {
    pub reason: String,
    pub fallback_profile: String,
}

/// Runtime decision for fulfillment orchestration before a provider side effect is invoked.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FulfillmentProviderRuntimeMode {
    pub provider_id: String,
    pub operation: String,
    pub can_execute: bool,
    pub degraded_mode: Option<FulfillmentProviderDegradedMode>,
}

/// External carrier registration contract. The adapter remains side-effect-only:
/// lifecycle persistence and replay/idempotency decisions stay in `FulfillmentService`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalFulfillmentProviderRegistration {
    pub descriptor: FulfillmentProviderDescriptor,
    pub health: FulfillmentProviderHealth,
    pub degraded_mode: Option<FulfillmentProviderDegradedMode>,
}

impl ExternalFulfillmentProviderRegistration {
    pub fn validate(&self, expected_provider_id: &str) -> FulfillmentResult<()> {
        if self.descriptor.provider_id != expected_provider_id {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider descriptor id `{}` does not match registration id `{}`",
                self.descriptor.provider_id, expected_provider_id
            )));
        }

        if self.descriptor.default_for_manual_options
            && self.health == FulfillmentProviderHealth::Unavailable
        {
            return Err(FulfillmentError::Validation(
                "unavailable fulfillment provider cannot be default for manual options".to_string(),
            ));
        }

        if self.health != FulfillmentProviderHealth::Ready && self.degraded_mode.is_none() {
            return Err(FulfillmentError::Validation(
                "non-ready fulfillment provider registration must declare degraded mode"
                    .to_string(),
            ));
        }

        Ok(())
    }
}

/// In-memory provider registry assembled by host composition before fulfillment runtime.
///
/// The registry keeps carrier lookup explicit and validates external registrations
/// before an adapter can become visible to orchestration code. It does not persist
/// lifecycle state and does not choose degraded fulfillment policy by itself.
#[derive(Clone, Default)]
pub struct FulfillmentProviderRegistry {
    providers: HashMap<String, Arc<dyn FulfillmentProvider>>,
    registrations: HashMap<String, ExternalFulfillmentProviderRegistration>,
}

impl FulfillmentProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_manual_provider() -> Self {
        let mut registry = Self::new();
        registry.register_builtin(Arc::new(ManualFulfillmentProvider));
        registry
    }

    pub fn register_builtin(&mut self, provider: Arc<dyn FulfillmentProvider>) {
        let descriptor = provider.descriptor();
        self.providers
            .insert(descriptor.provider_id.clone(), provider);
        self.registrations.insert(
            descriptor.provider_id.clone(),
            ExternalFulfillmentProviderRegistration {
                descriptor,
                health: FulfillmentProviderHealth::Ready,
                degraded_mode: None,
            },
        );
    }

    pub fn register_external(
        &mut self,
        expected_provider_id: &str,
        provider: Arc<dyn FulfillmentProvider>,
        registration: ExternalFulfillmentProviderRegistration,
    ) -> FulfillmentResult<()> {
        registration.validate(expected_provider_id)?;
        let descriptor = provider.descriptor();
        if descriptor.provider_id != registration.descriptor.provider_id {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider adapter id `{}` does not match descriptor id `{}`",
                descriptor.provider_id, registration.descriptor.provider_id
            )));
        }
        if self.providers.contains_key(expected_provider_id) {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider `{}` is already registered",
                expected_provider_id
            )));
        }

        self.providers
            .insert(expected_provider_id.to_string(), provider);
        self.registrations
            .insert(expected_provider_id.to_string(), registration);
        Ok(())
    }

    pub fn provider(&self, provider_id: &str) -> Option<Arc<dyn FulfillmentProvider>> {
        self.providers.get(provider_id).cloned()
    }

    pub fn registration(
        &self,
        provider_id: &str,
    ) -> Option<&ExternalFulfillmentProviderRegistration> {
        self.registrations.get(provider_id)
    }

    pub fn descriptors(&self) -> Vec<FulfillmentProviderDescriptor> {
        let mut descriptors = self
            .registrations
            .values()
            .map(|registration| registration.descriptor.clone())
            .collect::<Vec<_>>();
        descriptors.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        descriptors
    }

    /// Resolve runtime execution mode for an operation without invoking the adapter.
    ///
    /// This is intentionally side-effect-free: callers use it to map unavailable or
    /// degraded carrier providers into explicit fallback/degraded fulfillment modes,
    /// while lifecycle persistence remains in `FulfillmentService`.
    pub fn runtime_mode(
        &self,
        provider_id: &str,
        operation: &str,
    ) -> FulfillmentResult<FulfillmentProviderRuntimeMode> {
        let registration = self.registrations.get(provider_id).ok_or_else(|| {
            FulfillmentError::Validation(format!(
                "fulfillment provider `{}` is not registered",
                provider_id
            ))
        })?;
        let supported = match operation {
            "rate_quote" => registration.descriptor.capabilities.rate_quote,
            "create_label" => registration.descriptor.capabilities.create_label,
            "ship" => registration.descriptor.capabilities.ship,
            "cancel" => registration.descriptor.capabilities.cancel,
            "tracking_webhook_ingress" => {
                registration
                    .descriptor
                    .capabilities
                    .tracking_webhook_ingress
            }
            other => {
                return Err(FulfillmentError::Validation(format!(
                    "unknown fulfillment provider operation `{}`",
                    other
                )))
            }
        };

        if !supported {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider `{}` does not support `{}`",
                provider_id, operation
            )));
        }

        Ok(FulfillmentProviderRuntimeMode {
            provider_id: provider_id.to_string(),
            operation: operation.to_string(),
            can_execute: registration.health != FulfillmentProviderHealth::Unavailable,
            degraded_mode: registration.degraded_mode.clone(),
        })
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
