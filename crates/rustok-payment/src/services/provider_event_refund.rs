use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::{Map, Value};
use std::str::FromStr;
use uuid::Uuid;

use crate::dto::CompleteRefundInput;
use crate::error::PaymentError;
use crate::providers::PaymentProviderWebhookResult;

use super::{
    PaymentProviderEventApplier, PaymentProviderEventApplyError, PaymentProviderEventContext,
    PaymentService,
};

const EVENT_REFUND_COMPLETED: &str = "refund.completed";

pub struct RefundLifecycleEventApplier {
    payment_service: PaymentService,
}

impl RefundLifecycleEventApplier {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            payment_service: PaymentService::new(db),
        }
    }
}

#[async_trait]
impl PaymentProviderEventApplier for RefundLifecycleEventApplier {
    async fn apply(
        &self,
        context: PaymentProviderEventContext,
        event: PaymentProviderWebhookResult,
    ) -> Result<(), PaymentProviderEventApplyError> {
        if event.event_type != EVENT_REFUND_COMPLETED {
            return Err(non_retryable(
                "payment.webhook_event_unsupported",
                format!(
                    "refund applier does not support normalized event `{}`",
                    event.event_type
                ),
            ));
        }
        let payload = NormalizedRefundEvent::parse(&event)?;
        let refund = self
            .payment_service
            .get_refund(context.tenant_id, payload.refund_id)
            .await
            .map_err(map_payment_error)?;
        if refund.tenant_id != context.tenant_id {
            return Err(non_retryable(
                "payment.webhook_tenant_mismatch",
                "refund does not belong to the webhook tenant",
            ));
        }
        if !refund
            .currency_code
            .eq_ignore_ascii_case(payload.currency_code.as_str())
        {
            return Err(non_retryable(
                "payment.webhook_currency_mismatch",
                format!(
                    "refund {} currency does not match provider event",
                    refund.id
                ),
            ));
        }
        if refund.amount != payload.amount {
            return Err(non_retryable(
                "payment.webhook_refund_amount_mismatch",
                format!("refund {} amount does not match provider event", refund.id),
            ));
        }

        let collection = self
            .payment_service
            .get_collection(context.tenant_id, refund.payment_collection_id)
            .await
            .map_err(map_payment_error)?;
        if let Some(provider_id) = collection.provider_id.as_deref() {
            if provider_id != context.provider_id {
                return Err(non_retryable(
                    "payment.webhook_provider_mismatch",
                    format!(
                        "refund {} belongs to a payment owned by another provider",
                        refund.id
                    ),
                ));
            }
        }

        match refund.status.as_str() {
            "refunded" => return Ok(()),
            "pending" => {}
            "cancelled" => {
                return Err(non_retryable(
                    "payment.webhook_refund_cancelled",
                    format!("cancelled refund {} cannot be completed", refund.id),
                ));
            }
            status => {
                return Err(non_retryable(
                    "payment.webhook_refund_conflict",
                    format!(
                        "refund {} cannot apply completed event from `{status}`",
                        refund.id
                    ),
                ));
            }
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
                "completed refund webhook requires an external reference",
            ));
        }

        self.payment_service
            .complete_refund(
                context.tenant_id,
                refund.id,
                CompleteRefundInput {
                    metadata: owner_metadata(&context, &event, &payload.metadata),
                },
            )
            .await
            .map_err(map_payment_error)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct NormalizedRefundEvent {
    refund_id: Uuid,
    amount: Decimal,
    currency_code: String,
    metadata: Value,
}

impl NormalizedRefundEvent {
    fn parse(event: &PaymentProviderWebhookResult) -> Result<Self, PaymentProviderEventApplyError> {
        let metadata = event.metadata.as_object().ok_or_else(|| {
            non_retryable(
                "payment.webhook_metadata_invalid",
                "normalized refund webhook metadata must be an object",
            )
        })?;
        let refund_id =
            Uuid::parse_str(required_string(metadata, "refund_id")?.as_str()).map_err(|_| {
                non_retryable(
                    "payment.webhook_refund_id_invalid",
                    "normalized refund webhook refund_id must be a UUID",
                )
            })?;
        let amount =
            Decimal::from_str(required_string(metadata, "amount")?.as_str()).map_err(|_| {
                non_retryable(
                    "payment.webhook_amount_invalid",
                    "normalized refund webhook amount must be a decimal string",
                )
            })?;
        if amount <= Decimal::ZERO {
            return Err(non_retryable(
                "payment.webhook_amount_invalid",
                "normalized refund webhook amount must be positive",
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
                "normalized refund webhook currency_code must be a three-letter code",
            ));
        }
        let domain_metadata = metadata
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        if !domain_metadata.is_object() {
            return Err(non_retryable(
                "payment.webhook_domain_metadata_invalid",
                "normalized refund webhook metadata.metadata must be an object",
            ));
        }
        Ok(Self {
            refund_id,
            amount,
            currency_code,
            metadata: domain_metadata,
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
                format!("normalized refund webhook metadata requires `{key}`"),
            )
        })
}

fn owner_metadata(
    context: &PaymentProviderEventContext,
    event: &PaymentProviderWebhookResult,
    domain_metadata: &Value,
) -> Value {
    let mut metadata = domain_metadata.as_object().cloned().unwrap_or_default();
    metadata.insert(
        "provider_webhook".to_string(),
        serde_json::json!({
            "event_id": context.event_id,
            "provider_id": context.provider_id.clone(),
            "delivery_id": context.delivery_id.clone(),
            "idempotency_key": context.idempotency_key.clone(),
            "event_type": event.event_type.clone(),
            "external_reference": event.external_reference.clone(),
        }),
    );
    Value::Object(metadata)
}

fn map_payment_error(error: PaymentError) -> PaymentProviderEventApplyError {
    match error {
        PaymentError::Database(_) | PaymentError::ProviderUnavailable { .. } => retryable(
            "payment.webhook_storage_or_provider_unavailable",
            "payment owner or provider is temporarily unavailable",
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
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => retryable(
            "payment.webhook_owner_not_found",
            "payment owner record was not found",
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
