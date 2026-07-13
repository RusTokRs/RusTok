use rustok_payment::providers::{PaymentProviderOperationRequest, PaymentProviderRegistry};
use rustok_payment::{
    PaymentError, PaymentProviderOperationJournal, PaymentService,
    PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
    PROVIDER_OPERATION_SUCCEEDED,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::payment_orchestration::{
    PaymentOrchestrationError, PaymentOrchestrationResult,
};

const MANUAL_PROVIDER_ID: &str = "manual";

/// Resume a refund provider operation from the durable payment-owned journal.
///
/// Pending and provider-error operations are retried with the exact persisted
/// request and idempotency key. Provider-succeeded operations only finalize the
/// local journal and never invoke the adapter again.
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
        let refund = self.payment_service.get_refund(tenant_id, refund_id).await?;
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
            "refunded"
                if matches!(
                    operation.status.as_str(),
                    PROVIDER_OPERATION_COMMITTED
                        | PROVIDER_OPERATION_SUCCEEDED
                        | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
                ) => {}
            "refunded" => {
                return Err(PaymentError::Validation(format!(
                    "refund {refund_id} is already refunded, but provider operation {} has no recorded success; automatic retry is unsafe",
                    operation.id
                ))
                .into())
            }
            "cancelled" => {
                return Err(PaymentError::Validation(format!(
                    "refund {refund_id} is cancelled; provider retry is forbidden"
                ))
                .into())
            }
            status => {
                return Err(PaymentError::Validation(format!(
                    "refund {refund_id} has unsupported reconciliation status `{status}`"
                ))
                .into())
            }
        }

        if operation.status == PROVIDER_OPERATION_COMMITTED {
            return Ok(refund);
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
                .get_refund(tenant_id, refund_id)
                .await
                .map_err(Into::into);
        }

        let request: PaymentProviderOperationRequest =
            serde_json::from_value(operation.request_payload.clone()).map_err(|error| {
                PaymentError::Validation(format!(
                    "provider operation {} contains an invalid persisted request: {error}",
                    operation.id
                ))
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
                let _ = self
                    .provider_operation_journal
                    .mark_provider_error(operation.id, source.to_string())
                    .await;
                return Err(PaymentOrchestrationError::ProviderAfterRefundReservation {
                    refund_id,
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
                refund_id,
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
                refund_id,
                source: reconciliation_error(operation.id, "commit journal", source),
            });
        }

        self.payment_service
            .get_refund(tenant_id, refund_id)
            .await
            .map_err(Into::into)
    }
}

fn reconciliation_error(
    operation_id: Uuid,
    stage: &str,
    source: PaymentError,
) -> PaymentError {
    PaymentError::Validation(format!(
        "provider side effect succeeded, but failed to {stage} for operation {operation_id}: {source}"
    ))
}
