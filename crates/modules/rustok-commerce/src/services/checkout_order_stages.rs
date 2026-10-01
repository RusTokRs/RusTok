use std::sync::Arc;

use rustok_api::PortActor;
use rustok_cart::PreparedCartCheckoutSnapshot;
use rustok_inventory::InventoryReservationIdentityPort;
use rustok_order::{OrderResponse, OrderService};
use rustok_outbox::TransactionalEventBus;
use thiserror::Error;
use uuid::Uuid;

use super::{
    CheckoutInventoryExecutionError, CheckoutInventoryReservationExecutor, CheckoutOperationError,
    CheckoutOperationJournal, CheckoutOperationStage, CheckoutOrderConfirmationError,
    CheckoutOrderConfirmationExecutor, CheckoutOrderCreationError, CheckoutOrderCreationExecutor,
    CheckoutOrderPlanError, CheckoutOrderPlanJournal, CheckoutOrderPlanPayload,
    CheckoutOrderPlanRecord, DEFAULT_CHECKOUT_LEASE_SECONDS,
};

#[derive(Clone, Debug)]
pub struct CheckoutPaymentReadyState {
    pub operation_id: Uuid,
    pub order: OrderResponse,
    pub plan: CheckoutOrderPlanRecord,
}

#[derive(Debug, Error)]
pub enum CheckoutOrderStageError {
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    Plan(#[from] CheckoutOrderPlanError),
    #[error(transparent)]
    Inventory(#[from] CheckoutInventoryExecutionError),
    #[error(transparent)]
    Creation(#[from] CheckoutOrderCreationError),
    #[error(transparent)]
    Confirmation(#[from] CheckoutOrderConfirmationError),
    #[error(
        "checkout order boundary failed at `{stage}` with `{code}` (retryable={retryable}): {message}"
    )]
    Boundary {
        stage: &'static str,
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("checkout order stage conflict: {0}")]
    Conflict(String),
}

pub type CheckoutOrderStageResult<T> = Result<T, CheckoutOrderStageError>;

pub struct CheckoutOrderStageExecutor {
    operation_journal: CheckoutOperationJournal,
    plan_journal: CheckoutOrderPlanJournal,
    inventory_executor: CheckoutInventoryReservationExecutor,
    order_creation: CheckoutOrderCreationExecutor,
    order_confirmation: CheckoutOrderConfirmationExecutor,
    order_service: OrderService,
    lease_seconds: i64,
}

impl CheckoutOrderStageExecutor {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        event_bus: TransactionalEventBus,
        inventory_port: Arc<dyn InventoryReservationIdentityPort>,
    ) -> Self {
        Self {
            operation_journal: CheckoutOperationJournal::new(db.clone()),
            plan_journal: CheckoutOrderPlanJournal::new(db.clone()),
            inventory_executor: CheckoutInventoryReservationExecutor::new(
                db.clone(),
                inventory_port,
            ),
            order_creation: CheckoutOrderCreationExecutor::new(db.clone(), event_bus.clone()),
            order_confirmation: CheckoutOrderConfirmationExecutor::new(
                db.clone(),
                event_bus.clone(),
            ),
            order_service: OrderService::new(db, event_bus),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
        }
    }

    pub fn with_order_identity_port(
        mut self,
        order_identity_port: Arc<dyn rustok_order::CheckoutOrderIdentityPort>,
    ) -> Self {
        self.order_creation = self
            .order_creation
            .with_order_identity_port(order_identity_port);
        self
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self.order_confirmation = self.order_confirmation.with_lease_seconds(lease_seconds);
        self
    }

    /// Advances an already claimed and cart-locked checkout operation through
    /// inventory reservation, order creation & inventory adoption, and order
    /// confirmation to `payment_ready`.
    ///
    /// Sequence strictly aligns with database lifecycle guards:
    /// `cart_locked -> inventory_reserved -> order_created (with adopted lines) -> payment_ready (confirmed order)`
    pub async fn advance_to_payment_ready(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        snapshot: &PreparedCartCheckoutSnapshot,
        initial_plan: Option<CheckoutOrderPlanPayload>,
    ) -> CheckoutOrderStageResult<CheckoutPaymentReadyState> {
        let lease_owner = lease_owner.into();
        let mut supplied_plan = initial_plan;

        for _ in 0..4 {
            let operation = self.operation_journal.get(tenant_id, operation_id).await?;
            match operation.stage.as_str() {
                stage if stage == CheckoutOperationStage::CartLocked.as_str() => {
                    if operation.snapshot_hash.as_deref() != Some(snapshot.snapshot_hash.as_str())
                        || operation.cart_id != snapshot.cart.id
                    {
                        return Err(CheckoutOrderStageError::Conflict(format!(
                            "checkout operation {} does not match the prepared cart snapshot",
                            operation.id
                        )));
                    }
                    let payload = supplied_plan.take().ok_or_else(|| {
                        CheckoutOrderStageError::Conflict(format!(
                            "checkout operation {} requires an immutable order plan before inventory reservation",
                            operation.id
                        ))
                    })?;
                    self.plan_journal
                        .persist(
                            tenant_id,
                            operation_id,
                            snapshot.snapshot_hash.clone(),
                            payload,
                        )
                        .await?;
                    self.inventory_executor
                        .reserve_and_checkpoint(
                            tenant_id,
                            PortActor::user(actor_id.to_string()),
                            operation_id,
                            lease_owner.clone(),
                            snapshot,
                        )
                        .await?;
                }
                stage if stage == CheckoutOperationStage::InventoryReserved.as_str() => {
                    let plan = self.plan_journal.get(tenant_id, operation_id).await?;
                    self.order_creation
                        .create_pending_and_adopt(
                            tenant_id,
                            actor_id,
                            operation_id,
                            lease_owner.clone(),
                            plan.payload.order_input.clone(),
                            plan.payload.channel_id,
                            plan.payload.channel_slug.clone(),
                            plan.payload.context.locale.as_str(),
                            Some(plan.payload.context.default_locale.as_str()),
                        )
                        .await?;
                }
                stage if stage == CheckoutOperationStage::OrderCreated.as_str() => {
                    let plan = self.plan_journal.get(tenant_id, operation_id).await?;
                    self.order_confirmation
                        .confirm_and_checkpoint(
                            tenant_id,
                            actor_id,
                            operation_id,
                            lease_owner.clone(),
                            plan.payload.context.locale.as_str(),
                            Some(plan.payload.context.default_locale.as_str()),
                        )
                        .await?;
                }
                stage if stage == CheckoutOperationStage::PaymentReady.as_str() => {
                    return self.load_payment_ready_state(tenant_id, operation_id).await;
                }
                stage => {
                    return Err(CheckoutOrderStageError::Conflict(format!(
                        "checkout operation {} cannot enter order stages from `{stage}`",
                        operation.id
                    )));
                }
            }
        }

        Err(CheckoutOrderStageError::Conflict(format!(
            "checkout operation {operation_id} did not reach payment_ready within the bounded stage loop"
        )))
    }

    pub async fn load_payment_ready_state(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> CheckoutOrderStageResult<CheckoutPaymentReadyState> {
        let operation = self.operation_journal.get(tenant_id, operation_id).await?;
        let plan = self.plan_journal.get(tenant_id, operation_id).await?;
        let order_id = operation.order_id.ok_or_else(|| {
            CheckoutOrderStageError::Conflict(format!(
                "checkout operation {} has no persisted order id",
                operation.id
            ))
        })?;
        let order = self
            .order_service
            .get_order_with_locale_fallback(
                tenant_id,
                order_id,
                plan.payload.context.locale.as_str(),
                Some(plan.payload.context.default_locale.as_str()),
            )
            .await
            .map_err(|err| {
                CheckoutOrderStageError::Conflict(format!(
                    "failed to load order {order_id} for checkout operation {operation_id}: {err}"
                ))
            })?;
        Ok(CheckoutPaymentReadyState {
            operation_id,
            order,
            plan,
        })
    }

    pub fn plan_journal(&self) -> &CheckoutOrderPlanJournal {
        &self.plan_journal
    }
}
