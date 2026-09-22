use rustok_payment::providers::{PaymentProviderOperationRequest, PaymentProviderRegistry};
use rustok_payment::{
    PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
    PROVIDER_OPERATION_SUCCEEDED, PaymentError, PaymentProviderOperationJournal, PaymentService,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::payment_orchestration::{PaymentOrchestrationError, PaymentOrchestrationResult};

const MANUAL_PROVIDER_ID: &str = "manual";

/// Resume a refund provider operation from the durable payment-owned journal.
/// Only pending/provider-error operations may invoke the provider again. Unknown
/// outcomes remain in reconciliation until provider state is confirmed externally.
pub struct RefundReconciliationService {
    payment_service: PaymentService,
    provider_operation_journal: PaymentProviderOperationJournal,
    payment_provider_registry: PaymentProviderRegistry,
}

impl RefundReconciliationService {
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

    pub async fn retry_refund_provider(
        &self,
        tenant_id: Uuid,
        refund_id: Uuid,
    ) -> PaymentOrchestrationResult<rustok_payment::RefundResponse> {
        let refund = self
            .payment_service
            .get_refund(tenant_id, refund_id)
            .await?;
        let collection = self
            .payment_service
            .get_collection(tenant_id, refund.payment_collection_id)
            .await?;
        let provider_id = collection
            .provider_id
            .clone()
            .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());
        let idempotency_key = format!("payment_refund:{refund_id}");
        let operation = self
            .provider_operation_journal
            .find_by_key(tenant_id, &provider_id, &idempotency_key)
            .await?
            .ok_or_else(|| {
                PaymentError::Validation(format!(
                    "refund {refund_id} has no durable provider operation to retry"
                ))
            })?;

        if operation.refund_id != Some(refund_id)
            || operation.payment_collection_id != refund.payment_collection_id
            || operation.operation != "refund"
        {
            return Err(PaymentError::Validation(format!(
                "provider operation {} is not bound to refund {refund_id}",
                operation.id
            ))
            .into());
        }

        match refund.status.as_str() {
            "pending" => {}
            "refunded" if operation.status == PROVIDER_OPERATION_COMMITTED => return Ok(refund),
            "refunded" => {
                return Err(PaymentError::provider_outcome_unknown(&provider_id, "refund").into());
            }
            "cancelled" => {
                return Err(PaymentError::Validation(format!(
                    "refund {refund_id} is cancelled; provider retry is forbidden"
                ))
                .into());
            }
            status => {
                return Err(PaymentError::Validation(format!(
                    "refund {refund_id} has unsupported reconciliation status `{status}`"
                ))
                .into());
            }
        }

        if operation.status == PROVIDER_OPERATION_COMMITTED {
            return Ok(refund);
        }
        if operation.status == PROVIDER_OPERATION_SUCCEEDED {
            self.provider_operation_journal
                .mark_committed(operation.id)
                .await
                .map_err(
                    |_| PaymentOrchestrationError::ProviderAfterRefundReservation {
                        refund_id,
                        source: PaymentError::provider_outcome_unknown(&provider_id, "refund"),
                    },
                )?;
            return self
                .payment_service
                .get_refund(tenant_id, refund_id)
                .await
                .map_err(Into::into);
        }
        if operation.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
            return Err(PaymentOrchestrationError::ProviderAfterRefundReservation {
                refund_id,
                source: PaymentError::provider_outcome_unknown(&provider_id, "refund"),
            });
        }

        let request: PaymentProviderOperationRequest =
            serde_json::from_value(operation.request_payload.clone()).map_err(|_| {
                PaymentError::provider_invalid_response(&provider_id, "refund_journal_request")
            })?;
        if request.tenant_id != tenant_id
            || request.collection_id != refund.payment_collection_id
            || request.idempotency_key.as_deref() != Some(idempotency_key.as_str())
        {
            return Err(PaymentError::Validation(format!(
                "provider operation {} persisted request identity does not match refund {refund_id}",
                operation.id
            ))
            .into());
        }

        let provider_result = match self
            .payment_provider_registry
            .execute_refund(provider_id.as_str(), request)
            .await
        {
            Ok(result) => result,
            Err(source) => {
                let journal_result = if source.requires_provider_reconciliation() {
                    self.provider_operation_journal
                        .mark_reconciliation_required(operation.id, source.to_string())
                        .await
                } else {
                    self.provider_operation_journal
                        .mark_provider_error(operation.id, source.to_string())
                        .await
                };
                let source = if journal_result.is_err() {
                    PaymentError::provider_outcome_unknown(&provider_id, "refund")
                } else {
                    source
                };
                return Err(PaymentOrchestrationError::ProviderAfterRefundReservation {
                    refund_id,
                    source,
                });
            }
        };
        let provider_result_payload = match serde_json::to_value(&provider_result) {
            Ok(payload) => payload,
            Err(_) => {
                let _ = self
                    .provider_operation_journal
                    .mark_reconciliation_required(
                        operation.id,
                        "refund provider result serialization failed after external success",
                    )
                    .await;
                return Err(PaymentOrchestrationError::ProviderAfterRefundReservation {
                    refund_id,
                    source: PaymentError::provider_outcome_unknown(&provider_id, "refund"),
                });
            }
        };

        if self
            .provider_operation_journal
            .mark_provider_succeeded(
                operation.id,
                provider_result.external_reference.clone(),
                provider_result_payload,
            )
            .await
            .is_err()
        {
            let _ = self
                .provider_operation_journal
                .mark_reconciliation_required(
                    operation.id,
                    "refund provider success could not be durably checkpointed",
                )
                .await;
            return Err(PaymentOrchestrationError::ProviderAfterRefundReservation {
                refund_id,
                source: PaymentError::provider_outcome_unknown(&provider_id, "refund"),
            });
        }
        if self
            .provider_operation_journal
            .mark_committed(operation.id)
            .await
            .is_err()
        {
            let _ = self
                .provider_operation_journal
                .mark_reconciliation_required(
                    operation.id,
                    "refund provider journal commit failed after external success",
                )
                .await;
            return Err(PaymentOrchestrationError::ProviderAfterRefundReservation {
                refund_id,
                source: PaymentError::provider_outcome_unknown(&provider_id, "refund"),
            });
        }

        self.payment_service
            .get_refund(tenant_id, refund_id)
            .await
            .map_err(Into::into)
    }
}
