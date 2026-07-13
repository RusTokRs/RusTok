use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use uuid::Uuid;

use rustok_api::{
    normalize_locale_tag, PortActor, PortContext, PortError, PLATFORM_FALLBACK_LOCALE,
};
use rustok_cart::{
    in_process_cart_checkout_snapshot_port, AtomicCartCheckoutHandle, CartCheckoutSnapshotPort,
    PrepareCartCheckoutSnapshotRequest,
};

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
    cart_snapshot_port: Arc<dyn CartCheckoutSnapshotPort>,
    atomic_cart_checkout: Option<AtomicCartCheckoutHandle>,
    journal: CheckoutOperationJournal,
    lease_seconds: i64,
}

impl JournaledCheckoutService {
    pub fn new(checkout: CheckoutService, db: sea_orm::DatabaseConnection) -> Self {
        Self {
            checkout,
            cart_snapshot_port: in_process_cart_checkout_snapshot_port(db.clone()),
            atomic_cart_checkout: None,
            journal: CheckoutOperationJournal::new(db),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
        }
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    pub fn with_atomic_cart_checkout_handle(
        mut self,
        atomic_cart_checkout: AtomicCartCheckoutHandle,
    ) -> Self {
        self.atomic_cart_checkout = Some(atomic_cart_checkout);
        self
    }

    /// Executes checkout behind a durable operation identity. In the atomic
    /// path, the operation and lease exist before the cart is prepared. The
    /// final cart hash is checkpointed before order or provider side effects.
    pub async fn complete_checkout(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: CompleteCheckoutInput,
    ) -> JournaledCheckoutResult<CompleteCheckoutResponse> {
        let request_hash = checkout_request_hash(tenant_id, &input)?;
        let idempotency_key = idempotency_key.into().trim().to_string();
        if let Some(handle) = &self.atomic_cart_checkout {
            if handle.cart_id() != input.cart_id {
                return Err(CheckoutOperationError::Validation(format!(
                    "atomic cart checkout is bound to cart {}, not {}",
                    handle.cart_id(),
                    input.cart_id
                ))
                .into());
            }
        }

        let existing = self
            .journal
            .find_by_key(tenant_id, input.cart_id, idempotency_key.as_str())
            .await?;
        let operation = if let Some(existing) = existing {
            if existing.request_hash != request_hash {
                return Err(CheckoutOperationError::Conflict(format!(
                    "idempotency key `{idempotency_key}` is already bound to a different checkout request"
                ))
                .into());
            }
            existing
        } else {
            let snapshot_hash = if self.atomic_cart_checkout.is_some() {
                None
            } else {
                Some(
                    self.prepare_preview_snapshot(tenant_id, actor_id, &input)
                        .await?,
                )
            };
            self.journal
                .begin(BeginCheckoutOperation {
                    tenant_id,
                    cart_id: input.cart_id,
                    idempotency_key,
                    request_hash,
                    snapshot_hash,
                })
                .await?
        };

        if operation.status == CheckoutOperationStatus::Completed.as_str() {
            return self
                .checkout
                .complete_checkout(tenant_id, actor_id, input)
                .await
                .map_err(Into::into);
        }
        if matches!(operation.status.as_str(), "compensated" | "failed") {
            return Err(CheckoutOperationError::Conflict(format!(
                "checkout operation {} is terminal with status `{}`",
                operation.id, operation.status
            ))
            .into());
        }

        let lease_owner = format!("checkout:{actor_id}:{}", Uuid::new_v4());
        let Some(mut claimed) = self
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

        if let Some(handle) = &self.atomic_cart_checkout {
            match claimed.stage.as_str() {
                stage if stage == CheckoutOperationStage::Created.as_str() => {
                    let prepared = match handle.prepare(tenant_id, claimed.attempt_count > 1).await
                    {
                        Ok(prepared) => prepared,
                        Err(error) => {
                            let checkout_error =
                                checkout_port_error("prepare_atomic_cart_checkout", error);
                            return Err(persist_checkout_failure(
                                &self.journal,
                                tenant_id,
                                claimed.id,
                                lease_owner,
                                checkout_error,
                            )
                            .await);
                        }
                    };
                    claimed = self
                        .journal
                        .checkpoint(CheckoutOperationCheckpoint {
                            tenant_id,
                            operation_id: claimed.id,
                            lease_owner: lease_owner.clone(),
                            expected_stage: CheckoutOperationStage::Created,
                            next_stage: CheckoutOperationStage::CartLocked,
                            snapshot_hash: Some(prepared.snapshot_hash),
                            order_id: None,
                            payment_collection_id: None,
                            lease_seconds: self.lease_seconds,
                        })
                        .await?;
                }
                stage if stage == CheckoutOperationStage::CartLocked.as_str() => {
                    let prepared = match handle.prepare(tenant_id, true).await {
                        Ok(prepared) => prepared,
                        Err(error) => {
                            let checkout_error =
                                checkout_port_error("resume_atomic_cart_checkout", error);
                            return Err(persist_checkout_failure(
                                &self.journal,
                                tenant_id,
                                claimed.id,
                                lease_owner,
                                checkout_error,
                            )
                            .await);
                        }
                    };
                    if claimed.snapshot_hash.as_deref() != Some(prepared.snapshot_hash.as_str()) {
                        let checkout_error = CheckoutError::Validation(
                            "prepared cart snapshot changed after checkout lock".to_string(),
                        );
                        return Err(persist_checkout_failure(
                            &self.journal,
                            tenant_id,
                            claimed.id,
                            lease_owner,
                            checkout_error,
                        )
                        .await);
                    }
                }
                stage if stage == CheckoutOperationStage::CartCompleted.as_str() => {}
                stage => {
                    return Err(CheckoutOperationError::Conflict(format!(
                        "journaled checkout cannot resume operation {} from stage `{stage}`",
                        claimed.id
                    ))
                    .into());
                }
            }
        }

        match self
            .checkout
            .complete_checkout(tenant_id, actor_id, input)
            .await
        {
            Ok(response) => {
                claimed = match claimed.stage.as_str() {
                    stage if stage == CheckoutOperationStage::Created.as_str() => {
                        self.journal
                            .checkpoint(CheckoutOperationCheckpoint {
                                tenant_id,
                                operation_id: claimed.id,
                                lease_owner: lease_owner.clone(),
                                expected_stage: CheckoutOperationStage::Created,
                                next_stage: CheckoutOperationStage::CartCompleted,
                                snapshot_hash: claimed.snapshot_hash.clone(),
                                order_id: Some(response.order.id),
                                payment_collection_id: Some(response.payment_collection.id),
                                lease_seconds: self.lease_seconds,
                            })
                            .await?
                    }
                    stage if stage == CheckoutOperationStage::CartLocked.as_str() => {
                        self.journal
                            .checkpoint(CheckoutOperationCheckpoint {
                                tenant_id,
                                operation_id: claimed.id,
                                lease_owner: lease_owner.clone(),
                                expected_stage: CheckoutOperationStage::CartLocked,
                                next_stage: CheckoutOperationStage::CartCompleted,
                                snapshot_hash: None,
                                order_id: Some(response.order.id),
                                payment_collection_id: Some(response.payment_collection.id),
                                lease_seconds: self.lease_seconds,
                            })
                            .await?
                    }
                    stage if stage == CheckoutOperationStage::CartCompleted.as_str() => claimed,
                    stage => {
                        return Err(CheckoutOperationError::Conflict(format!(
                            "journaled checkout cannot finalize operation {} from stage `{stage}`",
                            claimed.id
                        ))
                        .into());
                    }
                };

                self.journal
                    .mark_completed(tenant_id, claimed.id, lease_owner)
                    .await?;
                Ok(response)
            }
            Err(checkout_error) => Err(persist_checkout_failure(
                &self.journal,
                tenant_id,
                claimed.id,
                lease_owner,
                checkout_error,
            )
            .await),
        }
    }

    pub fn operation_journal(&self) -> &CheckoutOperationJournal {
        &self.journal
    }

    async fn prepare_preview_snapshot(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: &CompleteCheckoutInput,
    ) -> JournaledCheckoutResult<String> {
        self.cart_snapshot_port
            .prepare_checkout_snapshot(
                checkout_snapshot_port_context(tenant_id, actor_id, input),
                PrepareCartCheckoutSnapshotRequest {
                    cart_id: input.cart_id,
                    region_id: input.region_id,
                    country_code: input.country_code.clone(),
                    locale_code: input.locale.clone(),
                    selected_shipping_option_id: input.shipping_option_id,
                    shipping_selections: input.shipping_selections.clone(),
                },
            )
            .await
            .map(|snapshot| snapshot.snapshot_hash)
            .map_err(checkout_snapshot_port_error)
    }
}

#[derive(Clone, Copy)]
enum CheckoutFailureDisposition {
    Retryable,
    CompensationRequired,
    Terminal,
}

async fn persist_checkout_failure(
    journal: &CheckoutOperationJournal,
    tenant_id: Uuid,
    operation_id: Uuid,
    lease_owner: String,
    checkout_error: CheckoutError,
) -> JournaledCheckoutError {
    let journal_result = match checkout_failure_disposition(&checkout_error) {
        CheckoutFailureDisposition::Retryable => {
            journal
                .mark_retryable_error(
                    tenant_id,
                    operation_id,
                    lease_owner,
                    checkout_error_code(&checkout_error),
                    checkout_error.to_string(),
                )
                .await
        }
        CheckoutFailureDisposition::CompensationRequired => {
            journal
                .mark_compensation_required(
                    tenant_id,
                    operation_id,
                    lease_owner,
                    checkout_error_code(&checkout_error),
                    checkout_error.to_string(),
                )
                .await
        }
        CheckoutFailureDisposition::Terminal => {
            journal
                .mark_failed(
                    tenant_id,
                    operation_id,
                    lease_owner,
                    checkout_error_code(&checkout_error),
                    checkout_error.to_string(),
                )
                .await
        }
    };

    match journal_result {
        Ok(_) => checkout_error.into(),
        Err(journal) => JournaledCheckoutError::CheckoutAndJournal {
            checkout: Box::new(checkout_error),
            journal,
        },
    }
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

fn checkout_snapshot_port_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    input: &CompleteCheckoutInput,
) -> PortContext {
    let locale = input
        .locale
        .as_deref()
        .and_then(normalize_locale_tag)
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        locale,
        format!("checkout:{}:cart:snapshot", input.cart_id),
    )
    .with_deadline(Duration::from_secs(2))
}

fn checkout_port_error(stage: &'static str, error: PortError) -> CheckoutError {
    CheckoutError::BoundaryFailure {
        stage,
        kind: error.kind,
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}

fn checkout_snapshot_port_error(error: PortError) -> JournaledCheckoutError {
    checkout_port_error("prepare_cart_checkout_snapshot", error).into()
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
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonicalize_json).collect())
        }
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
        assert!(stage_has_external_or_persisted_side_effects(
            "capture_payment"
        ));
        assert!(stage_has_external_or_persisted_side_effects(
            "mark_order_paid"
        ));
        assert!(!stage_has_external_or_persisted_side_effects(
            "resolve_context"
        ));
    }
}
