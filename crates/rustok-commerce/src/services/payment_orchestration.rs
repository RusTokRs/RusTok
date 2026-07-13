use rust_decimal::Decimal;
use rustok_payment::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CancelRefundInput, CapturePaymentInput,
    CompleteRefundInput, CreateRefundInput, PaymentCollectionResponse, RefundResponse,
};
use rustok_payment::error::PaymentError;
use rustok_payment::providers::{PaymentProviderOperationRequest, PaymentProviderRegistry};
use rustok_payment::{
    BeginProviderOperation, PaymentProviderOperationJournal, PaymentService,
    PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
    PROVIDER_OPERATION_SUCCEEDED,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;
use validator::Validate;

use super::journaled_payment_provider::{
    execute_journaled_provider_operation, local_persistence_after_provider_error,
    mark_journal_committed, mark_local_persistence_failed,
};

const MANUAL_PROVIDER_ID: &str = "manual";

#[derive(Debug, Error)]
pub enum PaymentOrchestrationError {
    #[error("payment provider error: {0}")]
    Provider(#[source] PaymentError),
    #[error("payment provider error after refund {refund_id} was reserved: {source}")]
    ProviderAfterRefundReservation {
        refund_id: Uuid,
        #[source]
        source: PaymentError,
    },
    #[error("payment error: {0}")]
    Payment(#[from] PaymentError),
}

pub type PaymentOrchestrationResult<T> = Result<T, PaymentOrchestrationError>;

/// Commerce-owned payment orchestration for post-checkout operator paths.
///
/// The umbrella module may choose *when* payment side effects are needed, but it
/// must route provider calls through the payment owner registry. Refund capacity
/// is reserved in the payment owner before a provider side effect is attempted so
/// concurrent requests cannot externally refund more than the captured amount.
pub struct PaymentOrchestrationService {
    payment_service: PaymentService,
    provider_operation_journal: PaymentProviderOperationJournal,
    payment_provider_registry: PaymentProviderRegistry,
}

impl PaymentOrchestrationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            payment_service: PaymentService::new(db.clone()),
            provider_operation_journal: PaymentProviderOperationJournal::new(db),
            payment_provider_registry: PaymentProviderRegistry::with_manual_provider(),
        }
    }

    pub fn with_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.payment_provider_registry = payment_provider_registry;
        self
    }

    pub async fn authorize_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        input: AuthorizePaymentInput,
    ) -> PaymentOrchestrationResult<PaymentCollectionResponse> {
        input
            .validate()
            .map_err(|error| PaymentError::Validation(error.to_string()))?;

        let collection = self
            .payment_service
            .get_collection(tenant_id, collection_id)
            .await?;
        let provider_id = input
            .provider_id
            .clone()
            .or_else(|| collection.provider_id.clone())
            .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());
        let idempotency_key = format!("payment_collection:{collection_id}:authorize");

        match collection.status.as_str() {
            "authorized" | "captured" => {
                if let Some(operation) = self
                    .provider_operation_journal
                    .find_by_key(tenant_id, &provider_id, &idempotency_key)
                    .await?
                {
                    if matches!(
                        operation.status.as_str(),
                        PROVIDER_OPERATION_SUCCEEDED | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
                    ) {
                        mark_journal_committed(
                            &self.provider_operation_journal,
                            operation.id,
                            "authorize",
                        )
                        .await?;
                    }
                }
                return Ok(collection);
            }
            "pending" => {}
            status => {
                return Err(PaymentError::InvalidTransition {
                    from: status.to_string(),
                    to: "authorized".to_string(),
                }
                .into());
            }
        }

        let AuthorizePaymentInput {
            provider_id: _,
            provider_payment_id,
            amount,
            metadata,
        } = input;
        let requested_amount = amount.unwrap_or(collection.amount);
        let provider_request = PaymentProviderOperationRequest {
            tenant_id,
            collection_id,
            amount: requested_amount,
            currency_code: collection.currency_code.clone(),
            idempotency_key: Some(idempotency_key),
            metadata: merge_provider_context(
                metadata.clone(),
                serde_json::json!({
                    "commerce_orchestration": {
                        "operation": "authorize_payment_collection",
                        "requested_provider_payment_id": provider_payment_id.clone(),
                    }
                }),
            ),
        };
        let journaled = execute_journaled_provider_operation(
            &self.provider_operation_journal,
            &self.payment_provider_registry,
            "authorize",
            None,
            provider_id.as_str(),
            provider_request,
        )
        .await?;
        let provider_result = journaled.result;

        match self
            .payment_service
            .authorize_collection(
                tenant_id,
                collection_id,
                AuthorizePaymentInput {
                    provider_id: Some(provider_result.provider_id),
                    provider_payment_id: provider_result.external_reference.or(provider_payment_id),
                    amount: Some(provider_result.authorized_amount),
                    metadata: merge_provider_context(metadata, provider_result.metadata),
                },
            )
            .await
        {
            Ok(collection) => {
                mark_journal_committed(
                    &self.provider_operation_journal,
                    journaled.operation_id,
                    "authorize",
                )
                .await?;
                Ok(collection)
            }
            Err(source) => {
                mark_local_persistence_failed(
                    &self.provider_operation_journal,
                    journaled.operation_id,
                    "authorize",
                    &source,
                )
                .await;
                Err(local_persistence_after_provider_error(
                    journaled.operation_id,
                    "authorize",
                    source,
                ))
            }
        }
    }

    pub async fn capture_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        input: CapturePaymentInput,
    ) -> PaymentOrchestrationResult<PaymentCollectionResponse> {
        let collection = self
            .payment_service
            .get_collection(tenant_id, collection_id)
            .await?;
        let provider_id = provider_id_for_collection(&collection);
        let idempotency_key = format!("payment_collection:{collection_id}:capture");

        match collection.status.as_str() {
            "captured" => {
                if let Some(operation) = self
                    .provider_operation_journal
                    .find_by_key(tenant_id, &provider_id, &idempotency_key)
                    .await?
                {
                    if matches!(
                        operation.status.as_str(),
                        PROVIDER_OPERATION_SUCCEEDED | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
                    ) {
                        mark_journal_committed(
                            &self.provider_operation_journal,
                            operation.id,
                            "capture",
                        )
                        .await?;
                    }
                }
                return Ok(collection);
            }
            "authorized" => {}
            status => {
                return Err(PaymentError::InvalidTransition {
                    from: status.to_string(),
                    to: "captured".to_string(),
                }
                .into());
            }
        }

        let CapturePaymentInput { amount, metadata } = input;
        let requested_amount = amount.unwrap_or(collection.authorized_amount);
        let provider_request = PaymentProviderOperationRequest {
            tenant_id,
            collection_id,
            amount: requested_amount,
            currency_code: collection.currency_code.clone(),
            idempotency_key: Some(idempotency_key),
            metadata: merge_provider_context(
                metadata.clone(),
                serde_json::json!({
                    "commerce_orchestration": {
                        "operation": "capture_payment_collection"
                    }
                }),
            ),
        };
        let journaled = execute_journaled_provider_operation(
            &self.provider_operation_journal,
            &self.payment_provider_registry,
            "capture",
            None,
            provider_id.as_str(),
            provider_request,
        )
        .await?;
        let provider_result = journaled.result;

        match self
            .payment_service
            .capture_collection(
                tenant_id,
                collection_id,
                CapturePaymentInput {
                    amount: Some(provider_result.captured_amount),
                    metadata: merge_provider_context(metadata, provider_result.metadata),
                },
            )
            .await
        {
            Ok(collection) => {
                mark_journal_committed(
                    &self.provider_operation_journal,
                    journaled.operation_id,
                    "capture",
                )
                .await?;
                Ok(collection)
            }
            Err(source) => {
                mark_local_persistence_failed(
                    &self.provider_operation_journal,
                    journaled.operation_id,
                    "capture",
                    &source,
                )
                .await;
                Err(local_persistence_after_provider_error(
                    journaled.operation_id,
                    "capture",
                    source,
                ))
            }
        }
    }

    pub async fn cancel_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        mut input: CancelPaymentInput,
    ) -> PaymentOrchestrationResult<PaymentCollectionResponse> {
        let collection = self
            .payment_service
            .get_collection(tenant_id, collection_id)
            .await?;
        let provider_id = provider_id_for_collection(&collection);
        let idempotency_key = format!("payment_collection:{collection_id}:cancel");

        match collection.status.as_str() {
            "cancelled" => {
                if let Some(operation) = self
                    .provider_operation_journal
                    .find_by_key(tenant_id, &provider_id, &idempotency_key)
                    .await?
                {
                    if matches!(
                        operation.status.as_str(),
                        PROVIDER_OPERATION_SUCCEEDED | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
                    ) {
                        mark_journal_committed(
                            &self.provider_operation_journal,
                            operation.id,
                            "cancel",
                        )
                        .await?;
                    }
                }
                return Ok(collection);
            }
            "captured" => {
                return Err(PaymentError::InvalidTransition {
                    from: collection.status,
                    to: "cancelled".to_string(),
                }
                .into());
            }
            "pending" | "authorized" => {}
            status => {
                return Err(PaymentError::InvalidTransition {
                    from: status.to_string(),
                    to: "cancelled".to_string(),
                }
                .into());
            }
        }

        let provider_operation_id = if should_cancel_provider(&collection) {
            let amount = executable_payment_amount(&collection);
            let provider_request = PaymentProviderOperationRequest {
                tenant_id,
                collection_id,
                amount,
                currency_code: collection.currency_code.clone(),
                idempotency_key: Some(idempotency_key),
                metadata: merge_provider_context(
                    input.metadata.clone(),
                    serde_json::json!({
                        "commerce_orchestration": {
                            "operation": "cancel_payment_collection",
                            "reason": input.reason.clone(),
                        }
                    }),
                ),
            };
            let journaled = execute_journaled_provider_operation(
                &self.provider_operation_journal,
                &self.payment_provider_registry,
                "cancel",
                None,
                provider_id.as_str(),
                provider_request,
            )
            .await?;
            input.metadata = merge_provider_context(input.metadata, journaled.result.metadata);
            Some(journaled.operation_id)
        } else {
            None
        };

        match self
            .payment_service
            .cancel_collection(tenant_id, collection_id, input)
            .await
        {
            Ok(collection) => {
                if let Some(operation_id) = provider_operation_id {
                    mark_journal_committed(
                        &self.provider_operation_journal,
                        operation_id,
                        "cancel",
                    )
                    .await?;
                }
                Ok(collection)
            }
            Err(source) => {
                if let Some(operation_id) = provider_operation_id {
                    mark_local_persistence_failed(
                        &self.provider_operation_journal,
                        operation_id,
                        "cancel",
                        &source,
                    )
                    .await;
                    Err(local_persistence_after_provider_error(
                        operation_id,
                        "cancel",
                        source,
                    ))
                } else {
                    Err(source.into())
                }
            }
        }
    }

    pub async fn create_refund(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        input: CreateRefundInput,
    ) -> PaymentOrchestrationResult<RefundResponse> {
        let collection = self
            .payment_service
            .get_collection(tenant_id, collection_id)
            .await?;
        let provider_id = provider_id_for_collection(&collection);

        // Reserve refundable capacity before invoking an external provider. If the
        // provider outcome is unknown, keep the pending refund for reconciliation;
        // automatically cancelling it could allow a duplicate external refund.
        let refund = self
            .payment_service
            .create_refund(tenant_id, collection_id, input.clone())
            .await?;
        let idempotency_key = format!("payment_refund:{}", refund.id);
        let provider_request = PaymentProviderOperationRequest {
            tenant_id,
            collection_id,
            amount: refund.amount,
            currency_code: refund.currency_code.clone(),
            idempotency_key: Some(idempotency_key.clone()),
            metadata: merge_provider_context(
                input.metadata,
                serde_json::json!({
                    "commerce_orchestration": {
                        "operation": "create_refund",
                        "refund_id": refund.id,
                        "reason": input.reason,
                    }
                }),
            ),
        };
        let request_payload = serde_json::to_value(&provider_request).map_err(|error| {
            PaymentError::Validation(format!(
                "failed to serialize refund provider request: {error}"
            ))
        })?;
        let operation = self
            .provider_operation_journal
            .begin(BeginProviderOperation {
                tenant_id,
                payment_collection_id: collection_id,
                refund_id: Some(refund.id),
                operation: "refund".to_string(),
                provider_id: provider_id.clone(),
                idempotency_key,
                request_payload,
            })
            .await?;

        if operation.status == PROVIDER_OPERATION_COMMITTED {
            return self
                .payment_service
                .get_refund(tenant_id, refund.id)
                .await
                .map_err(Into::into);
        }
        if matches!(
            operation.status.as_str(),
            PROVIDER_OPERATION_SUCCEEDED | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            self.provider_operation_journal
                .mark_committed(operation.id)
                .await?;
            return self
                .payment_service
                .get_refund(tenant_id, refund.id)
                .await
                .map_err(Into::into);
        }

        let provider_result = match self
            .payment_provider_registry
            .execute_refund(provider_id.as_str(), provider_request)
            .await
        {
            Ok(result) => result,
            Err(source) => {
                let _ = self
                    .provider_operation_journal
                    .mark_provider_error(operation.id, source.to_string())
                    .await;
                return Err(PaymentOrchestrationError::ProviderAfterRefundReservation {
                    refund_id: refund.id,
                    source,
                });
            }
        };
        let provider_result_payload = serde_json::to_value(&provider_result).map_err(|error| {
            PaymentError::Validation(format!(
                "failed to serialize refund provider result: {error}"
            ))
        })?;

        if let Err(source) = self
            .provider_operation_journal
            .mark_provider_succeeded(
                operation.id,
                provider_result.external_reference.clone(),
                provider_result_payload,
            )
            .await
        {
            return Err(PaymentOrchestrationError::ProviderAfterRefundReservation {
                refund_id: refund.id,
                source: reconciliation_error(operation.id, "record provider success", source),
            });
        }

        if let Err(source) = self
            .provider_operation_journal
            .mark_committed(operation.id)
            .await
        {
            let _ = self
                .provider_operation_journal
                .mark_reconciliation_required(operation.id, source.to_string())
                .await;
            return Err(PaymentOrchestrationError::ProviderAfterRefundReservation {
                refund_id: refund.id,
                source: reconciliation_error(operation.id, "commit journal", source),
            });
        }

        Ok(refund)
    }

    pub async fn complete_refund(
        &self,
        tenant_id: Uuid,
        refund_id: Uuid,
        input: CompleteRefundInput,
    ) -> PaymentOrchestrationResult<RefundResponse> {
        Ok(self
            .payment_service
            .complete_refund(tenant_id, refund_id, input)
            .await?)
    }

    pub async fn cancel_refund(
        &self,
        tenant_id: Uuid,
        refund_id: Uuid,
        input: CancelRefundInput,
    ) -> PaymentOrchestrationResult<RefundResponse> {
        Ok(self
            .payment_service
            .cancel_refund(tenant_id, refund_id, input)
            .await?)
    }
}

fn reconciliation_error(operation_id: Uuid, stage: &str, source: PaymentError) -> PaymentError {
    PaymentError::Validation(format!(
        "provider side effect succeeded, but failed to {stage} for operation {operation_id}: {source}"
    ))
}

fn provider_id_for_collection(collection: &PaymentCollectionResponse) -> String {
    collection
        .provider_id
        .clone()
        .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string())
}

fn should_cancel_provider(collection: &PaymentCollectionResponse) -> bool {
    collection.status == "authorized"
        || collection.authorized_amount > Decimal::ZERO
        || collection.provider_id.is_some()
}

fn executable_payment_amount(collection: &PaymentCollectionResponse) -> Decimal {
    if collection.captured_amount > Decimal::ZERO {
        collection.captured_amount
    } else if collection.authorized_amount > Decimal::ZERO {
        collection.authorized_amount
    } else {
        collection.amount
    }
}

fn merge_provider_context(current: Value, patch: Value) -> Value {
    match (current, patch) {
        (Value::Object(mut current), Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, patch) => patch,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal_macros::dec;

    use super::*;

    fn collection(status: &str) -> PaymentCollectionResponse {
        let now = Utc::now();
        PaymentCollectionResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            cart_id: Some(Uuid::new_v4()),
            order_id: None,
            customer_id: None,
            status: status.to_string(),
            currency_code: "USD".to_string(),
            amount: dec!(100),
            authorized_amount: Decimal::ZERO,
            captured_amount: Decimal::ZERO,
            refunded_amount: Decimal::ZERO,
            provider_id: None,
            cancellation_reason: None,
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            authorized_at: None,
            captured_at: None,
            cancelled_at: None,
            payments: Vec::new(),
            refunds: Vec::new(),
        }
    }

    #[test]
    fn pending_collection_without_provider_state_skips_external_cancel() {
        assert!(!should_cancel_provider(&collection("pending")));
    }

    #[test]
    fn authorized_or_provider_bound_collection_requires_external_cancel() {
        let authorized = collection("authorized");
        assert!(should_cancel_provider(&authorized));

        let mut provider_bound = collection("pending");
        provider_bound.provider_id = Some("stripe".to_string());
        assert!(should_cancel_provider(&provider_bound));

        let mut amount_bound = collection("pending");
        amount_bound.authorized_amount = dec!(25);
        assert!(should_cancel_provider(&amount_bound));
    }

    #[test]
    fn executable_amount_prefers_captured_then_authorized_then_collection() {
        let mut value = collection("pending");
        assert_eq!(executable_payment_amount(&value), dec!(100));

        value.authorized_amount = dec!(80);
        assert_eq!(executable_payment_amount(&value), dec!(80));

        value.captured_amount = dec!(60);
        assert_eq!(executable_payment_amount(&value), dec!(60));
    }
}
