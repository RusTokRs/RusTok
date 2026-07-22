use rustok_cart::{CartCheckoutPort, PreparedCartCheckoutSnapshot};
use rustok_fulfillment::{FulfillmentError, FulfillmentResponse, FulfillmentService};
use rustok_inventory::InventoryReservationIdentityPort;
use rustok_marketplace_allocation::MarketplaceAllocationCommandPort;
use rustok_marketplace_commission::MarketplaceCommissionCommandPort;
use rustok_marketplace_ledger::MarketplaceLedgerCommandPort;
use rustok_order::OrderResponse;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::error::PaymentError;
use rustok_payment::providers::PaymentProviderRegistry;
use rustok_payment::{PaymentCollectionResponse, PaymentService};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
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
    CheckoutOrderPlanJournal, CheckoutOrderPlanPayload, CheckoutOrderPlanRecord,
    CheckoutOrderStageError, CheckoutOrderStageExecutor, CheckoutPaymentCapturedState,
    CheckoutPaymentReadyState, CheckoutPaymentStageError, CheckoutPaymentStageExecutor,
    RecordCheckoutMarketplaceEconomicsCheckpoint, build_marketplace_economics_evidence,
    validate_marketplace_economics_checkpoint,
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
    #[error(transparent)]
    Payment(#[from] PaymentError),
    #[error(transparent)]
    Fulfillment(#[from] FulfillmentError),
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
    plan_journal: CheckoutOrderPlanJournal,
    order_stage: CheckoutOrderStageExecutor,
    marketplace_allocation_stage: Option<CheckoutMarketplaceAllocationStage>,
    marketplace_commission_stage: Option<CheckoutMarketplaceCommissionStage>,
    marketplace_economics_checkpoint_journal: CheckoutMarketplaceEconomicsCheckpointJournal,
    marketplace_financial_stage: Option<CheckoutMarketplaceFinancialStage>,
    payment_stage: CheckoutPaymentStageExecutor,
    fulfillment_stage: CheckoutFulfillmentStageExecutor,
    finalization: CheckoutFinalizationExecutor,
    payment_service: PaymentService,
    fulfillment_service: FulfillmentService,
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
            plan_journal: CheckoutOrderPlanJournal::new(db.clone()),
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
            finalization: CheckoutFinalizationExecutor::new(db.clone(), cart_checkout_port),
            payment_service: PaymentService::new(db.clone()),
            fulfillment_service: FulfillmentService::new(db),
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
    /// all durable stages to `completed`.
    ///
    /// `initial_plan` is consumed only at `cart_locked`. Every replay from a
    /// later stage reloads the immutable plan and owner aggregates instead of
    /// rebuilding them from current store configuration.
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
                self.load_fulfillment_created_state(tenant_id, operation_id)
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
        let operation = self.operation_journal.get(tenant_id, operation_id).await?;
        let collection_id = operation.payment_collection_id.ok_or_else(|| {
            CheckoutStagePipelineError::Conflict(format!(
                "checkout operation {} has no persisted payment collection id",
                operation.id
            ))
        })?;
        let collection = self
            .payment_service
            .get_collection(tenant_id, collection_id)
            .await?;
        validate_captured_collection(&ready.order, &collection, operation_id)?;
        Ok(CheckoutPaymentCapturedState {
            operation_id,
            order: ready.order,
            plan: ready.plan,
            payment_collection: collection,
        })
    }

    async fn load_fulfillment_created_state(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> CheckoutStagePipelineResult<CheckoutFulfillmentCreatedState> {
        let captured = self
            .load_payment_captured_state(tenant_id, operation_id)
            .await?;
        let operation_id_text = operation_id.to_string();
        let fulfillments = self
            .fulfillment_service
            .list_by_order(tenant_id, captured.order.id)
            .await?
            .into_iter()
            .filter(|fulfillment| {
                fulfillment
                    .metadata
                    .get("checkout")
                    .and_then(|checkout| checkout.get("operation_id"))
                    .and_then(Value::as_str)
                    == Some(operation_id_text.as_str())
            })
            .collect::<Vec<_>>();
        validate_loaded_fulfillments(&captured.plan, &fulfillments)?;
        Ok(CheckoutFulfillmentCreatedState {
            operation_id,
            order: captured.order,
            plan: captured.plan,
            payment_collection: captured.payment_collection,
            fulfillments,
        })
    }
}

fn validate_captured_collection(
    order: &OrderResponse,
    collection: &PaymentCollectionResponse,
    operation_id: Uuid,
) -> CheckoutStagePipelineResult<()> {
    let source_operation = collection
        .metadata
        .get("checkout")
        .and_then(|checkout| checkout.get("operation_id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    if source_operation != Some(operation_id)
        || collection.order_id != Some(order.id)
        || collection.status != "captured"
        || collection.captured_amount != order.total_amount
    {
        return Err(CheckoutStagePipelineError::Conflict(format!(
            "payment collection {} is not the captured result of checkout operation {}",
            collection.id, operation_id
        )));
    }
    Ok(())
}

fn validate_loaded_fulfillments(
    plan: &CheckoutOrderPlanRecord,
    fulfillments: &[FulfillmentResponse],
) -> CheckoutStagePipelineResult<()> {
    let expected = plan
        .payload
        .fulfillment_plans
        .iter()
        .enumerate()
        .map(|(index, _)| {
            (
                format!(
                    "checkout:{}:fulfillment:{index}",
                    plan.checkout_operation_id
                ),
                index,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut actual = BTreeMap::new();
    for fulfillment in fulfillments {
        let checkout = fulfillment
            .metadata
            .get("checkout")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                CheckoutStagePipelineError::Conflict(format!(
                    "fulfillment {} has no checkout identity",
                    fulfillment.id
                ))
            })?;
        let key = checkout
            .get("fulfillment_key")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CheckoutStagePipelineError::Conflict(format!(
                    "fulfillment {} has no fulfillment key",
                    fulfillment.id
                ))
            })?
            .to_string();
        let index = checkout
            .get("fulfillment_index")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                CheckoutStagePipelineError::Conflict(format!(
                    "fulfillment {} has no fulfillment index",
                    fulfillment.id
                ))
            })? as usize;
        if actual.insert(key, index).is_some() {
            return Err(CheckoutStagePipelineError::Conflict(
                "duplicate checkout fulfillment identity".to_string(),
            ));
        }
    }
    if expected != actual {
        return Err(CheckoutStagePipelineError::Conflict(format!(
            "checkout operation {} has an incomplete fulfillment set",
            plan.checkout_operation_id
        )));
    }
    Ok(())
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
