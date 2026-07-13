use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::dto::{CompleteCheckoutInput, CompleteCheckoutResponse};

use super::{
    BeginCheckoutOperation, CheckoutError, CheckoutOperationCheckpoint, CheckoutOperationError,
    CheckoutOperationJournal, CheckoutOperationStage, CheckoutOperationStatus, CheckoutService,
    DEFAULT_CHECKOUT_LEASE_SECONDS,
};

#[derive(Debug, Error)]
pub enum JournaledCheckoutError {
    #[error(transparent)]
    Checkout(#[from] CheckoutError),
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error("checkout failed: {checkout}; checkout journal update failed: {journal}")]
    CheckoutAndJournal {
        checkout: Box<CheckoutError>,
        journal: CheckoutOperationError,
    },
}

pub type JournaledCheckoutResult<T> = Result<T, JournaledCheckoutError>;

pub struct JournaledCheckoutService {
    checkout: CheckoutService,
    journal: CheckoutOperationJournal,
    lease_seconds: i64,
}

impl JournaledCheckoutService {
    pub fn new(checkout: CheckoutService, db: sea_orm::DatabaseConnection) -> Self {
        Self {
            checkout,
            journal: CheckoutOperationJournal::new(db),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
        }
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    /// Executes the existing checkout orchestration behind a durable operation
    /// identity. Repeated calls with the same key and request hash either resume
    /// the abandoned lease or recover the already-completed checkout response.
    pub async fn complete_checkout(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: CompleteCheckoutInput,
    ) -> JournaledCheckoutResult<CompleteCheckoutResponse> {
        let request_hash = checkout_request_hash(tenant_id, &input)?;
        let operation = self
            .journal
            .begin(BeginCheckoutOperation {
                tenant_id,
                cart_id: input.cart_id,
                idempotency_key: idempotency_key.into(),
                request_hash,
                snapshot_hash: None,
            })
            .await?;

        if operation.status == CheckoutOperationStatus::Completed.as_str() {
            return self
                .checkout
                .complete_checkout(tenant_id, actor_id, input)
                .await
                .map_err(Into::into);
        }
        if matches!(
            operation.status.as_str(),
            "compensated" | "failed"
        ) {
            return Err(CheckoutOperationError::Conflict(format!(
                "checkout operation {} is terminal with status `{}`",
                operation.id, operation.status
            ))
            .into());
        }

        let lease_owner = format!("checkout:{actor_id}:{}", Uuid::new_v4());
        let Some(claimed) = self
            .journal
            .claim_execution(
                tenant_id,
                operation.id,
                lease_owner.as_str(),
                self.lease_seconds,
            )
            .await?
        else {
            let current = self.journal.get(tenant_id, operation.id).await?;
            if current.status == CheckoutOperationStatus::Completed.as_str() {
                return self
                    .checkout
                    .complete_checkout(tenant_id, actor_id, input)
                    .await
                    .map_err(Into::into);
            }
            return Err(CheckoutOperationError::Conflict(format!(
                "checkout operation {} is owned by another execution; status={}, stage={}, lease_owner={}",
                current.id,
                current.status,
                current.stage,
                current.lease_owner.as_deref().unwrap_or("none")
            ))
            .into());
        };

        match self
            .checkout
            .complete_checkout(tenant_id, actor_id, input)
            .await
        {
            Ok(response) => {
                if claimed.stage == CheckoutOperationStage::Created.as_str() {
                    self.journal
                        .checkpoint(CheckoutOperationCheckpoint {
                            tenant_id,
                            operation_id: claimed.id,
                            lease_owner: lease_owner.clone(),
                            expected_stage: CheckoutOperationStage::Created,
                            next_stage: CheckoutOperationStage::CartCompleted,
                            snapshot_hash: None,
                            order_id: Some(response.order.id),
                            payment_collection_id: Some(response.payment_collection.id),
                            lease_seconds: self.lease_seconds,
                        })
                        .await?;
                } else if claimed.stage != CheckoutOperationStage::CartCompleted.as_str() {
                    return Err(CheckoutOperationError::Conflict(format!(
                        "journaled checkout cannot finalize operation {} from stage `{}`",
                        claimed.id, claimed.stage
                    ))
                    .into());
                }

                self.journal
                    .mark_completed(tenant_id, claimed.id, lease_owner)
                    .await?;
                Ok(response)
            }
            Err(checkout_error) => {
                let journal_result = match checkout_failure_disposition(&checkout_error) {
                    CheckoutFailureDisposition::Retryable => {
                        self.journal
                            .mark_retryable_error(
                                tenant_id,
                                claimed.id,
                                lease_owner,
                                checkout_error_code(&checkout_error),
                                checkout_error.to_string(),
                            )
                            .await
                    }
                    CheckoutFailureDisposition::CompensationRequired => {
                        self.journal
                            .mark_compensation_required(
                                tenant_id,
                                claimed.id,
                                lease_owner,
                                checkout_error_code(&checkout_error),
                                checkout_error.to_string(),
                            )
                            .await
                    }
                    CheckoutFailureDisposition::Terminal => {
                        self.journal
                            .mark_failed(
                                tenant_id,
                                claimed.id,
                                lease_owner,
                                checkout_error_code(&checkout_error),
                                checkout_error.to_string(),
                            )
                            .await
                    }
                };

                match journal_result {
                    Ok(_) => Err(checkout_error.into()),
                    Err(journal) => Err(JournaledCheckoutError::CheckoutAndJournal {
                        checkout: Box::new(checkout_error),
                        journal,
                    }),
                }
            }
        }
    }

    pub fn operation_journal(&self) -> &CheckoutOperationJournal {
        &self.journal
    }
}

#[derive(Clone, Copy)]
enum CheckoutFailureDisposition {
    Retryable,
    CompensationRequired,
    Terminal,
}

fn checkout_failure_disposition(error: &CheckoutError) -> CheckoutFailureDisposition {
    match error {
        CheckoutError::Validation(_)
        | CheckoutError::CartNotReady(_)
        | CheckoutError::EmptyCart(_) => CheckoutFailureDisposition::Terminal,
        CheckoutError::CheckoutInProgress(_) => CheckoutFailureDisposition::Retryable,
        CheckoutError::BoundaryFailure { retryable, .. } => {
            if *retryable {
                CheckoutFailureDisposition::Retryable
            } else {
                CheckoutFailureDisposition::Terminal
            }
        }
        CheckoutError::StageFailure { stage, .. } => {
            if stage_has_external_or_persisted_side_effects(stage) {
                CheckoutFailureDisposition::CompensationRequired
            } else {
                CheckoutFailureDisposition::Retryable
            }
        }
    }
}

fn stage_has_external_or_persisted_side_effects(stage: &str) -> bool {
    matches!(
        stage,
        "confirm_order"
            | "reload_order"
            | "attach_payment_collection"
            | "create_payment_collection"
            | "load_payment_collection"
            | "execute_authorize_payment_provider"
            | "authorize_payment"
            | "create_fulfillment"
            | "execute_fulfillment_label_provider"
            | "execute_capture_payment_provider"
            | "capture_payment"
            | "mark_order_paid"
            | "complete_cart_checkout"
    )
}

fn checkout_error_code(error: &CheckoutError) -> String {
    match error {
        CheckoutError::Validation(_) => "checkout.validation",
        CheckoutError::CartNotReady(_) => "checkout.cart_not_ready",
        CheckoutError::CheckoutInProgress(_) => "checkout.in_progress",
        CheckoutError::EmptyCart(_) => "checkout.empty_cart",
        CheckoutError::BoundaryFailure { code, .. } => code.as_str(),
        CheckoutError::StageFailure { stage, .. } => stage,
    }
    .to_string()
}

fn checkout_request_hash(
    tenant_id: Uuid,
    input: &CompleteCheckoutInput,
) -> JournaledCheckoutResult<String> {
    let value = serde_json::to_value(input).map_err(|error| {
        CheckoutOperationError::Validation(format!(
            "failed to serialize checkout request for hashing: {error}"
        ))
    })?;
    let canonical = canonicalize_json(value);
    let payload = serde_json::to_vec(&(tenant_id, canonical)).map_err(|error| {
        CheckoutOperationError::Validation(format!(
            "failed to encode checkout request hash payload: {error}"
        ))
    })?;
    let digest = Sha256::digest(payload);
    Ok(format!("{digest:x}"))
}

fn canonicalize_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(values) => {
            let ordered = values
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect::<std::collections::BTreeMap<_, _>>();
            serde_json::Value::Object(ordered.into_iter().collect())
        }
        serde_json::Value::Array(values) => serde_json::Value::Array(
            values.into_iter().map(canonicalize_json).collect(),
        ),
        value => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_json_hash_is_independent_of_object_key_order() {
        let first = canonicalize_json(serde_json::json!({"b": 2, "a": {"d": 4, "c": 3}}));
        let second = canonicalize_json(serde_json::json!({"a": {"c": 3, "d": 4}, "b": 2}));
        assert_eq!(first, second);
    }

    #[test]
    fn side_effect_stage_classification_is_fail_closed() {
        assert!(stage_has_external_or_persisted_side_effects("capture_payment"));
        assert!(stage_has_external_or_persisted_side_effects("mark_order_paid"));
        assert!(!stage_has_external_or_persisted_side_effects("resolve_context"));
    }
}
