use chrono::Utc;
use rustok_cart::CartCheckoutPort;
use rustok_inventory::InventoryReservationIdentityPort;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::providers::PaymentProviderRegistry;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::entities::checkout_operation;

use super::{CheckoutCompensationError, CheckoutCompensationService, CheckoutOperationStatus};

const DEFAULT_SWEEP_LIMIT: u64 = 25;
const MAX_SWEEP_LIMIT: u64 = 100;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckoutCompensationSweepFailure {
    pub operation_id: Uuid,
    pub manual_reconciliation: bool,
    pub error_code: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CheckoutCompensationSweepReport {
    pub scanned: usize,
    pub compensated: usize,
    pub retryable: usize,
    pub manual_reconciliation: usize,
    pub failures: Vec<CheckoutCompensationSweepFailure>,
}

pub struct CheckoutCompensationSweepService {
    db: DatabaseConnection,
    compensation: CheckoutCompensationService,
}

impl CheckoutCompensationSweepService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        reservation_port: Arc<dyn InventoryReservationIdentityPort>,
        cart_port: Arc<dyn CartCheckoutPort>,
    ) -> Self {
        Self {
            compensation: CheckoutCompensationService::new(
                db.clone(),
                event_bus,
                reservation_port,
                cart_port,
            ),
            db,
        }
    }

    pub fn with_payment_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.compensation = self
            .compensation
            .with_payment_provider_registry(payment_provider_registry);
        self
    }

    pub async fn run(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        worker_id: impl AsRef<str>,
        limit: Option<u64>,
    ) -> Result<CheckoutCompensationSweepReport, sea_orm::DbErr> {
        let limit = limit
            .unwrap_or(DEFAULT_SWEEP_LIMIT)
            .clamp(1, MAX_SWEEP_LIMIT);
        let now = Utc::now().fixed_offset();
        let claimable = Condition::any()
            .add(
                checkout_operation::Column::Status
                    .eq(CheckoutOperationStatus::CompensationRequired.as_str()),
            )
            .add(
                Condition::all()
                    .add(
                        checkout_operation::Column::Status
                            .eq(CheckoutOperationStatus::Compensating.as_str()),
                    )
                    .add(checkout_operation::Column::LeaseExpiresAt.lte(now)),
            );
        let candidates = checkout_operation::Entity::find()
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(claimable)
            .order_by_asc(checkout_operation::Column::UpdatedAt)
            .limit(limit)
            .all(&self.db)
            .await?;

        let mut report = CheckoutCompensationSweepReport {
            scanned: candidates.len(),
            ..Default::default()
        };
        for operation in candidates {
            let lease_owner = format!(
                "checkout-compensation:{}:{}",
                worker_id.as_ref(),
                operation.id
            );
            match self
                .compensation
                .compensate(tenant_id, actor_id, operation.id, lease_owner)
                .await
            {
                Ok(_) => report.compensated += 1,
                Err(CheckoutCompensationError::ManualReconciliation(_)) => {
                    report.manual_reconciliation += 1;
                    report.failures.push(CheckoutCompensationSweepFailure {
                        operation_id: operation.id,
                        manual_reconciliation: true,
                        error_code: "checkout.compensation_manual_reconciliation".to_string(),
                    });
                }
                Err(error) => {
                    report.retryable += 1;
                    report.failures.push(CheckoutCompensationSweepFailure {
                        operation_id: operation.id,
                        manual_reconciliation: false,
                        error_code: safe_error_code(&error).to_string(),
                    });
                }
            }
        }
        Ok(report)
    }
}

fn safe_error_code(error: &CheckoutCompensationError) -> &'static str {
    match error {
        CheckoutCompensationError::Boundary { .. } => "checkout.compensation_boundary_failed",
        CheckoutCompensationError::ReservationJournal(_) => {
            "checkout.compensation_inventory_failed"
        }
        CheckoutCompensationError::ManualReconciliation(_) => {
            "checkout.compensation_manual_reconciliation"
        }
        CheckoutCompensationError::Operation(_)
        | CheckoutCompensationError::Conflict(_)
        | CheckoutCompensationError::CompensationAndJournal { .. } => {
            "checkout.compensation_failed"
        }
    }
}
