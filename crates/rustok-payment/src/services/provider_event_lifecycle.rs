use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::{Map, Value};
use std::str::FromStr;
use uuid::Uuid;

use crate::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CapturePaymentInput,
    PaymentCollectionStatusKind,
};
use crate::error::PaymentError;
use crate::providers::PaymentProviderWebhookResult;

use super::{
    PaymentProviderEventApplier, PaymentProviderEventApplyError, PaymentProviderEventContext,
    PaymentService,
};

const EVENT_PAYMENT_AUTHORIZED: &str = "payment.authorized";
const EVENT_PAYMENT_CAPTURED: &str = "payment.captured";
const EVENT_PAYMENT_CANCELLED: &str = "payment.cancelled";

pub struct PaymentLifecycleEventApplier {
    payment_service: PaymentService,
}

impl PaymentLifecycleEventApplier {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            payment_service: PaymentService::new(db),
        }
    }
}

#[async_trait]
impl PaymentProviderEventApplier for PaymentLifecycleEventApplier {
    async fn apply(
        &self,
        context: PaymentProviderEventContext,
        event: PaymentProviderWebhookResult,
    ) -> Result<(), PaymentProviderEventApplyError> {
        let payload = NormalizedPaymentEvent::parse(&context, &event)?;
        let collection = self
            .payment_service
            .get_collection(context.tenant_id, payload.collection_id)
            .await
            .map_err(map_payment_error)?;
        validate_collection_identity(&context, &event, &payload, &collection)?;

        match event.event_type.as_str() {
            EVENT_PAYMENT_AUTHORIZED => {
                match collection.status_kind() {
                    PaymentCollectionStatusKind::Authorized
                    | PaymentCollectionStatusKind::Captured => return Ok(()),
                    PaymentCollectionStatusKind::Pending => {}
                    PaymentCollectionStatusKind::Cancelled
                    | PaymentCollectionStatusKind::Unknown => {
                        return Err(non_retryable(
                            "payment.webhook_authorize_conflict",
                            format!(
                                "payment collection {} cannot apply authorized event from its current lifecycle state",
                                collection.id
                            ),
                        ));
                    }
                }
                self.payment_service
                    .authorize_collection(
                        context.tenant_id,
                        collection.id,
                        AuthorizePaymentInput {
                            provider_id: Some(context.provider_id.clone()),
                            provider_payment_id: event.external_reference.clone(),
                            amount: Some(payload.amount),
                            metadata: owner_metadata(&context, &event, &payload.metadata),
                        },
                    )
                    .await
                    .map_err(map_payment_error)?;
            }
            EVENT_PAYMENT_CAPTURED => {
                match collection.status_kind() {
                    PaymentCollectionStatusKind::Captured => return Ok(()),
                    PaymentCollectionStatusKind::Authorized => {}
                    PaymentCollectionStatusKind::Pending => {
                        return Err(retryable(
                            "payment.webhook_capture_out_of_order",
                            format!(
                                "payment collection {} is still pending authorization",
                                collection.id
                            ),
                        ));
                    }
                    PaymentCollectionStatusKind::Cancelled
                    | PaymentCollectionStatusKind::Unknown => {
                        return Err(non_retryable(
                            "payment.webhook_capture_conflict",
                            format!(
                                "payment collection {} cannot apply captured event from its current lifecycle state",
                                collection.id
                            ),
                        ));
                    }
                }
                self.payment_service
                    .capture_collection(
                        context.tenant_id,
                        collection.id,
                        CapturePaymentInput {
                            amount: Some(payload.amount),
                            metadata: owner_metadata(&context, &event, &payload.metadata),
                        },
                    )
                    .await
                    .map_err(map_payment_error)?;
            }
            EVENT_PAYMENT_CANCELLED => {
                match collection.status_kind() {
                    PaymentCollectionStatusKind::Cancelled => return Ok(()),
                    PaymentCollectionStatusKind::Pending
                    | PaymentCollectionStatusKind::Authorized => {}
                    PaymentCollectionStatusKind::Captured => {
                        return Err(non_retryable(
                            "payment.webhook_cancel_after_capture",
                            format!(
                                "captured payment collection {} cannot be cancelled by webhook",
                                collection.id
                            ),
                        ));
                    }
                    PaymentCollectionStatusKind::Unknown => {
                        return Err(non_retryable(
                            "payment.webhook_cancel_conflict",
                            format!(
                                "payment collection {} has an unknown lifecycle state",
                                collection.id
                            ),
                        ));
                    }
                }
                self.payment_service
                    .cancel_collection(
                        context.tenant_id,
                        collection.id,
                        CancelPaymentInput {
                            reason: Some("provider_webhook".to_string()),
                            metadata: owner_metadata(&context, &event, &payload.metadata),
                        },
                    )
                    .await
                    .map_err(map_payment_error)?;
            }
            other => {
                return Err(non_retryable(
                    "payment.webhook_event_unsupported",
                    format!("unsupported normalized payment event `{other}`"),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
struct NormalizedPaymentEvent {
    collection_id: Uuid,
    amount: Decimal,
    currency_code: String,
    metadata: Value,
}

impl NormalizedPaymentEvent {
    fn parse(
        context: &PaymentProviderEventContext,
        event: &PaymentProviderWebhookResult,
    ) -> Result<Self, PaymentProviderEventApplyError> {
        if !matches!(
            event.event_type.as_str(),
            EVENT_PAYMENT_AUTHORIZED | EVENT_PAYMENT_CAPTURED | EVENT_PAYMENT_CANCELLED
        ) {
            return Err(non_retryable(
                "payment.webhook_event_unsupported",
                format!(
                    "provider {} returned unsupported normalized event `{}`",
                    context.provider_id, event.event_type
                ),
            ));
        }
        let metadata = event.metadata.as_object().ok_or_else(|| {
            non_retryable(
                "payment.webhook_metadata_invalid",
                "normalized payment webhook metadata must be an object",
            )
        })?;
        let collection_id = Uuid::parse_str(required_string(metadata, "collection_id")?.as_str())
            .map_err(|_| {
                non_retryable(
                    "payment.webhook_collection_id_invalid",
                    "normalized payment webhook collection_id must be a UUID",
                )
            })?;
        let amount =
            Decimal::from_str(required_string(metadata, "amount")?.as_str()).map_err(|_| {
                non_retryable(
                    "payment.webhook_amount_invalid",
                    "normalized payment webhook amount must be a decimal string",
                )
            })?;
        if amount <= Decimal::ZERO {
            return Err(non_retryable(
                "payment.webhook_amount_invalid",
                "normalized payment webhook amount must be positive",
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
                "normalized payment webhook currency_code must be a three-letter code",
            ));
        }
        let domain_metadata = metadata
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        if !domain_metadata.is_object() {
            return Err(non_retryable(
                "payment.webhook_domain_metadata_invalid",
                "normalized payment webhook metadata.metadata must be an object",
            ));
        }
        Ok(Self {
            collection_id,
            amount,
            currency_code,
            metadata: domain_metadata,
        })
    }
}

fn validate_collection_identity(
    context: &PaymentProviderEventContext,
    event: &PaymentProviderWebhookResult,
    payload: &NormalizedPaymentEvent,
    collection: &crate::dto::PaymentCollectionResponse,
) -> Result<(), PaymentProviderEventApplyError> {
    if collection.tenant_id != context.tenant_id {
        return Err(non_retryable(
            "payment.webhook_tenant_mismatch",
            "payment collection does not belong to the webhook tenant",
        ));
    }
    if !collection
        .currency_code
        .eq_ignore_ascii_case(payload.currency_code.as_str())
    {
        return Err(non_retryable(
            "payment.webhook_currency_mismatch",
            format!(
                "payment collection {} currency does not match provider event",
                collection.id
            ),
        ));
    }
    if payload.amount > collection.amount {
        return Err(non_retryable(
            "payment.webhook_amount_exceeds_collection",
            format!(
                "provider event amount exceeds payment collection {} amount",
                collection.id
            ),
        ));
    }
    if let Some(provider_id) = collection.provider_id.as_deref() {
        if provider_id != context.provider_id {
            return Err(non_retryable(
                "payment.webhook_provider_mismatch",
                format!(
                    "payment collection {} is owned by another provider",
                    collection.id
                ),
            ));
        }
    }
    if matches!(
        event.event_type.as_str(),
        EVENT_PAYMENT_AUTHORIZED | EVENT_PAYMENT_CAPTURED
    ) && event
        .external_reference
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        return Err(non_retryable(
            "payment.webhook_reference_required",
            "authorized and captured webhook events require an external reference",
        ));
    }
    Ok(())
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
                format!("normalized payment webhook metadata requires `{key}`"),
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
