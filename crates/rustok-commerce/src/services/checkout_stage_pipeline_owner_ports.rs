use std::sync::Arc;

use rustok_cart::{CartCheckoutPort, PreparedCartCheckoutSnapshot};
use rustok_inventory::InventoryReservationIdentityPort;
use rustok_marketplace_allocation::MarketplaceAllocationCommandPort;
use rustok_marketplace_commission::MarketplaceCommissionCommandPort;
use rustok_marketplace_ledger::MarketplaceLedgerCommandPort;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::providers::PaymentProviderRegistry;
use thiserror::Error;
use uuid::Uuid;

use super::{
    CheckoutCompletedState, CheckoutFinalizationError, CheckoutFinalizationExecutor,
    CheckoutFulfillmentCreatedState, CheckoutFulfillmentStageError,
    CheckoutFulfillmentStageExecutor, CheckoutMarketplaceAllocationError,
    CheckoutMarketplaceAllocationStage, CheckoutMarketplaceCommissionError,
    CheckoutMarketplaceCommissionStage, CheckoutMarketplaceEconomicsCheckpointError,
    CheckoutMarketplaceEconomicsCheckpointJournal, CheckoutMarketplaceFinancialError,
    CheckoutMarketplaceFinancialStage, CheckoutOperationError, CheckoutOperationJournal,
    CheckoutOperationStage, CheckoutOperationStatus, CheckoutOrderPlanError,
    CheckoutOrderPlanPayload, CheckoutOrderStageError, CheckoutOrderStageExecutor,
    CheckoutPaymentCapturedState, CheckoutPaymentReadyState, CheckoutPaymentStageError,
    CheckoutPaymentStageExecutor, RecordCheckoutMarketplaceEconomicsCheckpoint,
    build_marketplace_economics_evidence, validate_marketplace_economics_checkpoint,
};

#[derive(Debug, Error)]
pub enum CheckoutStagePipelineError {
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    Plan(#[from] CheckoutOrderPlanError),
    #[error(transparent)]
    OrderStage(Box<CheckoutOrderStageError>),
    #[error(transparent)]
    MarketplaceAllocation(#[from] CheckoutMarketplaceAllocationError),
    #[error(transparent)]
    MarketplaceCommission(#[from] CheckoutMarketplaceCommissionError),
    #[error(transparent)]
    MarketplaceEconomicsCheckpoint(#[from] CheckoutMarketplaceEconomicsCheckpointError),
    #[error(transparent)]
    MarketplaceFinancial(#[from] CheckoutMarketplaceFinancialError),
    #[error(transparent)]
    PaymentStage(#[from] CheckoutPaymentStageError),
    #[error(transparent)]
    FulfillmentStage(#[from] CheckoutFulfillmentStageError),
    #[error(transparent)]
    Finalization(#[from] CheckoutFinalizationError),
    #[error("checkout stage pipeline conflict: {0}")]
    Conflict(String),
}

pub type CheckoutStagePipelineResult<T> = Result<T, CheckoutStagePipelineError>;

impl From<CheckoutOrderStageError> for CheckoutStagePipelineError {
    fn from(error: CheckoutOrderStageError) -> Self {
        Self::OrderStage(Box::new(error))
    }
}

pub struct CheckoutStagePipeline {
    db: sea_orm::DatabaseConnection,
    operation_journal: CheckoutOperationJournal,
    order_stage: CheckoutOrderStageExecutor,
    marketplace_allocation_stage: Option<CheckoutMarketplaceAllocationStage>,
    marketplace_commission_stage: Option<CheckoutMarketplaceCommissionStage>,
    marketplace_economics_checkpoint_journal: CheckoutMarketplaceEconomicsCheckpointJournal,
    marketplace_financial_stage: Option<CheckoutMarketplaceFinancialStage>,
    payment_stage: CheckoutPaymentStageExecutor,
    fulfillment_stage: CheckoutFulfillmentStageExecutor,
    finalization: CheckoutFinalizationExecutor,
}

impl CheckoutStagePipeline {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        event_bus: TransactionalEventBus,
        inventory_port: Arc<dyn InventoryReservationIdentityPort>,
        cart_checkout_port: Arc<dyn CartCheckoutPort>,
    ) -> Self {
        Self {
            db: db.clone(),
            operation_journal: CheckoutOperationJournal::new(db.clone()),
            order_stage: CheckoutOrderStageExecutor::new(
                db.clone(),
                event_bus.clone(),
                inventory_port,
            ),
            marketplace_allocation_stage: None,
            marketplace_commission_stage: None,
            marketplace_economics_checkpoint_journal:
                CheckoutMarketplaceEconomicsCheckpointJournal::new(db.clone()),
            marketplace_financial_stage: None,
            payment_stage: CheckoutPaymentStageExecutor::new(db.clone()),
            fulfillment_stage: CheckoutFulfillmentStageExecutor::new(db.clone(), event_bus),
            finalization: CheckoutFinalizationExecutor::new(db, cart_checkout_port),
        }
    }

    pub fn with_marketplace_allocation_port(
        mut self,
        marketplace_allocation_port: Arc<dyn MarketplaceAllocationCommandPort>,
    ) -> Self {
        self.marketplace_allocation_stage = Some(CheckoutMarketplaceAllocationStage::new(
            marketplace_allocation_port,
        ));
        self
    }

    pub fn with_marketplace_commission_port(
        mut self,
        marketplace_commission_port: Arc<dyn MarketplaceCommissionCommandPort>,
    ) -> Self {
        self.marketplace_commission_stage = Some(CheckoutMarketplaceCommissionStage::new(
            marketplace_commission_port,
        ));
        self
    }

    pub fn with_marketplace_ledger_port(
        mut self,
        marketplace_ledger_port: Arc<dyn MarketplaceLedgerCommandPort>,
    ) -> Self {
        self.marketplace_financial_stage = Some(CheckoutMarketplaceFinancialStage::new(
            self.db.clone(),
            marketplace_ledger_port,
        ));
        self
    }

    pub fn with_payment_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.payment_stage = self
            .payment_stage
            .with_provider_registry(payment_provider_registry);
        self
    }

    /// Advances an already claimed and cart-locked checkout operation through
    /// all durable stages to `completed` using owner ports for every staged
    /// order, payment, and fulfillment read/write.
    pub async fn advance_to_completed(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        snapshot: &PreparedCartCheckoutSnapshot,
        initial_plan: Option<CheckoutOrderPlanPayload>,
    ) -> CheckoutStagePipelineResult<CheckoutCompletedState> {
        let lease_owner = lease_owner.into();
        let payment_ready_rank = stage_rank(CheckoutOperationStage::PaymentReady.as_str())?;
        let payment_captured_rank = stage_rank(CheckoutOperationStage::PaymentCaptured.as_str())?;
        let fulfillment_created_rank =
            stage_rank(CheckoutOperationStage::FulfillmentCreated.as_str())?;

        let operation = self.operation_journal.get(tenant_id, operation_id).await?;
        let payment_ready = if stage_rank(operation.stage.as_str())? <= payment_ready_rank {
            self.order_stage
                .advance_to_payment_ready(
                    tenant_id,
                    actor_id,
                    operation_id,
                    lease_owner.clone(),
                    snapshot,
                    initial_plan,
                )
                .await?
        } else {
            self.load_payment_ready_state(tenant_id, operation_id)
                .await?
        };

        self.ensure_marketplace_economics_before_capture(
            tenant_id,
            actor_id,
            operation_id,
            lease_owner.as_str(),
            &payment_ready,
        )
        .await?;

        let operation = self.operation_journal.get(tenant_id, operation_id).await?;
        let payment_captured = if stage_rank(operation.stage.as_str())? <= payment_captured_rank {
            self.payment_stage
                .advance_to_payment_captured(
                    tenant_id,
                    operation_id,
                    lease_owner.clone(),
                    payment_ready.order,
                    payment_ready.plan,
                )
                .await?
        } else {
            self.load_payment_captured_state(tenant_id, operation_id)
                .await?
        };

        self.ensure_marketplace_financial_after_capture(
            tenant_id,
            actor_id,
            lease_owner.as_str(),
            &payment_captured,
        )
        .await?;

        let operation = self.operation_journal.get(tenant_id, operation_id).await?;
        let fulfillment_created =
            if stage_rank(operation.stage.as_str())? <= fulfillment_created_rank {
                self.fulfillment_stage
                    .advance_to_fulfillment_created(
                        tenant_id,
                        actor_id,
                        lease_owner.clone(),
                        payment_captured,
                    )
                    .await?
            } else {
                self.load_fulfillment_created_state(tenant_id, actor_id, operation_id)
                    .await?
            };

        self.finalization
            .complete(tenant_id, actor_id, lease_owner, fulfillment_created)
            .await
            .map_err(Into::into)
    }

    async fn ensure_marketplace_economics_before_capture(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
        lease_owner: &str,
        payment_ready: &CheckoutPaymentReadyState,
    ) -> CheckoutStagePipelineResult<()> {
        let marketplace_line_count = payment_ready.plan.payload.marketplace_lines.len();
        if marketplace_line_count == 0 {
            return Ok(());
        }

        if let Some(checkpoint) = self
            .marketplace_economics_checkpoint_journal
            .get(tenant_id, operation_id)
            .await?
        {
            validate_marketplace_economics_checkpoint(
                &checkpoint,
                tenant_id,
                operation_id,
                payment_ready.order.id,
                payment_ready.plan.plan_hash.as_str(),
                payment_ready.order.currency_code.as_str(),
                marketplace_line_count,
            )?;
            return Ok(());
        }

        let operation = self.operation_journal.get(tenant_id, operation_id).await?;
        if operation.status != CheckoutOperationStatus::Executing.as_str()
            || operation.stage != CheckoutOperationStage::PaymentReady.as_str()
        {
            return Err(CheckoutStagePipelineError::Conflict(format!(
                "checkout operation {operation_id} advanced beyond payment_ready without a marketplace economics checkpoint"
            )));
        }

        let allocation_stage = self.marketplace_allocation_stage.as_ref().ok_or_else(|| {
            CheckoutStagePipelineError::Conflict(
                "marketplace checkout lines require a composed allocation command port".to_string(),
            )
        })?;
        let commission_stage = self.marketplace_commission_stage.as_ref().ok_or_else(|| {
            CheckoutStagePipelineError::Conflict(
                "marketplace checkout lines require a composed commission command port".to_string(),
            )
        })?;

        let allocation = allocation_stage
            .allocate_if_present(
                tenant_id,
                actor_id,
                operation_id,
                &payment_ready.plan.payload,
                &payment_ready.order,
            )
            .await?
            .ok_or_else(|| {
                CheckoutStagePipelineError::Conflict(
                    "marketplace checkout plan did not produce an allocation result".to_string(),
                )
            })?;
        let commission = commission_stage
            .assess_if_present(
                tenant_id,
                actor_id,
                operation_id,
                &payment_ready.plan.payload,
                &payment_ready.order,
            )
            .await?
            .ok_or_else(|| {
                CheckoutStagePipelineError::Conflict(
                    "marketplace checkout plan did not produce a commission result".to_string(),
                )
            })?;
        let evidence = build_marketplace_economics_evidence(
            payment_ready.plan.plan_hash.as_str(),
            &allocation,
            &commission,
        )?;
        let checkpoint = self
            .marketplace_economics_checkpoint_journal
            .record(RecordCheckoutMarketplaceEconomicsCheckpoint {
                tenant_id,
                checkout_operation_id: operation_id,
                lease_owner: lease_owner.to_string(),
                evidence,
            })
            .await?;
        validate_marketplace_economics_checkpoint(
            &checkpoint,
            tenant_id,
            operation_id,
            payment_ready.order.id,
            payment_ready.plan.plan_hash.as_str(),
            payment_ready.order.currency_code.as_str(),
            marketplace_line_count,
        )?;
        Ok(())
    }

    async fn ensure_marketplace_financial_after_capture(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        lease_owner: &str,
        payment_captured: &CheckoutPaymentCapturedState,
    ) -> CheckoutStagePipelineResult<()> {
        if payment_captured.plan.payload.marketplace_lines.is_empty() {
            return Ok(());
        }
        let stage = self.marketplace_financial_stage.as_ref().ok_or_else(|| {
            CheckoutStagePipelineError::Conflict(
                "marketplace checkout lines require a composed ledger command port".to_string(),
            )
        })?;
        stage
            .post_after_capture_if_present(tenant_id, actor_id, lease_owner, payment_captured)
            .await?;
        Ok(())
    }

    async fn load_payment_ready_state(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> CheckoutStagePipelineResult<CheckoutPaymentReadyState> {
        self.order_stage
            .load_payment_ready_state(tenant_id, operation_id)
            .await
            .map_err(Into::into)
    }

    async fn load_payment_captured_state(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> CheckoutStagePipelineResult<CheckoutPaymentCapturedState> {
        let ready = self
            .load_payment_ready_state(tenant_id, operation_id)
            .await?;
        self.payment_stage
            .load_payment_captured_state(tenant_id, operation_id, ready.order, ready.plan)
            .await
            .map_err(Into::into)
    }

    async fn load_fulfillment_created_state(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
    ) -> CheckoutStagePipelineResult<CheckoutFulfillmentCreatedState> {
        let captured = self
            .load_payment_captured_state(tenant_id, operation_id)
            .await?;
        self.fulfillment_stage
            .load_fulfillment_created_state(tenant_id, actor_id, captured)
            .await
            .map_err(Into::into)
    }
}

fn stage_rank(stage: &str) -> CheckoutStagePipelineResult<u8> {
    match stage {
        "cart_locked" => Ok(1),
        "inventory_reserved" => Ok(2),
        "order_created" => Ok(3),
        "payment_ready" => Ok(4),
        "payment_authorized" => Ok(5),
        "payment_captured" => Ok(6),
        "fulfillment_created" => Ok(7),
        "cart_completed" => Ok(8),
        "completed" => Ok(9),
        other => Err(CheckoutStagePipelineError::Conflict(format!(
            "checkout stage pipeline cannot resume from `{other}`"
        ))),
    }
}
