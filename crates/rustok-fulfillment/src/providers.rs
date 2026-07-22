use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
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
            create_label: true,
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
        validate_provider_id(expected_provider_id)?;
        if self.descriptor.provider_id != expected_provider_id {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider descriptor id `{}` does not match registration id `{}`",
                self.descriptor.provider_id, expected_provider_id
            )));
        }
        if self.descriptor.display_name.trim().is_empty() {
            return Err(FulfillmentError::Validation(
                "fulfillment provider display name must not be empty".to_string(),
            ));
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
        if let Some(mode) = &self.degraded_mode {
            if mode.reason.trim().is_empty() || mode.fallback_profile.trim().is_empty() {
                return Err(FulfillmentError::Validation(
                    "fulfillment provider degraded mode requires reason and fallback_profile"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// In-memory provider registry assembled by host composition before fulfillment runtime.
///
/// The registry validates all adapter inputs and normalized outputs. Provider facts are
/// never trusted solely because the adapter is registered.
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
        if descriptor != registration.descriptor {
            return Err(FulfillmentError::Validation(
                "fulfillment provider adapter descriptor does not match registration descriptor"
                    .to_string(),
            ));
        }
        if self.providers.contains_key(expected_provider_id) {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider `{}` is already registered",
                expected_provider_id
            )));
        }
        if registration.descriptor.default_for_manual_options
            && self
                .registrations
                .values()
                .any(|existing| existing.descriptor.default_for_manual_options)
        {
            return Err(FulfillmentError::Validation(
                "only one fulfillment provider may be default for manual options".to_string(),
            ));
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
                )));
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

    fn executable_provider(
        &self,
        provider_id: &str,
        operation: &str,
    ) -> FulfillmentResult<Arc<dyn FulfillmentProvider>> {
        let mode = self.runtime_mode(provider_id, operation)?;
        if !mode.can_execute {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider `{}` is unavailable for `{}`",
                provider_id, operation
            )));
        }
        self.provider(provider_id).ok_or_else(|| {
            FulfillmentError::Validation(format!(
                "fulfillment provider `{}` is not registered",
                provider_id
            ))
        })
    }

    pub async fn execute_quote_rates(
        &self,
        provider_id: &str,
        request: FulfillmentRateQuoteRequest,
    ) -> FulfillmentResult<Vec<FulfillmentRateQuote>> {
        validate_rate_quote_request(provider_id, &request)?;
        let expected_currency = request.currency_code.trim().to_ascii_uppercase();
        let quotes = self
            .executable_provider(provider_id, "rate_quote")?
            .quote_rates(request)
            .await?;
        validate_rate_quotes(provider_id, expected_currency.as_str(), &quotes)?;
        Ok(quotes)
    }

    pub async fn execute_create_label(
        &self,
        provider_id: &str,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        validate_operation_request(provider_id, "create_label", &request)?;
        let result = self
            .executable_provider(provider_id, "create_label")?
            .create_label(request)
            .await?;
        validate_operation_result(provider_id, "create_label", &result)?;
        Ok(result)
    }

    pub async fn execute_ship(
        &self,
        provider_id: &str,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        validate_operation_request(provider_id, "ship", &request)?;
        let result = self
            .executable_provider(provider_id, "ship")?
            .ship(request)
            .await?;
        validate_operation_result(provider_id, "ship", &result)?;
        Ok(result)
    }

    pub async fn execute_cancel(
        &self,
        provider_id: &str,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        validate_operation_request(provider_id, "cancel", &request)?;
        let result = self
            .executable_provider(provider_id, "cancel")?
            .cancel(request)
            .await?;
        validate_operation_result(provider_id, "cancel", &result)?;
        Ok(result)
    }

    pub async fn execute_tracking_webhook(
        &self,
        provider_id: &str,
        request: FulfillmentProviderWebhookRequest,
    ) -> FulfillmentResult<FulfillmentProviderWebhookResult> {
        validate_webhook_request(provider_id, &request)?;
        let expected_replay_key = request.idempotency_key.clone();
        let result = self
            .executable_provider(provider_id, "tracking_webhook_ingress")?
            .handle_tracking_webhook(request)
            .await?;
        validate_webhook_result(provider_id, expected_replay_key.as_str(), &result)?;
        Ok(result)
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

    async fn ship(
        &self,
        _request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        Err(FulfillmentError::Validation(
            "fulfillment provider does not implement ship operation".to_string(),
        ))
    }

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

    async fn ship(
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

fn validate_provider_id(value: &str) -> FulfillmentResult<()> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 100
        || !value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(FulfillmentError::Validation(
            "fulfillment provider_id must use lowercase ASCII, digits, underscore, or hyphen"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_currency_code(value: &str) -> FulfillmentResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(FulfillmentError::Validation(
            "currency_code must be a 3-letter code".to_string(),
        ));
    }
    Ok(normalized)
}

fn validate_optional_boundary_text(
    field: &str,
    value: Option<&str>,
    max_len: usize,
) -> FulfillmentResult<()> {
    if let Some(value) = value {
        let value = value.trim();
        if value.is_empty() || value.len() > max_len {
            return Err(FulfillmentError::Validation(format!(
                "{field} must be non-empty and at most {max_len} characters"
            )));
        }
    }
    Ok(())
}

fn validate_rate_quote_request(
    provider_id: &str,
    request: &FulfillmentRateQuoteRequest,
) -> FulfillmentResult<()> {
    validate_provider_id(provider_id)?;
    if request.tenant_id.is_nil() {
        return Err(FulfillmentError::Validation(
            "fulfillment rate quote request has nil tenant_id".to_string(),
        ));
    }
    if request.cart_id.is_some_and(|id| id.is_nil()) {
        return Err(FulfillmentError::Validation(
            "fulfillment rate quote request has nil cart_id".to_string(),
        ));
    }
    validate_currency_code(&request.currency_code)?;
    validate_optional_boundary_text("seller_id", request.seller_id.as_deref(), 100)?;
    validate_optional_boundary_text(
        "shipping_profile_slug",
        request.shipping_profile_slug.as_deref(),
        100,
    )?;
    Ok(())
}

fn validate_rate_quotes(
    provider_id: &str,
    expected_currency: &str,
    quotes: &[FulfillmentRateQuote],
) -> FulfillmentResult<()> {
    let mut service_codes = HashSet::new();
    for quote in quotes {
        if quote.provider_id != provider_id {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider `{provider_id}` returned quote for `{}`",
                quote.provider_id
            )));
        }
        let service_code = quote.service_code.trim();
        if service_code.is_empty() || service_code.len() > 100 {
            return Err(FulfillmentError::Validation(
                "fulfillment rate quote service_code must be non-empty and at most 100 characters"
                    .to_string(),
            ));
        }
        if !service_codes.insert(service_code.to_string()) {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider `{provider_id}` returned duplicate service_code `{service_code}`"
            )));
        }
        if quote.amount < Decimal::ZERO {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider `{provider_id}` returned a negative rate"
            )));
        }
        let quote_currency = validate_currency_code(&quote.currency_code)?;
        if quote_currency != expected_currency {
            return Err(FulfillmentError::Validation(format!(
                "fulfillment provider `{provider_id}` returned currency {quote_currency}, expected {expected_currency}"
            )));
        }
    }
    Ok(())
}

fn validate_operation_request(
    provider_id: &str,
    operation: &str,
    request: &FulfillmentProviderOperationRequest,
) -> FulfillmentResult<()> {
    validate_provider_id(provider_id)?;
    if request.tenant_id.is_nil() {
        return Err(FulfillmentError::Validation(format!(
            "fulfillment provider `{provider_id}` {operation} request has nil tenant_id"
        )));
    }
    if request.fulfillment_id.is_nil() {
        return Err(FulfillmentError::Validation(format!(
            "fulfillment provider `{provider_id}` {operation} request has nil fulfillment_id"
        )));
    }
    if request
        .idempotency_key
        .as_deref()
        .is_some_and(|key| key.trim().is_empty())
    {
        return Err(FulfillmentError::Validation(format!(
            "fulfillment provider `{provider_id}` {operation} idempotency_key must not be blank"
        )));
    }
    Ok(())
}

fn validate_operation_result(
    provider_id: &str,
    operation: &str,
    result: &FulfillmentProviderOperationResult,
) -> FulfillmentResult<()> {
    if result.provider_id != provider_id {
        return Err(FulfillmentError::Validation(format!(
            "fulfillment provider `{provider_id}` returned {operation} result for `{}`",
            result.provider_id
        )));
    }
    validate_optional_boundary_text(
        "external_reference",
        result.external_reference.as_deref(),
        191,
    )?;
    validate_optional_boundary_text("tracking_number", result.tracking_number.as_deref(), 191)?;
    Ok(())
}

fn validate_webhook_request(
    provider_id: &str,
    request: &FulfillmentProviderWebhookRequest,
) -> FulfillmentResult<()> {
    validate_provider_id(provider_id)?;
    if request.tenant_id.is_nil() {
        return Err(FulfillmentError::Validation(
            "fulfillment provider webhook request has nil tenant_id".to_string(),
        ));
    }
    if request.provider_id != provider_id {
        return Err(FulfillmentError::Validation(format!(
            "fulfillment webhook provider `{}` does not match registry provider `{provider_id}`",
            request.provider_id
        )));
    }
    if request.delivery_id.trim().is_empty() || request.idempotency_key.trim().is_empty() {
        return Err(FulfillmentError::Validation(
            "fulfillment provider webhook requires delivery_id and idempotency_key".to_string(),
        ));
    }
    if request.raw_payload.is_empty() {
        return Err(FulfillmentError::Validation(
            "fulfillment provider webhook payload must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_webhook_result(
    provider_id: &str,
    expected_replay_key: &str,
    result: &FulfillmentProviderWebhookResult,
) -> FulfillmentResult<()> {
    if result.provider_id != provider_id {
        return Err(FulfillmentError::Validation(format!(
            "fulfillment provider `{provider_id}` returned webhook result for `{}`",
            result.provider_id
        )));
    }
    if result.event_type.trim().is_empty() || result.replay_key != expected_replay_key {
        return Err(FulfillmentError::Validation(format!(
            "fulfillment provider `{provider_id}` returned an invalid webhook event or replay key"
        )));
    }
    validate_optional_boundary_text(
        "external_reference",
        result.external_reference.as_deref(),
        191,
    )?;
    validate_optional_boundary_text("tracking_number", result.tracking_number.as_deref(), 191)?;
    Ok(())
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn rejects_corrupt_rate_quotes() {
        let wrong_provider = FulfillmentRateQuote {
            provider_id: "other".to_string(),
            service_code: "ground".to_string(),
            amount: Decimal::ONE,
            currency_code: "USD".to_string(),
            metadata: Value::Null,
        };
        assert!(validate_rate_quotes("carrier", "USD", &[wrong_provider]).is_err());

        let negative = FulfillmentRateQuote {
            provider_id: "carrier".to_string(),
            service_code: "ground".to_string(),
            amount: -Decimal::ONE,
            currency_code: "USD".to_string(),
            metadata: Value::Null,
        };
        assert!(validate_rate_quotes("carrier", "USD", &[negative]).is_err());
    }

    #[test]
    fn rejects_duplicate_service_codes() {
        let quote = FulfillmentRateQuote {
            provider_id: "carrier".to_string(),
            service_code: "ground".to_string(),
            amount: Decimal::ONE,
            currency_code: "USD".to_string(),
            metadata: Value::Null,
        };
        assert!(validate_rate_quotes("carrier", "USD", &[quote.clone(), quote]).is_err());
    }

    #[test]
    fn accepts_valid_operation_result() {
        let result = FulfillmentProviderOperationResult {
            provider_id: "carrier".to_string(),
            external_reference: Some("label-1".to_string()),
            tracking_number: Some("track-1".to_string()),
            metadata: Value::Null,
        };
        assert!(validate_operation_result("carrier", "create_label", &result).is_ok());
    }
}
