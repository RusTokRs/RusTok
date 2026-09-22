use rustok_order::{OrderError, OrderResponse, OrderService};
use rustok_outbox::TransactionalEventBus;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use super::{
    CheckoutOperationCheckpoint, CheckoutOperationError, CheckoutOperationJournal,
    CheckoutOperationStage, CheckoutOperationStatus, DEFAULT_CHECKOUT_LEASE_SECONDS,
};

#[derive(Debug, Error)]
pub enum CheckoutOrderConfirmationError {
    #[error(transparent)]
    Order(#[from] OrderError),
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error("checkout order confirmation conflict: {0}")]
    Conflict(String),
}

pub type CheckoutOrderConfirmationResult<T> = Result<T, CheckoutOrderConfirmationError>;

pub struct CheckoutOrderConfirmationExecutor {
    order_service: OrderService,
    operation_journal: CheckoutOperationJournal,
    lease_seconds: i64,
}

impl CheckoutOrderConfirmationExecutor {
    pub fn new(db: sea_orm::DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            order_service: OrderService::new(db.clone(), event_bus),
            operation_journal: CheckoutOperationJournal::new(db),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
        }
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    /// Confirms the adopted pending order and checkpoints
    /// `order_created -> payment_ready`.
    ///
    /// If the process dies after the owner transition but before the checkout
    /// checkpoint, a replay adopts the already confirmed order and advances the
    /// same operation without repeating the lifecycle mutation.
    pub async fn confirm_and_checkpoint(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> CheckoutOrderConfirmationResult<OrderResponse> {
        let lease_owner = lease_owner.into();
        let operation = self.operation_journal.get(tenant_id, operation_id).await?;
        if operation.status != CheckoutOperationStatus::Executing.as_str() {
            return Err(CheckoutOrderConfirmationError::Conflict(format!(
                "checkout operation {} must be executing, not `{}`",
                operation.id, operation.status
            )));
        }
        let order_id = operation.order_id.ok_or_else(|| {
            CheckoutOrderConfirmationError::Conflict(format!(
                "checkout operation {} has no persisted order id",
                operation.id
            ))
        })?;
        let order = self
            .order_service
            .get_order_with_locale_fallback(tenant_id, order_id, locale, fallback_locale)
            .await?;
        validate_order_identity(&order, operation_id)?;

        if operation.stage == CheckoutOperationStage::PaymentReady.as_str() {
            if !matches!(
                order.status.as_str(),
                "confirmed" | "paid" | "shipped" | "delivered"
            ) {
                return Err(CheckoutOrderConfirmationError::Conflict(format!(
                    "checkout operation {} is payment_ready but order {} is `{}`",
                    operation.id, order.id, order.status
                )));
            }
            return Ok(order);
        }
        if operation.stage != CheckoutOperationStage::OrderCreated.as_str() {
            return Err(CheckoutOrderConfirmationError::Conflict(format!(
                "checkout operation {} cannot confirm an order from stage `{}`",
                operation.id, operation.stage
            )));
        }

        let order = match order.status.as_str() {
            "pending" => {
                self.order_service
                    .confirm_order(tenant_id, actor_id, order_id)
                    .await?
            }
            "confirmed" => order,
            status => {
                return Err(CheckoutOrderConfirmationError::Conflict(format!(
                    "order {} cannot be adopted into payment_ready from status `{status}`",
                    order_id
                )));
            }
        };
        validate_order_identity(&order, operation_id)?;

        self.operation_journal
            .checkpoint(CheckoutOperationCheckpoint {
                tenant_id,
                operation_id,
                lease_owner,
                expected_stage: CheckoutOperationStage::OrderCreated,
                next_stage: CheckoutOperationStage::PaymentReady,
                snapshot_hash: None,
                order_id: Some(order_id),
                payment_collection_id: operation.payment_collection_id,
                lease_seconds: self.lease_seconds,
            })
            .await?;

        Ok(order)
    }
}

fn validate_order_identity(
    order: &OrderResponse,
    operation_id: Uuid,
) -> CheckoutOrderConfirmationResult<()> {
    let source_operation = order
        .metadata
        .get("checkout")
        .and_then(|checkout| checkout.get("operation_id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    if source_operation != Some(operation_id) {
        return Err(CheckoutOrderConfirmationError::Conflict(format!(
            "order {} is not bound to checkout operation {}",
            order.id, operation_id
        )));
    }
    Ok(())
}
