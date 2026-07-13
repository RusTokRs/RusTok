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
        if expected_provider_id.trim().is_empty() {
            return Err(PaymentError::Validation(
                "payment provider registration id must not be empty".to_string(),
            ));
        }
        if self.descriptor.provider_id != expected_provider_id {
            return Err(PaymentError::Validation(format!(
                "payment provider descriptor id `{}` does not match registration id `{}`",
                self.descriptor.provider_id, expected_provider_id
            )));
        }
        if self.descriptor.display_name.trim().is_empty() {
            return Err(PaymentError::Validation(
                "payment provider display name must not be empty".to_string(),
            ));
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
        if let Some(mode) = &self.degraded_mode {
            if mode.reason.trim().is_empty() || mode.fallback_profile.trim().is_empty() {
                return Err(PaymentError::Validation(
                    "payment provider degraded mode requires reason and fallback_profile"
                        .to_string(),
                ));
            }
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
        if registration.descriptor.default_for_new_collections
            && self
                .registrations
                .values()
                .any(|existing| existing.descriptor.default_for_new_collections)
        {
            return Err(PaymentError::Validation(
                "only one payment provider may be default for new collections".to_string(),
            ));
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

    fn executable_provider(
        &self,
        provider_id: &str,
        operation: &str,
    ) -> PaymentResult<Arc<dyn PaymentProvider>> {
        let mode = self.runtime_mode(provider_id, operation)?;
        if !mode.can_execute {
            return Err(PaymentError::Validation(format!(
                "payment provider `{}` is unavailable for `{}`",
                provider_id, operation
            )));
        }
        self.provider(provider_id).ok_or_else(|| {
            PaymentError::Validation(format!(
                "payment provider `{}` is not registered",
                provider_id
            ))
        })
    }

    fn validate_operation_request(
        provider_id: &str,
        operation: &str,
        request: &PaymentProviderOperationRequest,
    ) -> PaymentResult<()> {
        if request.tenant_id.is_nil() {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` {operation} request has nil tenant_id"
            )));
        }
        if request.collection_id.is_nil() {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` {operation} request has nil collection_id"
            )));
        }
        if request.amount <= Decimal::ZERO {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` {operation} amount must be greater than zero"
            )));
        }
        let currency = request.currency_code.trim();
        if currency.len() != 3 || !currency.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` {operation} currency_code must be a 3-letter code"
            )));
        }
        if request
            .idempotency_key
            .as_deref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` {operation} idempotency_key must not be blank"
            )));
        }
        Ok(())
    }

    fn validate_operation_result(
        provider_id: &str,
        operation: &str,
        requested_amount: Decimal,
        result: &PaymentProviderOperationResult,
    ) -> PaymentResult<()> {
        if result.provider_id != provider_id {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` returned result for `{}`",
                result.provider_id
            )));
        }
        if result.authorized_amount < Decimal::ZERO || result.captured_amount < Decimal::ZERO {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` {operation} returned a negative amount"
            )));
        }
        if result.authorized_amount > requested_amount || result.captured_amount > requested_amount
        {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` {operation} returned an amount above the request"
            )));
        }
        if result.authorized_amount > Decimal::ZERO
            && result.captured_amount > result.authorized_amount
        {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` {operation} captured more than it authorized"
            )));
        }
        match operation {
            "authorize" if result.authorized_amount <= Decimal::ZERO => {
                return Err(PaymentError::Validation(format!(
                    "payment provider `{provider_id}` authorize returned no authorized amount"
                )))
            }
            "capture" if result.captured_amount <= Decimal::ZERO => {
                return Err(PaymentError::Validation(format!(
                    "payment provider `{provider_id}` capture returned no captured amount"
                )))
            }
            _ => {}
        }
        Ok(())
    }

    /// Guard and invoke a provider authorize operation. The registry performs the
    /// side-effect-free runtime-mode check before the adapter is called.
    pub async fn execute_authorize(
        &self,
        provider_id: &str,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        Self::validate_operation_request(provider_id, "authorize", &request)?;
        let requested_amount = request.amount;
        let result = self
            .executable_provider(provider_id, "authorize")?
            .authorize(request)
            .await?;
        Self::validate_operation_result(provider_id, "authorize", requested_amount, &result)?;
        Ok(result)
    }

    /// Guard and invoke a provider capture operation without persisting lifecycle state.
    pub async fn execute_capture(
        &self,
        provider_id: &str,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        Self::validate_operation_request(provider_id, "capture", &request)?;
        let requested_amount = request.amount;
        let result = self
            .executable_provider(provider_id, "capture")?
            .capture(request)
            .await?;
        Self::validate_operation_result(provider_id, "capture", requested_amount, &result)?;
        Ok(result)
    }

    /// Guard and invoke a provider cancel operation without persisting lifecycle state.
    pub async fn execute_cancel(
        &self,
        provider_id: &str,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        Self::validate_operation_request(provider_id, "cancel", &request)?;
        let requested_amount = request.amount;
        let result = self
            .executable_provider(provider_id, "cancel")?
            .cancel(request)
            .await?;
        Self::validate_operation_result(provider_id, "cancel", requested_amount, &result)?;
        Ok(result)
    }

    /// Guard and invoke a provider refund operation without persisting lifecycle state.
    pub async fn execute_refund(
        &self,
        provider_id: &str,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        Self::validate_operation_request(provider_id, "refund", &request)?;
        let requested_amount = request.amount;
        let result = self
            .executable_provider(provider_id, "refund")?
            .refund(request)
            .await?;
        Self::validate_operation_result(provider_id, "refund", requested_amount, &result)?;
        Ok(result)
    }

    /// Guard and invoke webhook normalization; replay-safe lifecycle handling remains in `PaymentService`.
    pub async fn execute_webhook(
        &self,
        provider_id: &str,
        request: PaymentProviderWebhookRequest,
    ) -> PaymentResult<PaymentProviderWebhookResult> {
        if request.tenant_id.is_nil() {
            return Err(PaymentError::Validation(
                "payment provider webhook request has nil tenant_id".to_string(),
            ));
        }
        if request.provider_id != provider_id {
            return Err(PaymentError::Validation(format!(
                "payment webhook provider `{}` does not match registry provider `{provider_id}`",
                request.provider_id
            )));
        }
        if request.delivery_id.trim().is_empty() || request.idempotency_key.trim().is_empty() {
            return Err(PaymentError::Validation(
                "payment provider webhook requires delivery_id and idempotency_key".to_string(),
            ));
        }
        if request.raw_payload.is_empty() {
            return Err(PaymentError::Validation(
                "payment provider webhook payload must not be empty".to_string(),
            ));
        }
        let expected_replay_key = request.idempotency_key.clone();
        let result = self
            .executable_provider(provider_id, "webhook_ingress")?
            .handle_webhook(request)
            .await?;
        if result.provider_id != provider_id {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` returned webhook result for `{}`",
                result.provider_id
            )));
        }
        if result.event_type.trim().is_empty() || result.replay_key != expected_replay_key {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` returned an invalid webhook event or replay key"
            )));
        }
        Ok(result)
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

#[cfg(test)]
mod boundary_tests {
    use super::*;

    fn valid_result() -> PaymentProviderOperationResult {
        PaymentProviderOperationResult {
            provider_id: "gateway".to_string(),
            external_reference: None,
            authorized_amount: Decimal::new(100, 0),
            captured_amount: Decimal::ZERO,
            metadata: Value::Null,
        }
    }

    #[test]
    fn rejects_provider_identity_and_amount_corruption() {
        let mut result = valid_result();
        result.provider_id = "other".to_string();
        assert!(PaymentProviderRegistry::validate_operation_result(
            "gateway",
            "authorize",
            Decimal::new(100, 0),
            &result,
        )
        .is_err());

        let mut result = valid_result();
        result.authorized_amount = Decimal::new(101, 0);
        assert!(PaymentProviderRegistry::validate_operation_result(
            "gateway",
            "authorize",
            Decimal::new(100, 0),
            &result,
        )
        .is_err());
    }

    #[test]
    fn accepts_partial_authorization_within_request() {
        let mut result = valid_result();
        result.authorized_amount = Decimal::new(75, 0);
        assert!(PaymentProviderRegistry::validate_operation_result(
            "gateway",
            "authorize",
            Decimal::new(100, 0),
            &result,
        )
        .is_ok());
    }
}
