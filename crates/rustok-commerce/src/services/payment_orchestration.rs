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

        match collection.status.as_str() {
            "cancelled" => return Ok(collection),
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

        if should_cancel_provider(&collection) {
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
