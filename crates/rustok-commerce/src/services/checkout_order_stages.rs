use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError};
use rustok_cart::PreparedCartCheckoutSnapshot;
use rustok_inventory::InventoryReservationIdentityPort;
use rustok_order::{
    CheckoutCompletionPort, CheckoutOrderRecoveryAdapter, CompleteCheckoutPortRequest,
    OrderResponse, OrderStatusKind, ReadCheckoutOrderProjectionRequest,
    RecoverExistingCheckoutOrderRequest, in_process_checkout_completion_port,
    in_process_checkout_order_recovery_adapter,
};
use rustok_outbox::TransactionalEventBus;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::Arc,
    time::Duration,
};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::checkout_operation;

use super::{
    CheckoutInventoryExecutionError, CheckoutInventoryOrderAdoptionError,
    CheckoutInventoryOrderAdoptionService, CheckoutInventoryReservationExecutor,
    CheckoutOperationCheckpoint, CheckoutOperationError, CheckoutOperationJournal,
    CheckoutOperationStage, CheckoutOrderPlanError, CheckoutOrderPlanJournal,
    CheckoutOrderPlanPayload, CheckoutOrderPlanRecord, DEFAULT_CHECKOUT_LEASE_SECONDS,
};

const ORDER_COMPLETION_PORT_DEADLINE_SECONDS: u64 = 3;

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
    Adoption(#[from] CheckoutInventoryOrderAdoptionError),
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
    inventory_adoption: CheckoutInventoryOrderAdoptionService,
    completion_port: Arc<dyn CheckoutCompletionPort>,
    recovery_adapter: CheckoutOrderRecoveryAdapter,
    lease_seconds: i64,
    port_deadline: Duration,
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
            inventory_adoption: CheckoutInventoryOrderAdoptionService::new(db.clone()),
            completion_port: in_process_checkout_completion_port(db.clone(), event_bus.clone()),
            recovery_adapter: in_process_checkout_order_recovery_adapter(db, event_bus),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
            port_deadline: Duration::from_secs(ORDER_COMPLETION_PORT_DEADLINE_SECONDS),
        }
    }

    pub fn with_completion_port(
        mut self,
        completion_port: Arc<dyn CheckoutCompletionPort>,
    ) -> Self {
        self.completion_port = completion_port;
        self
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self.inventory_adoption = self
            .inventory_adoption
            .clone()
            .with_lease_seconds(lease_seconds);
        self
    }

    /// Advances a claimed checkout operation to `payment_ready`.
    ///
    /// `initial_plan` is required only while the operation is `cart_locked`.
    /// Once persisted, every later stage reloads the immutable plan. Order
    /// creation and confirmation are one owner command through
    /// `CheckoutCompletionPort`; commerce only adopts its reserved inventory
    /// rows and checkpoints the durable orchestration stages.
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
                    let request = completion_request(&operation, &plan)?;
                    let legacy_snapshot_hash =
                        operation.snapshot_hash.clone().ok_or_else(|| {
                            CheckoutOrderStageError::Conflict(format!(
                                "checkout operation {} has no immutable cart snapshot hash",
                                operation.id
                            ))
                        })?;
                    let legacy_request_hash = legacy_order_request_hash(
                        &plan,
                        operation_id,
                        legacy_snapshot_hash.as_str(),
                    )?;
                    let write_context = completion_context(
                        tenant_id,
                        PortActor::user(actor_id.to_string()),
                        operation_id,
                        plan.payload.context.locale.as_str(),
                        self.port_deadline,
                        "complete",
                        true,
                    );

                    let order = match self
                        .recovery_adapter
                        .recover_existing_checkout(
                            write_context.clone(),
                            RecoverExistingCheckoutOrderRequest {
                                checkout_operation_id: operation_id,
                                completion: request.clone(),
                                legacy_snapshot_hash,
                                legacy_request_hash,
                            },
                        )
                        .await
                        .map_err(|error| boundary_error("recover_existing", error))?
                    {
                        Some(order) => order,
                        None => {
                            let completion = self
                                .completion_port
                                .complete_checkout(write_context, request)
                                .await
                                .map_err(|error| boundary_error("complete", error))?;
                            let order = self
                                .read_order_projection(tenant_id, operation_id, &plan)
                                .await?;
                            if completion.order_id != order.id {
                                return Err(CheckoutOrderStageError::Conflict(format!(
                                    "checkout operation {operation_id} completed order {} but resolved order {}",
                                    completion.order_id, order.id
                                )));
                            }
                            order
                        }
                    };
                    validate_order_projection(&operation, &order, &[OrderStatusKind::Confirmed])?;
                    self.inventory_adoption
                        .adopt_and_checkpoint(tenant_id, operation_id, lease_owner.clone(), &order)
                        .await?;
                }
                stage if stage == CheckoutOperationStage::OrderCreated.as_str() => {
                    let plan = self.plan_journal.get(tenant_id, operation_id).await?;
                    let order = self
                        .read_order_projection(tenant_id, operation_id, &plan)
                        .await?;
                    validate_order_projection(&operation, &order, &[OrderStatusKind::Confirmed])?;
                    self.operation_journal
                        .checkpoint(CheckoutOperationCheckpoint {
                            tenant_id,
                            operation_id,
                            lease_owner: lease_owner.clone(),
                            expected_stage: CheckoutOperationStage::OrderCreated,
                            next_stage: CheckoutOperationStage::PaymentReady,
                            snapshot_hash: None,
                            order_id: Some(order.id),
                            payment_collection_id: operation.payment_collection_id,
                            lease_seconds: self.lease_seconds,
                        })
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
        if !matches!(
            operation.stage.as_str(),
            "payment_ready"
                | "payment_authorized"
                | "payment_captured"
                | "fulfillment_created"
                | "cart_completed"
                | "completed"
        ) {
            return Err(CheckoutOrderStageError::Conflict(format!(
                "checkout operation {} has not reached payment_ready, stage={}",
                operation.id, operation.stage
            )));
        }
        let plan = self.plan_journal.get(tenant_id, operation_id).await?;
        let order = self
            .read_order_projection(tenant_id, operation_id, &plan)
            .await?;
        validate_order_projection(
            &operation,
            &order,
            &[
                OrderStatusKind::Confirmed,
                OrderStatusKind::Paid,
                OrderStatusKind::Shipped,
                OrderStatusKind::Delivered,
            ],
        )?;
        Ok(CheckoutPaymentReadyState {
            operation_id,
            order,
            plan,
        })
    }

    async fn read_order_projection(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        plan: &CheckoutOrderPlanRecord,
    ) -> CheckoutOrderStageResult<OrderResponse> {
        self.recovery_adapter
            .read_checkout_order(
                completion_context(
                    tenant_id,
                    PortActor::service("rustok-commerce.checkout-order-stage"),
                    operation_id,
                    plan.payload.context.locale.as_str(),
                    self.port_deadline,
                    "read-order",
                    false,
                ),
                ReadCheckoutOrderProjectionRequest {
                    checkout_operation_id: operation_id,
                    locale: Some(plan.payload.context.locale.clone()),
                    fallback_locale: Some(plan.payload.context.default_locale.clone()),
                },
            )
            .await
            .map_err(|error| boundary_error("read_order", error))
    }

    pub fn plan_journal(&self) -> &CheckoutOrderPlanJournal {
        &self.plan_journal
    }
}

fn completion_request(
    operation: &checkout_operation::Model,
    plan: &CheckoutOrderPlanRecord,
) -> CheckoutOrderStageResult<CompleteCheckoutPortRequest> {
    validate_line_item_provenance(&plan.payload.order_input)?;
    let input = &plan.payload.order_input;
    Ok(CompleteCheckoutPortRequest {
        cart_id: operation.cart_id,
        customer_id: input.customer_id,
        payment_collection_id: operation.payment_collection_id,
        shipping_option_id: unique_shipping_option_id(&plan.payload),
        channel_id: plan.payload.channel_id,
        channel_slug: plan.payload.channel_slug.clone(),
        locale: Some(plan.payload.context.locale.clone()),
        fallback_locale: Some(plan.payload.context.default_locale.clone()),
        currency_code: input.currency_code.clone(),
        shipping_total: input.shipping_total,
        line_items: input.line_items.clone(),
        adjustments: input.adjustments.clone(),
        tax_lines: input.tax_lines.clone(),
        metadata: input.metadata.clone(),
    })
}

fn unique_shipping_option_id(payload: &CheckoutOrderPlanPayload) -> Option<Uuid> {
    let ids = payload
        .fulfillment_plans
        .iter()
        .filter_map(|plan| plan.shipping_option_id)
        .collect::<BTreeSet<_>>();
    (ids.len() == 1).then(|| *ids.first().expect("one shipping option exists"))
}

fn legacy_order_request_hash(
    plan: &CheckoutOrderPlanRecord,
    operation_id: Uuid,
    snapshot_hash: &str,
) -> CheckoutOrderStageResult<String> {
    let mut input = plan.payload.order_input.clone();
    let root = input.metadata.as_object_mut().ok_or_else(|| {
        CheckoutOrderStageError::Conflict("order metadata must be a JSON object".to_string())
    })?;
    let checkout = root
        .entry("checkout".to_string())
        .or_insert_with(|| Value::Object(Default::default()))
        .as_object_mut()
        .ok_or_else(|| {
            CheckoutOrderStageError::Conflict(
                "order metadata.checkout must be a JSON object".to_string(),
            )
        })?;
    checkout.insert(
        "operation_id".to_string(),
        Value::String(operation_id.to_string()),
    );
    checkout.insert(
        "snapshot_hash".to_string(),
        Value::String(snapshot_hash.to_string()),
    );
    let value = serde_json::to_value((
        &input,
        plan.payload.channel_id,
        plan.payload.channel_slug.as_deref(),
    ))
    .map_err(|error| {
        CheckoutOrderStageError::Conflict(format!(
            "failed to serialize legacy order creation request: {error}"
        ))
    })?;
    let canonical = canonicalize_json(value);
    let payload = serde_json::to_vec(&canonical).map_err(|error| {
        CheckoutOrderStageError::Conflict(format!(
            "failed to encode legacy order creation request: {error}"
        ))
    })?;
    Ok(hex::encode(Sha256::digest(payload)))
}

fn validate_line_item_provenance(
    input: &rustok_order::CreateOrderInput,
) -> CheckoutOrderStageResult<()> {
    let mut seen = HashSet::new();
    for (index, line) in input.line_items.iter().enumerate() {
        if line.variant_id.is_none() {
            continue;
        }
        let cart_line_item_id = line
            .metadata
            .get("checkout")
            .and_then(|checkout| checkout.get("cart_line_item_id"))
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| {
                CheckoutOrderStageError::Conflict(format!(
                    "variant-backed order line input {index} has no valid cart-line provenance"
                ))
            })?;
        if !seen.insert(cart_line_item_id) {
            return Err(CheckoutOrderStageError::Conflict(format!(
                "multiple order line inputs reference cart line {cart_line_item_id}"
            )));
        }
    }
    Ok(())
}

fn validate_order_projection(
    operation: &checkout_operation::Model,
    order: &OrderResponse,
    allowed_statuses: &[OrderStatusKind],
) -> CheckoutOrderStageResult<()> {
    if order.tenant_id != operation.tenant_id
        || operation.order_id.is_some() && operation.order_id != Some(order.id)
    {
        return Err(CheckoutOrderStageError::Conflict(format!(
            "checkout operation {} resolved a conflicting order projection",
            operation.id
        )));
    }
    if !allowed_statuses.contains(&order.status_kind()) {
        return Err(CheckoutOrderStageError::Conflict(format!(
            "checkout operation {} resolved order {} in an unsupported lifecycle state",
            operation.id, order.id
        )));
    }
    Ok(())
}

fn completion_context(
    tenant_id: Uuid,
    actor: PortActor,
    operation_id: Uuid,
    locale: &str,
    deadline: Duration,
    action: &str,
    write: bool,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        actor,
        if locale.trim().is_empty() {
            PLATFORM_FALLBACK_LOCALE
        } else {
            locale
        },
        format!("checkout:{operation_id}:order:{action}"),
    )
    .with_causation_id(operation_id.to_string())
    .with_deadline(deadline);
    if write {
        context.with_idempotency_key(format!("checkout:{operation_id}:order:complete"))
    } else {
        context
    }
}

fn boundary_error(stage: &'static str, error: PortError) -> CheckoutOrderStageError {
    CheckoutOrderStageError::Boundary {
        stage,
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
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
