use thiserror::Error;
use uuid::Uuid;

use crate::dto::{CompleteCheckoutInput, CompleteCheckoutResponse};

use super::{
    CheckoutCompensationError, CheckoutCompensationService, CheckoutOperationError,
    CheckoutOperationStatus, StagedCheckoutError, StagedCheckoutService,
};

const RECONCILIATION_REQUIRED_STATUS: &str = "reconciliation_required";

#[derive(Debug, Error)]
pub enum RecoveringStagedCheckoutError {
    #[error(transparent)]
    Staged(#[from] StagedCheckoutError),
    #[error("checkout failed: {staged}; recovery lookup failed: {journal}")]
    StagedAndJournal {
        staged: Box<StagedCheckoutError>,
        journal: CheckoutOperationError,
    },
    #[error("checkout failed: {staged}; compensation failed: {compensation}")]
    StagedAndCompensation {
        staged: Box<StagedCheckoutError>,
        compensation: CheckoutCompensationError,
    },
}

pub type RecoveringStagedCheckoutResult<T> = Result<T, RecoveringStagedCheckoutError>;

pub struct RecoveringStagedCheckoutService {
    staged: StagedCheckoutService,
    compensation: CheckoutCompensationService,
}

impl RecoveringStagedCheckoutService {
    pub fn new(
        staged: StagedCheckoutService,
        compensation: CheckoutCompensationService,
    ) -> Self {
        Self {
            staged,
            compensation,
        }
    }

    pub async fn complete_checkout(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: CompleteCheckoutInput,
    ) -> RecoveringStagedCheckoutResult<CompleteCheckoutResponse> {
        let idempotency_key = idempotency_key.into();
        let cart_id = input.cart_id;
        if let Some(current) = self
            .staged
            .operation_journal()
            .find_latest_by_cart(tenant_id, cart_id)
            .await?
        {
            if current.status == RECONCILIATION_REQUIRED_STATUS {
                return Err(reconciliation_required_error(current.id));
            }
        }

        match self
            .staged
            .complete_checkout(
                tenant_id,
                actor_id,
                idempotency_key.clone(),
                input,
            )
            .await
        {
            Ok(response) => Ok(response),
            Err(staged) => {
                let operation = self
                    .staged
                    .operation_journal()
                    .find_by_key(tenant_id, cart_id, idempotency_key.as_str())
                    .await
                    .map_err(|journal| RecoveringStagedCheckoutError::StagedAndJournal {
                        staged: Box::new(staged),
                        journal,
                    })?;
                let Some(operation) = operation else {
                    return Err(staged.into());
                };
                if operation.status == RECONCILIATION_REQUIRED_STATUS {
                    return Err(reconciliation_required_error(operation.id));
                }
                if operation.status != CheckoutOperationStatus::CompensationRequired.as_str() {
                    return Err(staged.into());
                }

                let lease_owner = format!(
                    "checkout:{}:synchronous-compensation:{}",
                    operation.id,
                    Uuid::new_v4()
                );
                match self
                    .compensation
                    .compensate(
                        tenant_id,
                        actor_id,
                        operation.id,
                        lease_owner,
                    )
                    .await
                {
                    Ok(_) => Err(staged.into()),
                    Err(compensation) => {
                        Err(RecoveringStagedCheckoutError::StagedAndCompensation {
                            staged: Box::new(staged),
                            compensation,
                        })
                    }
                }
            }
        }
    }

    pub fn staged(&self) -> &StagedCheckoutService {
        &self.staged
    }

    pub fn compensation(&self) -> &CheckoutCompensationService {
        &self.compensation
    }
}

fn reconciliation_required_error(operation_id: Uuid) -> RecoveringStagedCheckoutError {
    RecoveringStagedCheckoutError::StagedAndCompensation {
        staged: Box::new(StagedCheckoutError::Operation(
            CheckoutOperationError::Conflict(format!(
                "checkout operation {operation_id} requires reconciliation"
            )),
        )),
        compensation: CheckoutCompensationError::ManualReconciliation(format!(
            "checkout operation {operation_id} requires reconciliation"
        )),
    }
}
