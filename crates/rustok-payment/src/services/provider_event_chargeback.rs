use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::{Map, Value};
use std::str::FromStr;
use uuid::Uuid;

use crate::error::PaymentError;
use crate::providers::PaymentProviderWebhookResult;

use super::{
    PaymentProviderEventApplier, PaymentProviderEventApplyError, PaymentProviderEventContext,
    PaymentService,
};

const EVENT_CHARGEBACK_COMPLETED: &str = "chargeback.completed";

pub struct ChargebackLifecycleEventApplier {
    payment_service: PaymentService,
}

impl ChargebackLifecycleEventApplier {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            payment_service: PaymentService::new(db),
        }
    }
}

#[async_trait]
impl PaymentProviderEventApplier for ChargebackLifecycleEventApplier {
    async fn apply(
        &self,
        context: PaymentProviderEventContext,
        event: PaymentProviderWebhookResult,
    ) -> Result<(), PaymentProviderEventApplyError> {
        if event.event_type != EVENT_CHARGEBACK_COMPLETED {
            return Err(non_retryable(
                "payment.webhook_event_unsupported",
                format!(
                    "chargeback applier does not support normalized event `{}`",
                    event.event_type
                ),
            ));
        }
        let payload = NormalizedChargebackEvent::parse(&event)?;
        let collection = self
            .payment_service
            .get_collection(context.tenant_id, payload.collection_id)
            .await
            .map_err(map_payment_error)?;
        if let Some(provider_id) = collection.provider_id.as_deref() {
            if provider_id != context.provider_id {
                return Err(non_retryable(
                    "payment.webhook_provider_mismatch",
                    format!(
                        "payment collection {} belongs to another provider",
                        collection.id
                    ),
                ));
            }
        }
        if collection.status != "captured" {
            return Err(retryable(
                "payment.webhook_chargeback_payment_not_captured",
                format!(
                    "payment collection {} is `{}` rather than captured",
                    collection.id, collection.status
                ),
            ));
        }
        if !collection
            .currency_code
            .eq_ignore_ascii_case(payload.currency_code.as_str())
        {
            return Err(non_retryable(
                "payment.webhook_currency_mismatch",
                format!(
                    "chargeback currency does not match payment collection {}",
                    collection.id
                ),
            ));
        }
        if payload.amount > collection.captured_amount {
            return Err(non_retryable(
                "payment.webhook_chargeback_amount_exceeds_capture",
                format!(
                    "chargeback amount exceeds captured amount for payment collection {}",
                    collection.id
                ),
            ));
        }
        if event
            .external_reference
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            return Err(non_retryable(
                "payment.webhook_reference_required",
                "completed chargeback webhook requires an external reference",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct NormalizedChargebackEvent {
    collection_id: Uuid,
    amount: Decimal,
    currency_code: String,
}

impl NormalizedChargebackEvent {
    fn parse(event: &PaymentProviderWebhookResult) -> Result<Self, PaymentProviderEventApplyError> {
        let metadata = event.metadata.as_object().ok_or_else(|| {
            non_retryable(
                "payment.webhook_metadata_invalid",
                "normalized chargeback webhook metadata must be an object",
            )
        })?;
        let collection_id = Uuid::parse_str(required_string(metadata, "collection_id")?.as_str())
            .map_err(|_| {
                non_retryable(
                    "payment.webhook_collection_id_invalid",
                    "normalized chargeback webhook collection_id must be a UUID",
                )
            })?;
        let amount = Decimal::from_str(required_string(metadata, "amount")?.as_str()).map_err(
            |_| {
                non_retryable(
                    "payment.webhook_amount_invalid",
                    "normalized chargeback webhook amount must be a decimal string",
                )
            },
        )?;
        if amount <= Decimal::ZERO {
            return Err(non_retryable(
                "payment.webhook_amount_invalid",
                "normalized chargeback webhook amount must be positive",
            ));
        }
        let currency_code = required_string(metadata, "currency_code")?.to_ascii_uppercase();
        if currency_code.len() != 3
            || !currency_code
                .chars()
                .all(|character| character.is_ascii_alphabetic())
        {
            return Err(non_retryable(
                "payment.webhook_currency_invalid",
                "normalized chargeback webhook currency_code must be a three-letter code",
            ));
        }
        Ok(Self {
            collection_id,
            amount,
            currency_code,
        })
    }
}

fn required_string(
    metadata: &Map<String, Value>,
    key: &str,
) -> Result<String, PaymentProviderEventApplyError> {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            non_retryable(
                "payment.webhook_metadata_missing",
                format!("normalized chargeback webhook metadata requires `{key}`"),
            )
        })
}

fn map_payment_error(error: PaymentError) -> PaymentProviderEventApplyError {
    match error {
        PaymentError::Database(_) | PaymentError::ProviderUnavailable { .. } => retryable(
            "payment.webhook_storage_or_provider_unavailable",
            "payment owner or provider is temporarily unavailable",
        ),
        PaymentError::PaymentCollectionNotFound(_) => retryable(
            "payment.webhook_owner_not_found",
            "payment collection was not found",
        ),
        PaymentError::InvalidTransition { from, to } => retryable(
            "payment.webhook_transition_pending",
            format!("payment transition from `{from}` to `{to}` is not ready"),
        ),
        PaymentError::Validation(message) => {
            non_retryable("payment.webhook_validation_failed", message)
        }
        PaymentError::ProviderRejected { .. } => non_retryable(
            "payment.webhook_provider_rejected",
            "payment provider rejected the normalized event",
        ),
        PaymentError::ProviderInvalidResponse { .. } => non_retryable(
            "payment.webhook_provider_invalid_response",
            "payment provider returned invalid normalized facts",
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => non_retryable(
            "payment.webhook_provider_outcome_unknown",
            "payment provider outcome requires operator reconciliation",
        ),
        PaymentError::ProviderConfiguration { .. } => non_retryable(
            "payment.webhook_provider_not_configured",
            "payment provider is not configured for this tenant",
        ),
        PaymentError::PaymentNotFound(_) | PaymentError::RefundNotFound(_) => non_retryable(
            "payment.webhook_owner_identity_invalid",
            "chargeback normalized facts reference an unsupported payment owner identity",
        ),
    }
}

fn retryable(
    code: impl Into<String>,
    message: impl Into<String>,
) -> PaymentProviderEventApplyError {
    PaymentProviderEventApplyError::new(code, message, true)
}

fn non_retryable(
    code: impl Into<String>,
    message: impl Into<String>,
) -> PaymentProviderEventApplyError {
    PaymentProviderEventApplyError::new(code, message, false)
}
