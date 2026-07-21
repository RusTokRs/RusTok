use rustok_api::PortError;
#[cfg(test)]
use rustok_api::PortErrorKind;
use rustok_cart::AtomicCartCheckoutHandle;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

use crate::dto::{CompleteCheckoutInput, CompleteCheckoutResponse};

use super::{
    BeginCheckoutOperation, CheckoutCompletedState, CheckoutError,
    CheckoutMarketplaceAllocationError, CheckoutMarketplaceCommissionError,
    CheckoutMarketplaceEconomicsCheckpointError, CheckoutOperationCheckpoint,
    CheckoutOperationError, CheckoutOperationJournal, CheckoutOperationStage,
    CheckoutOperationStatus, CheckoutPlanBuilder, CheckoutStagePipeline,
    CheckoutStagePipelineError, DEFAULT_CHECKOUT_LEASE_SECONDS,
};

#[derive(Debug, Error)]
pub enum StagedCheckoutError {
    #[error(transparent)]
    Checkout(#[from] CheckoutError),
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    Pipeline(#[from] CheckoutStagePipelineError),
    #[error("checkout failed: {checkout}; checkout journal update failed: {journal}")]
    CheckoutAndJournal {
        checkout: Box<CheckoutError>,
        journal: CheckoutOperationError,
    },
    #[error("checkout pipeline failed: {pipeline}; checkout journal update failed: {journal}")]
    PipelineAndJournal {
        pipeline: Box<CheckoutStagePipelineError>,
        journal: CheckoutOperationError,
    },
}

pub type StagedCheckoutResult<T> = Result<T, StagedCheckoutError>;

pub struct StagedCheckoutService {
    plan_builder: CheckoutPlanBuilder,
    pipeline: CheckoutStagePipeline,
    atomic_cart_checkout: AtomicCartCheckoutHandle,
    journal: CheckoutOperationJournal,
    lease_seconds: i64,
}

impl StagedCheckoutService {
    pub fn new(
        plan_builder: CheckoutPlanBuilder,
        pipeline: CheckoutStagePipeline,
        atomic_cart_checkout: AtomicCartCheckoutHandle,
        db: sea_orm::DatabaseConnection,
    ) -> Self {
        Self {
            plan_builder,
            pipeline,
            atomic_cart_checkout,
            journal: CheckoutOperationJournal::new(db),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
        }
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    pub async fn complete_checkout(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: CompleteCheckoutInput,
    ) -> StagedCheckoutResult<CompleteCheckoutResponse> {
        if self.atomic_cart_checkout.cart_id() != input.cart_id {
            return Err(CheckoutOperationError::Validation(format!(
                "atomic cart checkout is bound to cart {}, not {}",
                self.atomic_cart_checkout.cart_id(),
                input.cart_id
            ))
            .into());
        }
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
                .load_completed_response(tenant_id, actor_id, operation.id)
                .await;
        }
        if matches!(operation.status.as_str(), "compensated" | "failed") {
            return Err(CheckoutOperationError::Conflict(format!(
                "checkout operation {} is terminal with status `{}`",
                operation.id, operation.status
            ))
            .into());
        }
        if matches!(
            operation.status.as_str(),
            "compensation_required" | "compensating"
        ) {
            return Err(CheckoutOperationError::Conflict(format!(
                "checkout operation {} requires compensation before it can resume",
                operation.id
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
                    .load_completed_response(tenant_id, actor_id, current.id)
                    .await;
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

        let prepared = match self
            .atomic_cart_checkout
            .prepare(
                tenant_id,
                claimed.stage != CheckoutOperationStage::Created.as_str()
                    || claimed.attempt_count > 1,
            )
            .await
        {
            Ok(prepared) => prepared,
            Err(error) => {
                let checkout = checkout_port_error("prepare_atomic_cart_checkout", error);
                return Err(self
                    .persist_checkout_failure(tenant_id, claimed.id, lease_owner, checkout)
                    .await);
            }
        };

        if claimed.stage == CheckoutOperationStage::Created.as_str() {
            claimed = self
                .journal
                .checkpoint(CheckoutOperationCheckpoint {
                    tenant_id,
                    operation_id: claimed.id,
                    lease_owner: lease_owner.clone(),
                    expected_stage: CheckoutOperationStage::Created,
                    next_stage: CheckoutOperationStage::CartLocked,
                    snapshot_hash: Some(prepared.snapshot_hash.clone()),
                    order_id: None,
                    payment_collection_id: None,
                    lease_seconds: self.lease_seconds,
                })
                .await?;
        } else if claimed.snapshot_hash.as_deref() != Some(prepared.snapshot_hash.as_str()) {
            let checkout = CheckoutError::Validation(
                "prepared cart snapshot changed after checkout lock".to_string(),
            );
            return Err(self
                .persist_checkout_failure(tenant_id, claimed.id, lease_owner, checkout)
                .await);
        }

        let initial_plan = if claimed.stage == CheckoutOperationStage::CartLocked.as_str() {
            match self
                .plan_builder
                .build(tenant_id, actor_id, claimed.id, &input, &prepared)
                .await
            {
                Ok(plan) => Some(plan),
                Err(checkout) => {
                    return Err(self
                        .persist_checkout_failure(tenant_id, claimed.id, lease_owner, checkout)
                        .await);
                }
            }
        } else {
            None
        };

        match self
            .pipeline
            .advance_to_completed(
                tenant_id,
                actor_id,
                claimed.id,
                lease_owner.clone(),
                &prepared,
                initial_plan,
            )
            .await
        {
            Ok(completed) => Ok(completed_response(completed)),
            Err(pipeline) => Err(self
                .persist_pipeline_failure(tenant_id, claimed.id, lease_owner, pipeline)
                .await),
        }
    }

    pub fn operation_journal(&self) -> &CheckoutOperationJournal {
        &self.journal
    }

    async fn load_completed_response(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
    ) -> StagedCheckoutResult<CompleteCheckoutResponse> {
        let prepared = self
            .atomic_cart_checkout
            .prepare(tenant_id, true)
            .await
            .map_err(|error| checkout_port_error("read_completed_cart_checkout", error))?;
        let operation = self.journal.get(tenant_id, operation_id).await?;
        if operation.snapshot_hash.as_deref() != Some(prepared.snapshot_hash.as_str()) {
            return Err(CheckoutOperationError::Conflict(format!(
                "completed checkout operation {} no longer matches the cart snapshot",
                operation.id
            ))
            .into());
        }
        let completed = self
            .pipeline
            .advance_to_completed(
                tenant_id,
                actor_id,
                operation_id,
                format!("checkout:{operation_id}:completed-read"),
                &prepared,
                None,
            )
            .await?;
        Ok(completed_response(completed))
    }

    async fn persist_checkout_failure(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        lease_owner: String,
        checkout: CheckoutError,
    ) -> StagedCheckoutError {
        let journal_result = match checkout_failure_disposition(&checkout) {
            FailureDisposition::Retryable => {
                self.journal
                    .mark_retryable_error(
                        tenant_id,
                        operation_id,
                        lease_owner,
                        checkout_error_code(&checkout),
                        checkout.to_string(),
                    )
                    .await
            }
            FailureDisposition::CompensationRequired => {
                self.journal
                    .mark_compensation_required(
                        tenant_id,
                        operation_id,
                        lease_owner,
                        checkout_error_code(&checkout),
                        checkout.to_string(),
                    )
                    .await
            }
        };
        match journal_result {
            Ok(_) => checkout.into(),
            Err(journal) => StagedCheckoutError::CheckoutAndJournal {
                checkout: Box::new(checkout),
                journal,
            },
        }
    }

    async fn persist_pipeline_failure(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        lease_owner: String,
        pipeline: CheckoutStagePipelineError,
    ) -> StagedCheckoutError {
        let journal_result = match pipeline_failure_disposition(&pipeline) {
            FailureDisposition::Retryable => {
                self.journal
                    .mark_retryable_error(
                        tenant_id,
                        operation_id,
                        lease_owner,
                        pipeline_error_code(&pipeline),
                        pipeline.to_string(),
                    )
                    .await
            }
            FailureDisposition::CompensationRequired => {
                self.journal
                    .mark_compensation_required(
                        tenant_id,
                        operation_id,
                        lease_owner,
                        pipeline_error_code(&pipeline),
                        pipeline.to_string(),
                    )
                    .await
            }
        };
        match journal_result {
            Ok(_) => pipeline.into(),
            Err(journal) => StagedCheckoutError::PipelineAndJournal {
                pipeline: Box::new(pipeline),
                journal,
            },
        }
    }
}

#[derive(Clone, Copy)]
enum FailureDisposition {
    Retryable,
    CompensationRequired,
}

fn checkout_failure_disposition(error: &CheckoutError) -> FailureDisposition {
    match error {
        CheckoutError::CheckoutInProgress(_) => FailureDisposition::Retryable,
        CheckoutError::BoundaryFailure {
            retryable: true, ..
        } => FailureDisposition::Retryable,
        CheckoutError::Validation(_)
        | CheckoutError::CartNotReady(_)
        | CheckoutError::EmptyCart(_)
        | CheckoutError::BoundaryFailure { .. }
        | CheckoutError::StageFailure { .. } => FailureDisposition::CompensationRequired,
    }
}

fn pipeline_failure_disposition(error: &CheckoutStagePipelineError) -> FailureDisposition {
    match error {
        CheckoutStagePipelineError::MarketplaceAllocation(
            CheckoutMarketplaceAllocationError::Boundary {
                retryable: true, ..
            },
        )
        | CheckoutStagePipelineError::MarketplaceCommission(
            CheckoutMarketplaceCommissionError::Boundary {
                retryable: true, ..
            },
        )
        | CheckoutStagePipelineError::MarketplaceEconomicsCheckpoint(
            CheckoutMarketplaceEconomicsCheckpointError::Database(_),
        ) => FailureDisposition::Retryable,
        _ => FailureDisposition::CompensationRequired,
    }
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

fn pipeline_error_code(error: &CheckoutStagePipelineError) -> String {
    match error {
        CheckoutStagePipelineError::MarketplaceAllocation(
            CheckoutMarketplaceAllocationError::Boundary { code, .. },
        )
        | CheckoutStagePipelineError::MarketplaceCommission(
            CheckoutMarketplaceCommissionError::Boundary { code, .. },
        ) => code.clone(),
        CheckoutStagePipelineError::MarketplaceEconomicsCheckpoint(
            CheckoutMarketplaceEconomicsCheckpointError::Database(_),
        ) => "checkout.marketplace_economics_checkpoint_unavailable".to_string(),
        CheckoutStagePipelineError::MarketplaceEconomicsCheckpoint(_) => {
            "checkout.marketplace_economics_checkpoint_conflict".to_string()
        }
        _ => "checkout.pipeline_failed".to_string(),
    }
}

fn checkout_request_hash(
    tenant_id: Uuid,
    input: &CompleteCheckoutInput,
) -> Result<String, CheckoutOperationError> {
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
    Ok(Sha256::digest(payload)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(values) => {
            let ordered = values
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(ordered.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        value => value,
    }
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

fn completed_response(completed: CheckoutCompletedState) -> CompleteCheckoutResponse {
    let checkout = completed.checkout;
    let fulfillment = if checkout.fulfillments.len() == 1 {
        checkout.fulfillments.first().cloned()
    } else {
        None
    };
    CompleteCheckoutResponse {
        cart: completed.cart,
        order: checkout.order,
        payment_collection: checkout.payment_collection,
        fulfillment,
        fulfillments: checkout.fulfillments,
        context: checkout.plan.payload.context,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_hash_is_independent_of_metadata_key_order() {
        let tenant_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
        let cart_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
        let input = |metadata| CompleteCheckoutInput {
            cart_id,
            shipping_option_id: None,
            shipping_selections: None,
            region_id: None,
            country_code: None,
            locale: None,
            create_fulfillment: false,
            metadata,
        };
        assert_eq!(
            checkout_request_hash(tenant_id, &input(serde_json::json!({"b": 2, "a": 1}))).unwrap(),
            checkout_request_hash(tenant_id, &input(serde_json::json!({"a": 1, "b": 2}))).unwrap()
        );
    }

    #[test]
    fn retryable_boundary_errors_do_not_force_compensation() {
        let error = CheckoutError::BoundaryFailure {
            stage: "inventory",
            kind: PortErrorKind::Unavailable,
            code: "inventory.unavailable".to_string(),
            message: "inventory unavailable".to_string(),
            retryable: true,
        };
        assert!(matches!(
            checkout_failure_disposition(&error),
            FailureDisposition::Retryable
        ));
    }

    #[test]
    fn retryable_marketplace_boundary_does_not_force_compensation() {
        let error = CheckoutStagePipelineError::MarketplaceCommission(
            CheckoutMarketplaceCommissionError::Boundary {
                code: "marketplace_commission.storage_unavailable".to_string(),
                message: "temporarily unavailable".to_string(),
                retryable: true,
            },
        );
        assert!(matches!(
            pipeline_failure_disposition(&error),
            FailureDisposition::Retryable
        ));
    }
}
