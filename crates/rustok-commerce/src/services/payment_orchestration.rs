use rust_decimal::Decimal;
use rustok_payment::dto::{
    CancelPaymentInput, CancelRefundInput, CompleteRefundInput, CreateRefundInput,
    PaymentCollectionResponse, RefundResponse,
};
use rustok_payment::error::PaymentError;
use rustok_payment::providers::{PaymentProviderOperationRequest, PaymentProviderRegistry};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use rustok_payment::PaymentService;

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
    payment_provider_registry: PaymentProviderRegistry,
}

impl PaymentOrchestrationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            payment_service: PaymentService::new(db),
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

    pub async fn cancel_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        input: CancelPaymentInput,
    ) -> PaymentOrchestrationResult<PaymentCollectionResponse> {
        let collection = self
            .payment_service
            .get_collection(tenant_id, collection_id)
            .await?;
        if collection.status != "cancelled" && collection.status != "captured" {
            let provider_id = provider_id_for_collection(&collection);
            let amount = executable_payment_amount(&collection);
            self.payment_provider_registry
                .execute_cancel(
                    provider_id.as_str(),
                    PaymentProviderOperationRequest {
                        tenant_id,
                        collection_id,
                        amount,
                        currency_code: collection.currency_code.clone(),
                        idempotency_key: Some(format!(
                            "payment_collection:{}:cancel",
                            collection_id
                        )),
                        metadata: merge_provider_context(
                            input.metadata.clone(),
                            serde_json::json!({
                                "commerce_orchestration": {
                                    "operation": "cancel_payment_collection",
                                    "reason": input.reason.clone(),
                                }
                            }),
                        ),
                    },
                )
                .await
                .map_err(PaymentOrchestrationError::Provider)?;
        }

        Ok(self
            .payment_service
            .cancel_collection(tenant_id, collection_id, input)
            .await?)
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

        self.payment_provider_registry
            .execute_refund(
                provider_id.as_str(),
                PaymentProviderOperationRequest {
                    tenant_id,
                    collection_id,
                    amount: refund.amount,
                    currency_code: refund.currency_code.clone(),
                    idempotency_key: Some(format!("payment_refund:{}", refund.id)),
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
                },
            )
            .await
            .map_err(|source| PaymentOrchestrationError::ProviderAfterRefundReservation {
                refund_id: refund.id,
                source,
            })?;

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

fn provider_id_for_collection(collection: &PaymentCollectionResponse) -> String {
    collection
        .provider_id
        .clone()
        .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string())
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
