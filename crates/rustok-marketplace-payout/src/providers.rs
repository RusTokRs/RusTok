use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{MarketplacePayoutError, MarketplacePayoutResult};

pub const MANUAL_PAYOUT_PROVIDER_ID: &str = "manual";
const MAX_PROVIDER_IDENTITY_LENGTH: usize = 191;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PayoutProviderTransferStatus {
    Submitted,
    Processing,
    Paid,
    Failed,
    Cancelled,
    Unknown,
}

impl PayoutProviderTransferStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Processing => "processing",
            Self::Paid => "paid",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PayoutProviderCapabilities {
    pub submit: bool,
    pub lookup: bool,
    pub cancel: bool,
    pub webhook_ingress: bool,
}

impl PayoutProviderCapabilities {
    pub const fn manual() -> Self {
        Self {
            submit: true,
            lookup: false,
            cancel: true,
            webhook_ingress: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PayoutProviderDescriptor {
    pub provider_id: String,
    pub display_name: String,
    pub capabilities: PayoutProviderCapabilities,
    pub default_for_new_payouts: bool,
}

impl PayoutProviderDescriptor {
    pub fn manual() -> Self {
        Self {
            provider_id: MANUAL_PAYOUT_PROVIDER_ID.to_string(),
            display_name: "Manual payout".to_string(),
            capabilities: PayoutProviderCapabilities::manual(),
            default_for_new_payouts: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PayoutProviderHealth {
    Ready,
    Degraded,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PayoutProviderDegradedMode {
    pub reason: String,
    pub fallback_profile: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PayoutProviderRegistration {
    pub descriptor: PayoutProviderDescriptor,
    pub health: PayoutProviderHealth,
    pub degraded_mode: Option<PayoutProviderDegradedMode>,
}

impl PayoutProviderRegistration {
    pub fn validate(&self, expected_provider_id: &str) -> MarketplacePayoutResult<()> {
        validate_identity(expected_provider_id, "provider registration id")?;
        if self.descriptor.provider_id != expected_provider_id {
            return Err(MarketplacePayoutError::Validation(format!(
                "payout provider descriptor id `{}` does not match registration id `{expected_provider_id}`",
                self.descriptor.provider_id
            )));
        }
        if self.descriptor.display_name.trim().is_empty() {
            return Err(MarketplacePayoutError::Validation(
                "payout provider display name must not be empty".to_string(),
            ));
        }
        if self.descriptor.default_for_new_payouts
            && self.health == PayoutProviderHealth::Unavailable
        {
            return Err(MarketplacePayoutError::Validation(
                "unavailable payout provider cannot be the default".to_string(),
            ));
        }
        if self.health != PayoutProviderHealth::Ready && self.degraded_mode.is_none() {
            return Err(MarketplacePayoutError::Validation(
                "non-ready payout provider registration must declare degraded mode".to_string(),
            ));
        }
        if let Some(mode) = &self.degraded_mode {
            if mode.reason.trim().is_empty() || mode.fallback_profile.trim().is_empty() {
                return Err(MarketplacePayoutError::Validation(
                    "payout provider degraded mode requires reason and fallback_profile"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SubmitPayoutProviderRequest {
    pub tenant_id: Uuid,
    pub payout_id: Uuid,
    pub seller_id: Uuid,
    pub amount: i64,
    pub currency_code: String,
    pub destination_reference: Option<String>,
    pub idempotency_key: String,
    pub metadata: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LookupPayoutProviderRequest {
    pub tenant_id: Uuid,
    pub payout_id: Uuid,
    pub provider_reference: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CancelPayoutProviderRequest {
    pub tenant_id: Uuid,
    pub payout_id: Uuid,
    pub provider_reference: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PayoutProviderResult {
    pub provider_id: String,
    pub status: PayoutProviderTransferStatus,
    pub external_reference: Option<String>,
    pub failure_code: Option<String>,
    pub metadata: Value,
}

#[async_trait]
pub trait PayoutProvider: Send + Sync {
    fn descriptor(&self) -> PayoutProviderDescriptor;

    async fn submit(
        &self,
        request: SubmitPayoutProviderRequest,
    ) -> MarketplacePayoutResult<PayoutProviderResult>;

    async fn lookup(
        &self,
        request: LookupPayoutProviderRequest,
    ) -> MarketplacePayoutResult<PayoutProviderResult>;

    async fn cancel(
        &self,
        request: CancelPayoutProviderRequest,
    ) -> MarketplacePayoutResult<PayoutProviderResult>;
}

#[derive(Clone, Default)]
pub struct PayoutProviderRegistry {
    providers: HashMap<String, Arc<dyn PayoutProvider>>,
    registrations: HashMap<String, PayoutProviderRegistration>,
}

impl PayoutProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_manual_provider() -> Self {
        let mut registry = Self::new();
        registry.register_builtin(Arc::new(ManualPayoutProvider));
        registry
    }

    pub fn register_builtin(&mut self, provider: Arc<dyn PayoutProvider>) {
        let descriptor = provider.descriptor();
        self.providers
            .insert(descriptor.provider_id.clone(), provider);
        self.registrations.insert(
            descriptor.provider_id.clone(),
            PayoutProviderRegistration {
                descriptor,
                health: PayoutProviderHealth::Ready,
                degraded_mode: None,
            },
        );
    }

    pub fn register_external(
        &mut self,
        expected_provider_id: &str,
        provider: Arc<dyn PayoutProvider>,
        registration: PayoutProviderRegistration,
    ) -> MarketplacePayoutResult<()> {
        registration.validate(expected_provider_id)?;
        let descriptor = provider.descriptor();
        if descriptor.provider_id != registration.descriptor.provider_id {
            return Err(MarketplacePayoutError::Validation(format!(
                "payout provider adapter id `{}` does not match descriptor id `{}`",
                descriptor.provider_id, registration.descriptor.provider_id
            )));
        }
        if self.providers.contains_key(expected_provider_id) {
            return Err(MarketplacePayoutError::Validation(format!(
                "payout provider `{expected_provider_id}` is already registered"
            )));
        }
        if registration.descriptor.default_for_new_payouts
            && self
                .registrations
                .values()
                .any(|existing| existing.descriptor.default_for_new_payouts)
        {
            return Err(MarketplacePayoutError::Validation(
                "only one payout provider may be default for new payouts".to_string(),
            ));
        }
        self.providers
            .insert(expected_provider_id.to_string(), provider);
        self.registrations
            .insert(expected_provider_id.to_string(), registration);
        Ok(())
    }

    pub fn provider(&self, provider_id: &str) -> Option<Arc<dyn PayoutProvider>> {
        self.providers.get(provider_id).cloned()
    }

    pub fn descriptors(&self) -> Vec<PayoutProviderDescriptor> {
        let mut descriptors = self
            .registrations
            .values()
            .map(|registration| registration.descriptor.clone())
            .collect::<Vec<_>>();
        descriptors.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        descriptors
    }

    pub async fn execute_submit(
        &self,
        provider_id: &str,
        request: SubmitPayoutProviderRequest,
    ) -> MarketplacePayoutResult<PayoutProviderResult> {
        validate_submit_request(&request)?;
        let result = self
            .executable_provider(provider_id, "submit")?
            .submit(request)
            .await?;
        validate_result(provider_id, "submit", &result)?;
        Ok(result)
    }

    pub async fn execute_lookup(
        &self,
        provider_id: &str,
        request: LookupPayoutProviderRequest,
    ) -> MarketplacePayoutResult<PayoutProviderResult> {
        validate_lookup_request(&request)?;
        let result = self
            .executable_provider(provider_id, "lookup")?
            .lookup(request)
            .await?;
        validate_result(provider_id, "lookup", &result)?;
        Ok(result)
    }

    pub async fn execute_cancel(
        &self,
        provider_id: &str,
        request: CancelPayoutProviderRequest,
    ) -> MarketplacePayoutResult<PayoutProviderResult> {
        validate_cancel_request(&request)?;
        let result = self
            .executable_provider(provider_id, "cancel")?
            .cancel(request)
            .await?;
        validate_result(provider_id, "cancel", &result)?;
        Ok(result)
    }

    fn executable_provider(
        &self,
        provider_id: &str,
        operation: &str,
    ) -> MarketplacePayoutResult<Arc<dyn PayoutProvider>> {
        let registration = self.registrations.get(provider_id).ok_or_else(|| {
            MarketplacePayoutError::ProviderConfiguration {
                provider_id: provider_id.to_string(),
            }
        })?;
        let supported = match operation {
            "submit" => registration.descriptor.capabilities.submit,
            "lookup" => registration.descriptor.capabilities.lookup,
            "cancel" => registration.descriptor.capabilities.cancel,
            other => {
                return Err(MarketplacePayoutError::Validation(format!(
                    "unknown payout provider operation `{other}`"
                )))
            }
        };
        if !supported {
            return Err(MarketplacePayoutError::ProviderConfiguration {
                provider_id: provider_id.to_string(),
            });
        }
        if registration.health == PayoutProviderHealth::Unavailable {
            return Err(MarketplacePayoutError::ProviderUnavailable {
                provider_id: provider_id.to_string(),
                operation: operation.to_string(),
            });
        }
        self.provider(provider_id)
            .ok_or_else(|| MarketplacePayoutError::ProviderConfiguration {
                provider_id: provider_id.to_string(),
            })
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ManualPayoutProvider;

#[async_trait]
impl PayoutProvider for ManualPayoutProvider {
    fn descriptor(&self) -> PayoutProviderDescriptor {
        PayoutProviderDescriptor::manual()
    }

    async fn submit(
        &self,
        request: SubmitPayoutProviderRequest,
    ) -> MarketplacePayoutResult<PayoutProviderResult> {
        Ok(PayoutProviderResult {
            provider_id: MANUAL_PAYOUT_PROVIDER_ID.to_string(),
            status: PayoutProviderTransferStatus::Submitted,
            external_reference: None,
            failure_code: None,
            metadata: request.metadata,
        })
    }

    async fn lookup(
        &self,
        _request: LookupPayoutProviderRequest,
    ) -> MarketplacePayoutResult<PayoutProviderResult> {
        Err(MarketplacePayoutError::ProviderConfiguration {
            provider_id: MANUAL_PAYOUT_PROVIDER_ID.to_string(),
        })
    }

    async fn cancel(
        &self,
        _request: CancelPayoutProviderRequest,
    ) -> MarketplacePayoutResult<PayoutProviderResult> {
        Ok(PayoutProviderResult {
            provider_id: MANUAL_PAYOUT_PROVIDER_ID.to_string(),
            status: PayoutProviderTransferStatus::Cancelled,
            external_reference: None,
            failure_code: None,
            metadata: serde_json::json!({}),
        })
    }
}

fn validate_submit_request(request: &SubmitPayoutProviderRequest) -> MarketplacePayoutResult<()> {
    validate_common_request(
        request.tenant_id,
        request.payout_id,
        request.idempotency_key.as_str(),
    )?;
    if request.seller_id.is_nil() {
        return Err(MarketplacePayoutError::Validation(
            "payout provider submit request has nil seller_id".to_string(),
        ));
    }
    if request.amount <= 0 {
        return Err(MarketplacePayoutError::Validation(
            "payout provider submit amount must be greater than zero".to_string(),
        ));
    }
    validate_currency(request.currency_code.as_str())?;
    validate_optional_identity(
        request.destination_reference.as_deref(),
        "destination_reference",
    )?;
    if !request.metadata.is_object() {
        return Err(MarketplacePayoutError::Validation(
            "payout provider submit metadata must be a JSON object".to_string(),
        ));
    }
    Ok(())
}

fn validate_lookup_request(request: &LookupPayoutProviderRequest) -> MarketplacePayoutResult<()> {
    validate_common_request(
        request.tenant_id,
        request.payout_id,
        request.idempotency_key.as_str(),
    )?;
    validate_identity(request.provider_reference.as_str(), "provider_reference")
}

fn validate_cancel_request(request: &CancelPayoutProviderRequest) -> MarketplacePayoutResult<()> {
    validate_common_request(
        request.tenant_id,
        request.payout_id,
        request.idempotency_key.as_str(),
    )?;
    validate_identity(request.provider_reference.as_str(), "provider_reference")
}

fn validate_common_request(
    tenant_id: Uuid,
    payout_id: Uuid,
    idempotency_key: &str,
) -> MarketplacePayoutResult<()> {
    if tenant_id.is_nil() {
        return Err(MarketplacePayoutError::Validation(
            "payout provider request has nil tenant_id".to_string(),
        ));
    }
    if payout_id.is_nil() {
        return Err(MarketplacePayoutError::Validation(
            "payout provider request has nil payout_id".to_string(),
        ));
    }
    validate_identity(idempotency_key, "idempotency_key")
}

fn validate_result(
    provider_id: &str,
    operation: &str,
    result: &PayoutProviderResult,
) -> MarketplacePayoutResult<()> {
    if result.provider_id != provider_id || !result.metadata.is_object() {
        return Err(MarketplacePayoutError::ProviderInvalidResponse {
            provider_id: provider_id.to_string(),
            operation: operation.to_string(),
        });
    }
    validate_optional_identity(result.external_reference.as_deref(), "external_reference")?;
    validate_optional_identity(result.failure_code.as_deref(), "failure_code")?;
    if result.status == PayoutProviderTransferStatus::Failed && result.failure_code.is_none() {
        return Err(MarketplacePayoutError::ProviderInvalidResponse {
            provider_id: provider_id.to_string(),
            operation: operation.to_string(),
        });
    }
    if result.status != PayoutProviderTransferStatus::Failed && result.failure_code.is_some() {
        return Err(MarketplacePayoutError::ProviderInvalidResponse {
            provider_id: provider_id.to_string(),
            operation: operation.to_string(),
        });
    }
    if result.status == PayoutProviderTransferStatus::Paid
        && provider_id != MANUAL_PAYOUT_PROVIDER_ID
        && result.external_reference.is_none()
    {
        return Err(MarketplacePayoutError::ProviderInvalidResponse {
            provider_id: provider_id.to_string(),
            operation: operation.to_string(),
        });
    }
    Ok(())
}

fn validate_currency(value: &str) -> MarketplacePayoutResult<()> {
    let value = value.trim();
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(MarketplacePayoutError::Validation(
            "payout provider currency_code must contain exactly three ASCII letters".to_string(),
        ));
    }
    Ok(())
}

fn validate_optional_identity(value: Option<&str>, label: &str) -> MarketplacePayoutResult<()> {
    if let Some(value) = value {
        validate_identity(value, label)?;
    }
    Ok(())
}

fn validate_identity(value: &str, label: &str) -> MarketplacePayoutResult<()> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_PROVIDER_IDENTITY_LENGTH {
        return Err(MarketplacePayoutError::Validation(format!(
            "payout provider {label} must contain 1 to {MAX_PROVIDER_IDENTITY_LENGTH} bytes"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn manual_provider_never_reports_paid_implicitly() {
        let registry = PayoutProviderRegistry::with_manual_provider();
        let result = registry
            .execute_submit(
                MANUAL_PAYOUT_PROVIDER_ID,
                SubmitPayoutProviderRequest {
                    tenant_id: Uuid::new_v4(),
                    payout_id: Uuid::new_v4(),
                    seller_id: Uuid::new_v4(),
                    amount: 1_000,
                    currency_code: "USD".to_string(),
                    destination_reference: None,
                    idempotency_key: "manual-submit".to_string(),
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .expect("manual submit should be admitted");

        assert_eq!(result.status, PayoutProviderTransferStatus::Submitted);
        assert_ne!(result.status, PayoutProviderTransferStatus::Paid);
    }

    #[test]
    fn paid_external_result_requires_provider_reference() {
        let result = PayoutProviderResult {
            provider_id: "gateway".to_string(),
            status: PayoutProviderTransferStatus::Paid,
            external_reference: None,
            failure_code: None,
            metadata: serde_json::json!({}),
        };

        assert!(validate_result("gateway", "lookup", &result).is_err());
    }
}
