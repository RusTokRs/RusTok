use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::{PaymentError, PaymentResult};

pub const MANUAL_PAYMENT_PROVIDER_ID: &str = "manual";
const MAX_WEBHOOK_IDENTITY_LENGTH: usize = 191;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaymentProviderHealth {
    Ready,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentProviderDegradedMode {
    pub reason: String,
    pub fallback_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentProviderRuntimeMode {
    pub provider_id: String,
    pub operation: String,
    pub can_execute: bool,
    pub degraded_mode: Option<PaymentProviderDegradedMode>,
}

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
                "payment provider `{expected_provider_id}` is already registered"
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

    pub fn runtime_mode(
        &self,
        provider_id: &str,
        operation: &str,
    ) -> PaymentResult<PaymentProviderRuntimeMode> {
        let registration = self
            .registrations
            .get(provider_id)
            .ok_or_else(|| PaymentError::provider_configuration(provider_id))?;
        let supported = match operation {
            "authorize" => registration.descriptor.capabilities.authorize,
            "capture" => registration.descriptor.capabilities.capture,
            "refund" => registration.descriptor.capabilities.refund,
            "cancel" => registration.descriptor.capabilities.cancel,
            "webhook_ingress" => registration.descriptor.capabilities.webhook_ingress,
            other => {
                return Err(PaymentError::Validation(format!(
                    "unknown payment provider operation `{other}`"
                )));
            }
        };
        if !supported {
            return Err(PaymentError::provider_configuration(provider_id));
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
            return Err(PaymentError::provider_unavailable(provider_id, operation));
        }
        self.provider(provider_id)
            .ok_or_else(|| PaymentError::provider_configuration(provider_id))
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
        let invalid = result.provider_id != provider_id
            || result.authorized_amount < Decimal::ZERO
            || result.captured_amount < Decimal::ZERO
            || result.authorized_amount > requested_amount
            || result.captured_amount > requested_amount
            || (result.authorized_amount > Decimal::ZERO
                && result.captured_amount > result.authorized_amount)
            || (operation == "authorize" && result.authorized_amount <= Decimal::ZERO)
            || (operation == "capture" && result.captured_amount <= Decimal::ZERO);
        if invalid {
            return Err(PaymentError::provider_invalid_response(
                provider_id,
                operation,
            ));
        }
        Ok(())
    }

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
        validate_optional_webhook_hint(request.delivery_id.as_deref(), "delivery_id")?;
        validate_optional_webhook_hint(request.idempotency_key.as_deref(), "idempotency_key")?;
        if request
            .signature
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            return Err(PaymentError::Validation(
                "payment provider webhook signature must not be empty".to_string(),
            ));
        }
        if request.raw_payload.is_empty() {
            return Err(PaymentError::Validation(
                "payment provider webhook payload must not be empty".to_string(),
            ));
        }

        let delivery_hint = request.delivery_id.clone();
        let replay_hint = request.idempotency_key.clone();
        let result = self
            .executable_provider(provider_id, "webhook_ingress")?
            .handle_webhook(request)
            .await?;
        validate_verified_webhook_result(provider_id, &result)?;
        if delivery_hint
            .as_deref()
            .is_some_and(|hint| hint.trim() != result.delivery_id)
        {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` returned a delivery identity that conflicts with the transport hint"
            )));
        }
        if replay_hint
            .as_deref()
            .is_some_and(|hint| hint.trim() != result.replay_key)
        {
            return Err(PaymentError::Validation(format!(
                "payment provider `{provider_id}` returned a replay identity that conflicts with the transport hint"
            )));
        }
        Ok(result)
    }
}

fn validate_optional_webhook_hint(value: Option<&str>, label: &str) -> PaymentResult<()> {
    if let Some(value) = value {
        let value = value.trim();
        if value.is_empty() || value.len() > MAX_WEBHOOK_IDENTITY_LENGTH {
            return Err(PaymentError::Validation(format!(
                "payment provider webhook {label} hint must contain 1 to {MAX_WEBHOOK_IDENTITY_LENGTH} bytes"
            )));
        }
    }
    Ok(())
}

fn validate_verified_webhook_result(
    provider_id: &str,
    result: &PaymentProviderWebhookResult,
) -> PaymentResult<()> {
    let invalid_identity = result.provider_id != provider_id
        || result.delivery_id.trim().is_empty()
        || result.delivery_id.len() > MAX_WEBHOOK_IDENTITY_LENGTH
        || result.replay_key.trim().is_empty()
        || result.replay_key.len() > MAX_WEBHOOK_IDENTITY_LENGTH
        || result.event_type.trim().is_empty()
        || result.event_type.len() > MAX_WEBHOOK_IDENTITY_LENGTH
        || !result.metadata.is_object();
    if invalid_identity {
        return Err(PaymentError::provider_invalid_response(
            provider_id,
            "webhook_ingress",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentProviderOperationRequest {
    pub tenant_id: Uuid,
    pub collection_id: Uuid,
    pub amount: Decimal,
    pub currency_code: String,
    pub idempotency_key: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentProviderOperationResult {
    pub provider_id: String,
    pub external_reference: Option<String>,
    pub authorized_amount: Decimal,
    pub captured_amount: Decimal,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentProviderWebhookRequest {
    pub tenant_id: Uuid,
    pub provider_id: String,
    pub delivery_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub signature: Option<String>,
    pub raw_payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentProviderWebhookResult {
    pub provider_id: String,
    pub delivery_id: String,
    pub external_reference: Option<String>,
    pub event_type: String,
    pub replay_key: String,
    pub metadata: Value,
}

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
        Err(PaymentError::provider_rejected(
            MANUAL_PAYMENT_PROVIDER_ID,
            "webhook_ingress",
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
        assert!(matches!(
            PaymentProviderRegistry::validate_operation_result(
                "gateway",
                "authorize",
                Decimal::new(100, 0),
                &result,
            ),
            Err(PaymentError::ProviderInvalidResponse { .. })
        ));

        let mut result = valid_result();
        result.authorized_amount = Decimal::new(101, 0);
        assert!(matches!(
            PaymentProviderRegistry::validate_operation_result(
                "gateway",
                "authorize",
                Decimal::new(100, 0),
                &result,
            ),
            Err(PaymentError::ProviderInvalidResponse { .. })
        ));
    }

    #[test]
    fn accepts_partial_authorization_within_request() {
        let mut result = valid_result();
        result.authorized_amount = Decimal::new(75, 0);
        assert!(
            PaymentProviderRegistry::validate_operation_result(
                "gateway",
                "authorize",
                Decimal::new(100, 0),
                &result,
            )
            .is_ok()
        );
    }

    #[test]
    fn verifies_authoritative_webhook_identity() {
        let result = PaymentProviderWebhookResult {
            provider_id: "gateway".to_string(),
            delivery_id: "evt_1".to_string(),
            external_reference: None,
            event_type: "payment.captured".to_string(),
            replay_key: "evt_1".to_string(),
            metadata: serde_json::json!({}),
        };
        assert!(validate_verified_webhook_result("gateway", &result).is_ok());
    }
}
