use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
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

/// Health state reported by external adapter registration. Runtime orchestration
/// maps non-ready states to degraded checkout modes before invoking providers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaymentProviderHealth {
    Ready,
    Degraded,
    Unavailable,
}

/// Registration-time degraded mode used by hosts to keep checkout policy explicit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentProviderDegradedMode {
    pub reason: String,
    pub fallback_profile: String,
}

/// Runtime decision for checkout/payment orchestration before a provider side effect is invoked.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentProviderRuntimeMode {
    pub provider_id: String,
    pub operation: String,
    pub can_execute: bool,
    pub degraded_mode: Option<PaymentProviderDegradedMode>,
}

/// External provider registration contract. The adapter remains side-effect-only:
/// lifecycle persistence and replay/idempotency decisions stay in `PaymentService`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalPaymentProviderRegistration {
    pub descriptor: PaymentProviderDescriptor,
    pub health: PaymentProviderHealth,
    pub degraded_mode: Option<PaymentProviderDegradedMode>,
}

impl ExternalPaymentProviderRegistration {
    pub fn validate(&self, expected_provider_id: &str) -> PaymentResult<()> {
        if self.descriptor.provider_id != expected_provider_id {
            return Err(PaymentError::Validation(format!(
                "payment provider descriptor id `{}` does not match registration id `{}`",
                self.descriptor.provider_id, expected_provider_id
            )));
        }

        if self.descriptor.default_for_new_collections
            && self.health == PaymentProviderHealth::Unavailable
        {
            return Err(PaymentError::Validation(
                "unavailable payment provider cannot be default for new collections".to_string(),
            ));
        }

        if self.health != PaymentProviderHealth::Ready && self.degraded_mode.is_none() {
            return Err(PaymentError::Validation(
                "non-ready payment provider registration must declare degraded mode".to_string(),
            ));
        }

        Ok(())
    }
}

/// In-memory provider registry assembled by host composition before checkout runtime.
///
/// The registry keeps adapter lookup explicit and validates external registrations
/// before an adapter can become visible to orchestration code. It does not persist
/// lifecycle state and does not choose checkout fallback policy by itself.
#[derive(Clone, Default)]
pub struct PaymentProviderRegistry {
    providers: HashMap<String, Arc<dyn PaymentProvider>>,
    registrations: HashMap<String, ExternalPaymentProviderRegistration>,
}

impl PaymentProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_manual_provider() -> Self {
        let mut registry = Self::new();
        registry.register_builtin(Arc::new(ManualPaymentProvider));
        registry
    }

    pub fn register_builtin(&mut self, provider: Arc<dyn PaymentProvider>) {
        let descriptor = provider.descriptor();
        self.providers
            .insert(descriptor.provider_id.clone(), provider);
        self.registrations.insert(
            descriptor.provider_id.clone(),
            ExternalPaymentProviderRegistration {
                descriptor,
                health: PaymentProviderHealth::Ready,
                degraded_mode: None,
            },
        );
    }

    pub fn register_external(
        &mut self,
        expected_provider_id: &str,
        provider: Arc<dyn PaymentProvider>,
        registration: ExternalPaymentProviderRegistration,
    ) -> PaymentResult<()> {
        registration.validate(expected_provider_id)?;
        let descriptor = provider.descriptor();
        if descriptor.provider_id != registration.descriptor.provider_id {
            return Err(PaymentError::Validation(format!(
                "payment provider adapter id `{}` does not match descriptor id `{}`",
                descriptor.provider_id, registration.descriptor.provider_id
            )));
        }
        if self.providers.contains_key(expected_provider_id) {
            return Err(PaymentError::Validation(format!(
                "payment provider `{}` is already registered",
                expected_provider_id
            )));
        }

        self.providers
            .insert(expected_provider_id.to_string(), provider);
        self.registrations
            .insert(expected_provider_id.to_string(), registration);
        Ok(())
    }

    pub fn provider(&self, provider_id: &str) -> Option<Arc<dyn PaymentProvider>> {
        self.providers.get(provider_id).cloned()
    }

    pub fn registration(&self, provider_id: &str) -> Option<&ExternalPaymentProviderRegistration> {
        self.registrations.get(provider_id)
    }

    pub fn descriptors(&self) -> Vec<PaymentProviderDescriptor> {
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
    /// degraded external providers into explicit fallback/degraded checkout modes,
    /// while lifecycle persistence remains in `PaymentService`.
    pub fn runtime_mode(
        &self,
        provider_id: &str,
        operation: &str,
    ) -> PaymentResult<PaymentProviderRuntimeMode> {
        let registration = self.registrations.get(provider_id).ok_or_else(|| {
            PaymentError::Validation(format!(
                "payment provider `{}` is not registered",
                provider_id
            ))
        })?;
        let supported = match operation {
            "authorize" => registration.descriptor.capabilities.authorize,
            "capture" => registration.descriptor.capabilities.capture,
            "refund" => registration.descriptor.capabilities.refund,
            "cancel" => registration.descriptor.capabilities.cancel,
            "webhook_ingress" => registration.descriptor.capabilities.webhook_ingress,
            other => {
                return Err(PaymentError::Validation(format!(
                    "unknown payment provider operation `{}`",
                    other
                )))
            }
        };

        if !supported {
            return Err(PaymentError::Validation(format!(
                "payment provider `{}` does not support `{}`",
                provider_id, operation
            )));
        }

        Ok(PaymentProviderRuntimeMode {
            provider_id: provider_id.to_string(),
            operation: operation.to_string(),
            can_execute: registration.health != PaymentProviderHealth::Unavailable,
            degraded_mode: registration.degraded_mode.clone(),
        })
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
