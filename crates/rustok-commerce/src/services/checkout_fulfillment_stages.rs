use rustok_fulfillment::{
    CreateFulfillmentInput, CreateFulfillmentItemInput, FulfillmentError, FulfillmentResponse,
    FulfillmentService,
};
use rustok_order::{OrderError, OrderLineItemResponse, OrderResponse, OrderService};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::PaymentCollectionResponse;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

use super::{
    CheckoutFulfillmentPlan, CheckoutOperationCheckpoint, CheckoutOperationError,
    CheckoutOperationJournal, CheckoutOperationStage, CheckoutOperationStatus,
    CheckoutOrderPlanRecord, CheckoutPaymentCapturedState, DEFAULT_CHECKOUT_LEASE_SECONDS,
};

const MANUAL_PROVIDER_ID: &str = "manual";

#[derive(Clone, Debug)]
pub struct CheckoutFulfillmentCreatedState {
    pub operation_id: Uuid,
    pub order: OrderResponse,
    pub plan: CheckoutOrderPlanRecord,
    pub payment_collection: PaymentCollectionResponse,
    pub fulfillments: Vec<FulfillmentResponse>,
}

#[derive(Debug, Error)]
pub enum CheckoutFulfillmentStageError {
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    Fulfillment(#[from] FulfillmentError),
    #[error(transparent)]
    Order(#[from] OrderError),
    #[error("checkout fulfillment stage conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type CheckoutFulfillmentStageResult<T> = Result<T, CheckoutFulfillmentStageError>;

pub struct CheckoutFulfillmentStageExecutor {
    db: DatabaseConnection,
    fulfillment_service: FulfillmentService,
    order_service: OrderService,
    operation_journal: CheckoutOperationJournal,
    lease_seconds: i64,
}

impl CheckoutFulfillmentStageExecutor {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            fulfillment_service: FulfillmentService::new(db.clone()),
            order_service: OrderService::new(db.clone(), event_bus),
            operation_journal: CheckoutOperationJournal::new(db.clone()),
            db,
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
        }
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    /// Creates or adopts every fulfillment from the immutable order plan,
    /// marks the captured order paid, and checkpoints
    /// `payment_captured -> fulfillment_created`.
    ///
    /// Fulfillment rows are created before `mark_paid`, so the paid-order event
    /// listener always observes the complete fulfillment set. A crash after
    /// either side effect is replayed through owner-owned immutable identities.
    pub async fn advance_to_fulfillment_created(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        lease_owner: impl Into<String>,
        state: CheckoutPaymentCapturedState,
    ) -> CheckoutFulfillmentStageResult<CheckoutFulfillmentCreatedState> {
        let lease_owner = lease_owner.into();
        validate_captured_state(tenant_id, &state)?;

        for _ in 0..2 {
            let operation = self
                .operation_journal
                .get(tenant_id, state.operation_id)
                .await?;
            if operation.status != CheckoutOperationStatus::Executing.as_str() {
                return Err(CheckoutFulfillmentStageError::Conflict(format!(
                    "checkout operation {} must be executing, not `{}`",
                    operation.id, operation.status
                )));
            }
            if operation.order_id != Some(state.order.id)
                || operation.payment_collection_id != Some(state.payment_collection.id)
            {
                return Err(CheckoutFulfillmentStageError::Conflict(format!(
                    "checkout operation {} does not match the captured order and collection",
                    operation.id
                )));
            }

            match operation.stage.as_str() {
                stage if stage == CheckoutOperationStage::PaymentCaptured.as_str() => {
                    let fulfillments = self
                        .ensure_fulfillments(tenant_id, &state.order, &state.plan)
                        .await?;
                    let paid_order = self
                        .ensure_paid_order(
                            tenant_id,
                            actor_id,
                            &state.order,
                            &state.payment_collection,
                            &state.plan,
                        )
                        .await?;
                    self.operation_journal
                        .checkpoint(CheckoutOperationCheckpoint {
                            tenant_id,
                            operation_id: operation.id,
                            lease_owner: lease_owner.clone(),
                            expected_stage: CheckoutOperationStage::PaymentCaptured,
                            next_stage: CheckoutOperationStage::FulfillmentCreated,
                            snapshot_hash: None,
                            order_id: Some(paid_order.id),
                            payment_collection_id: Some(state.payment_collection.id),
                            lease_seconds: self.lease_seconds,
                        })
                        .await?;
                    let _ = fulfillments;
                }
                stage if stage == CheckoutOperationStage::FulfillmentCreated.as_str() => {
                    let fulfillments = self
                        .ensure_fulfillments(tenant_id, &state.order, &state.plan)
                        .await?;
                    let paid_order = self
                        .ensure_paid_order(
                            tenant_id,
                            actor_id,
                            &state.order,
                            &state.payment_collection,
                            &state.plan,
                        )
                        .await?;
                    return Ok(CheckoutFulfillmentCreatedState {
                        operation_id: operation.id,
                        order: paid_order,
                        plan: state.plan,
                        payment_collection: state.payment_collection,
                        fulfillments,
                    });
                }
                stage => {
                    return Err(CheckoutFulfillmentStageError::Conflict(format!(
                        "checkout operation {} cannot enter fulfillment stages from `{stage}`",
                        operation.id
                    )));
                }
            }
        }

        Err(CheckoutFulfillmentStageError::Conflict(format!(
            "checkout operation {} did not reach fulfillment_created within the bounded stage loop",
            state.operation_id
        )))
    }

    async fn ensure_fulfillments(
        &self,
        tenant_id: Uuid,
        order: &OrderResponse,
        plan: &CheckoutOrderPlanRecord,
    ) -> CheckoutFulfillmentStageResult<Vec<FulfillmentResponse>> {
        if !plan.payload.create_fulfillment {
            if !plan.payload.fulfillment_plans.is_empty() {
                return Err(CheckoutFulfillmentStageError::Conflict(
                    "immutable order plan contains disabled fulfillment work".to_string(),
                ));
            }
            return Ok(Vec::new());
        }

        let order_lines = order_lines_by_cart_identity(order)?;
        let mut fulfillments = Vec::with_capacity(plan.payload.fulfillment_plans.len());
        for (index, fulfillment_plan) in plan.payload.fulfillment_plans.iter().enumerate() {
            let fulfillment_key = fulfillment_key(plan.checkout_operation_id, index);
            let input = build_fulfillment_input(
                order,
                fulfillment_plan,
                &order_lines,
                plan,
                index,
                fulfillment_key.as_str(),
            )?;
            let existing_id =
                find_fulfillment_id_by_key(&self.db, tenant_id, fulfillment_key.as_str()).await?;
            let fulfillment = match existing_id {
                Some(fulfillment_id) => {
                    self.fulfillment_service
                        .get_fulfillment(tenant_id, fulfillment_id)
                        .await?
                }
                None => match self
                    .fulfillment_service
                    .create_fulfillment(tenant_id, input.clone())
                    .await
                {
                    Ok(fulfillment) => fulfillment,
                    Err(error) => {
                        let Some(fulfillment_id) = find_fulfillment_id_by_key(
                            &self.db,
                            tenant_id,
                            fulfillment_key.as_str(),
                        )
                        .await?
                        else {
                            return Err(error.into());
                        };
                        self.fulfillment_service
                            .get_fulfillment(tenant_id, fulfillment_id)
                            .await?
                    }
                },
            };
            validate_fulfillment(
                &fulfillment,
                tenant_id,
                order,
                &input,
                plan,
                index,
                fulfillment_key.as_str(),
            )?;
            fulfillments.push(fulfillment);
        }
        Ok(fulfillments)
    }

    async fn ensure_paid_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        expected_order: &OrderResponse,
        payment_collection: &PaymentCollectionResponse,
        plan: &CheckoutOrderPlanRecord,
    ) -> CheckoutFulfillmentStageResult<OrderResponse> {
        let payment_reference = payment_reference(payment_collection, expected_order.id);
        let payment_method = payment_collection
            .provider_id
            .clone()
            .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());
        let current = self
            .order_service
            .get_order_with_locale_fallback(
                tenant_id,
                expected_order.id,
                plan.payload.context.locale.as_str(),
                Some(plan.payload.context.default_locale.as_str()),
            )
            .await?;
        let paid = match current.status.as_str() {
            "confirmed" => {
                self.order_service
                    .mark_paid(
                        tenant_id,
                        actor_id,
                        current.id,
                        payment_reference.clone(),
                        payment_method.clone(),
                    )
                    .await?
            }
            "paid" | "shipped" | "delivered" => current,
            status => {
                return Err(CheckoutFulfillmentStageError::Conflict(format!(
                    "order {} cannot be adopted into fulfillment_created from status `{status}`",
                    current.id
                )));
            }
        };
        if paid.payment_id.as_deref() != Some(payment_reference.as_str())
            || paid.payment_method.as_deref() != Some(payment_method.as_str())
        {
            return Err(CheckoutFulfillmentStageError::Conflict(format!(
                "order {} was paid by another payment identity",
                paid.id
            )));
        }
        Ok(paid)
    }
}

async fn find_fulfillment_id_by_key<C>(
    conn: &C,
    tenant_id: Uuid,
    fulfillment_key: &str,
) -> Result<Option<Uuid>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let sql = match conn.get_database_backend() {
        DbBackend::Postgres => {
            "SELECT id FROM fulfillments WHERE tenant_id = ? AND metadata #>> '{checkout,fulfillment_key}' = ? LIMIT 2"
        }
        DbBackend::Sqlite => {
            "SELECT id FROM fulfillments WHERE tenant_id = ? AND json_extract(metadata, '$.checkout.fulfillment_key') = ? LIMIT 2"
        }
        DbBackend::MySql => {
            "SELECT id FROM fulfillments WHERE tenant_id = ? AND JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_key')) = ? LIMIT 2"
        }
    };
    let rows = conn
        .query_all(Statement::from_sql_and_values(
            conn.get_database_backend(),
            sql,
            vec![tenant_id.into(), fulfillment_key.into()],
        ))
        .await?;
    if rows.len() > 1 {
        return Err(sea_orm::DbErr::Custom(format!(
            "multiple fulfillments are bound to checkout identity {fulfillment_key}"
        )));
    }
    rows.into_iter()
        .next()
        .map(|row| row.try_get("", "id"))
        .transpose()
}

fn order_lines_by_cart_identity(
    order: &OrderResponse,
) -> CheckoutFulfillmentStageResult<HashMap<Uuid, &OrderLineItemResponse>> {
    let mut result = HashMap::new();
    for line in &order.line_items {
        let Some(cart_line_item_id) = line
            .metadata
            .get("checkout")
            .and_then(|checkout| checkout.get("cart_line_item_id"))
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
        else {
            continue;
        };
        if result.insert(cart_line_item_id, line).is_some() {
            return Err(CheckoutFulfillmentStageError::Conflict(format!(
                "order {} contains duplicate cart-line provenance {}",
                order.id, cart_line_item_id
            )));
        }
    }
    Ok(result)
}

fn build_fulfillment_input(
    order: &OrderResponse,
    fulfillment_plan: &CheckoutFulfillmentPlan,
    order_lines: &HashMap<Uuid, &OrderLineItemResponse>,
    plan: &CheckoutOrderPlanRecord,
    index: usize,
    fulfillment_key: &str,
) -> CheckoutFulfillmentStageResult<CreateFulfillmentInput> {
    let mut items = Vec::with_capacity(fulfillment_plan.items.len());
    let mut seen_order_lines = HashSet::new();
    for item in &fulfillment_plan.items {
        let order_line = order_lines.get(&item.cart_line_item_id).ok_or_else(|| {
            CheckoutFulfillmentStageError::Conflict(format!(
                "fulfillment plan {index} references missing cart line {}",
                item.cart_line_item_id
            ))
        })?;
        if item.quantity != order_line.quantity || !seen_order_lines.insert(order_line.id) {
            return Err(CheckoutFulfillmentStageError::Conflict(format!(
                "fulfillment plan {index} does not exactly cover order line {}",
                order_line.id
            )));
        }
        items.push(CreateFulfillmentItemInput {
            order_line_item_id: order_line.id,
            quantity: item.quantity,
            metadata: fulfillment_item_metadata(
                item.metadata.clone(),
                plan.checkout_operation_id,
                item.cart_line_item_id,
                plan.plan_hash.as_str(),
                index,
            ),
        });
    }
    if items.is_empty() {
        return Err(CheckoutFulfillmentStageError::Conflict(format!(
            "fulfillment plan {index} contains no items"
        )));
    }
    Ok(CreateFulfillmentInput {
        order_id: order.id,
        shipping_option_id: fulfillment_plan.shipping_option_id,
        customer_id: order.customer_id,
        carrier: fulfillment_plan.carrier.clone(),
        tracking_number: fulfillment_plan.tracking_number.clone(),
        items: Some(items),
        metadata: fulfillment_metadata(
            fulfillment_plan.metadata.clone(),
            plan.checkout_operation_id,
            order.id,
            plan.plan_hash.as_str(),
            index,
            fulfillment_key,
        ),
    })
}

fn validate_fulfillment(
    fulfillment: &FulfillmentResponse,
    tenant_id: Uuid,
    order: &OrderResponse,
    input: &CreateFulfillmentInput,
    plan: &CheckoutOrderPlanRecord,
    index: usize,
    fulfillment_key: &str,
) -> CheckoutFulfillmentStageResult<()> {
    if fulfillment.tenant_id != tenant_id
        || fulfillment.order_id != order.id
        || fulfillment.shipping_option_id != input.shipping_option_id
        || fulfillment.customer_id != order.customer_id
        || fulfillment.carrier != input.carrier
        || fulfillment.tracking_number != input.tracking_number
    {
        return Err(CheckoutFulfillmentStageError::Conflict(format!(
            "fulfillment {} does not match immutable plan {index}",
            fulfillment.id
        )));
    }
    let expected_items = input
        .items
        .as_ref()
        .into_iter()
        .flatten()
        .map(|item| (item.order_line_item_id, item.quantity))
        .collect::<BTreeMap<_, _>>();
    let actual_items = fulfillment
        .items
        .iter()
        .map(|item| (item.order_line_item_id, item.quantity))
        .collect::<BTreeMap<_, _>>();
    if expected_items != actual_items {
        return Err(CheckoutFulfillmentStageError::Conflict(format!(
            "fulfillment {} items do not match immutable plan {index}",
            fulfillment.id
        )));
    }
    let checkout = fulfillment
        .metadata
        .get("checkout")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            CheckoutFulfillmentStageError::Conflict(format!(
                "fulfillment {} has no checkout identity metadata",
                fulfillment.id
            ))
        })?;
    let operation_id = plan.checkout_operation_id.to_string();
    if checkout.get("operation_id").and_then(Value::as_str) != Some(operation_id.as_str())
        || checkout.get("fulfillment_key").and_then(Value::as_str) != Some(fulfillment_key)
        || checkout.get("order_plan_hash").and_then(Value::as_str) != Some(plan.plan_hash.as_str())
        || checkout.get("fulfillment_index").and_then(Value::as_u64) != Some(index as u64)
    {
        return Err(CheckoutFulfillmentStageError::Conflict(format!(
            "fulfillment {} has a mismatched checkout identity",
            fulfillment.id
        )));
    }
    Ok(())
}

fn validate_captured_state(
    tenant_id: Uuid,
    state: &CheckoutPaymentCapturedState,
) -> CheckoutFulfillmentStageResult<()> {
    if state.order.tenant_id != tenant_id
        || state.plan.tenant_id != tenant_id
        || state.payment_collection.tenant_id != tenant_id
        || state.plan.checkout_operation_id != state.operation_id
        || state.payment_collection.order_id != Some(state.order.id)
        || state.payment_collection.status != "captured"
        || state.payment_collection.captured_amount != state.order.total_amount
    {
        return Err(CheckoutFulfillmentStageError::Conflict(
            "captured payment state does not describe one checkout identity".to_string(),
        ));
    }
    Ok(())
}

fn payment_reference(collection: &PaymentCollectionResponse, order_id: Uuid) -> String {
    collection
        .payments
        .last()
        .map(|payment| payment.provider_payment_id.clone())
        .unwrap_or_else(|| format!("manual_{order_id}"))
}

fn fulfillment_key(operation_id: Uuid, index: usize) -> String {
    format!("checkout:{operation_id}:fulfillment:{index}")
}

fn fulfillment_metadata(
    base: Value,
    operation_id: Uuid,
    order_id: Uuid,
    plan_hash: &str,
    index: usize,
    fulfillment_key: &str,
) -> Value {
    let mut root = object_or_empty(base);
    let mut checkout = root
        .remove("checkout")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    checkout.insert(
        "operation_id".to_string(),
        Value::String(operation_id.to_string()),
    );
    checkout.insert("order_id".to_string(), Value::String(order_id.to_string()));
    checkout.insert(
        "order_plan_hash".to_string(),
        Value::String(plan_hash.to_string()),
    );
    checkout.insert(
        "fulfillment_index".to_string(),
        Value::Number((index as u64).into()),
    );
    checkout.insert(
        "fulfillment_key".to_string(),
        Value::String(fulfillment_key.to_string()),
    );
    root.insert("checkout".to_string(), Value::Object(checkout));
    root.insert(
        "commerce_orchestration".to_string(),
        json!({"operation": "checkout_create_fulfillment"}),
    );
    Value::Object(root)
}

fn fulfillment_item_metadata(
    base: Value,
    operation_id: Uuid,
    cart_line_item_id: Uuid,
    plan_hash: &str,
    index: usize,
) -> Value {
    let mut root = object_or_empty(base);
    root.insert(
        "checkout".to_string(),
        json!({
            "operation_id": operation_id,
            "cart_line_item_id": cart_line_item_id,
            "order_plan_hash": plan_hash,
            "fulfillment_index": index,
        }),
    );
    Value::Object(root)
}

fn object_or_empty(value: Value) -> serde_json::Map<String, Value> {
    match value {
        Value::Object(object) => object,
        _ => Default::default(),
    }
}
