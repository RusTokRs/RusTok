//! Stock-relevant owner snapshot/preparation. No peer I/O may run under the parent write lock.
use super::*;
use crate::shipment_stock::{
    MAX_SHIPMENT_STOCK_ITEMS, PreparedFulfillmentStock, ShipmentStockAction,
    ShipmentStockItemScope, ShipmentStockScope,
};
use rustok_api::{PortContext, PortError};

impl FulfillmentService {
    /// Native F-only revalidation on the short claim transaction; never calls the stock port.
    pub(crate) async fn correlate_shipment_stock_scope(
        &self,
        txn: &DatabaseTransaction,
        expected: &ShipmentStockScope,
    ) -> FulfillmentResult<()> {
        let fulfillment = self
            .load_fulfillment(txn, expected.tenant_id, expected.fulfillment_id)
            .await?;
        let items = self
            .load_stock_items(txn, expected.tenant_id, expected.fulfillment_id)
            .await?;
        let requested = (expected.action == ShipmentStockAction::Ship).then(|| {
            expected
                .items
                .iter()
                .filter(|item| item.movement_quantity > 0)
                .map(|item| FulfillmentItemQuantityInput {
                    fulfillment_item_id: item.fulfillment_item_id,
                    quantity: item.movement_quantity,
                })
                .collect::<Vec<_>>()
        });
        let current = stock_scope(&fulfillment, &items, expected.action, requested.as_deref())?;
        if current != *expected {
            return Err(stale_stock_scope());
        }
        Ok(())
    }

    pub(super) async fn assert_progress_not_fenced(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        own_operation: Option<Uuid>,
    ) -> FulfillmentResult<()> {
        let mut query = entities::provider_operation::Entity::find()
            .filter(entities::provider_operation::Column::TenantId.eq(tenant_id))
            .filter(entities::provider_operation::Column::FulfillmentId.eq(fulfillment_id))
            .filter(super::super::provider_operation::unresolved_provider_effects());
        if let Some(own) = own_operation {
            query = query.filter(entities::provider_operation::Column::Id.ne(own));
        }
        if query.limit(1).one(txn).await?.is_some() {
            return Err(stock_refusal(
                "fulfillment.provider_operation_scope_busy",
                "retained provider outcome fences local progress; reconcile its original identity before another mutation",
            ));
        }
        Ok(())
    }
    pub(super) async fn finish_progress_transition(
        &self,
        txn: DatabaseTransaction,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        receipt: Option<rustok_outbox::idempotency::Lease>,
        budget: Option<(&PortContext, std::time::Instant)>,
    ) -> FulfillmentResult<FulfillmentResponse> {
        let fulfillment = self
            .load_fulfillment(&txn, tenant_id, fulfillment_id)
            .await?;
        let response = self
            .build_fulfillment_response(&txn, tenant_id, fulfillment)
            .await?;
        if let Some(receipt) = receipt {
            rustok_outbox::idempotency::complete(&txn, receipt, &response).await?;
        }
        if let Some((context, started)) = budget {
            crate::shipment_stock::remaining_stock_context(context, started)
                .map_err(FulfillmentError::OwnerPort)?;
        }
        txn.commit().await?;
        Ok(response)
    }

    /// Freeze native owner facts and prepare selected capabilities before a fresh provider
    /// effect. The returned opaque token must be revalidated under the parent lock.
    pub async fn prepare_shipment_stock(
        &self,
        context: PortContext,
        fulfillment_id: Uuid,
        action: ShipmentStockAction,
        requested: Option<&[FulfillmentItemQuantityInput]>,
    ) -> FulfillmentResult<PreparedFulfillmentStock> {
        self.prepare_shipment_stock_with_status(context, fulfillment_id, action, requested, None)
            .await
    }

    pub(crate) async fn prepare_shipment_stock_with_status(
        &self,
        context: PortContext,
        fulfillment_id: Uuid,
        action: ShipmentStockAction,
        requested: Option<&[FulfillmentItemQuantityInput]>,
        required_status: Option<&str>,
    ) -> FulfillmentResult<PreparedFulfillmentStock> {
        let mut context = context;
        if let Some(deadline) = context.deadline_ms {
            context.deadline_ms = Some(deadline.min(30_000));
        }
        let started = std::time::Instant::now();
        let tenant_id = stock_tenant(&context)?;
        validate_fulfillment_id(fulfillment_id)?;
        let fulfillment = entities::fulfillment::Entity::find_by_id(fulfillment_id)
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(FulfillmentError::FulfillmentNotFound(fulfillment_id))?;
        if required_status.is_some_and(|status| fulfillment.status != status) {
            return Err(FulfillmentError::OwnerPort(PortError::conflict(
                if required_status == Some(STATUS_PENDING) {
                    "fulfillment.pending_cancellation_conflict"
                } else {
                    "fulfillment.lifecycle_precondition_conflict"
                },
                "fulfillment source status changed; reject before stock preparation or carrier effects",
            )));
        }
        let items = self
            .load_stock_items(&self.db, tenant_id, fulfillment_id)
            .await?;
        if required_status == Some(STATUS_PENDING)
            && items
                .iter()
                .any(|item| item.shipped_quantity != 0 || item.delivered_quantity != 0)
        {
            return Err(FulfillmentError::OwnerPort(PortError::conflict(
                "fulfillment.pending_cancellation_conflict",
                "pending-only cancellation refuses retained shipping progress; reconcile lifecycle facts before effects",
            )));
        }
        let scope = stock_scope(&fulfillment, &items, action, requested)?;
        let plan = if requires_stock_preparation(&scope) {
            let budget = crate::shipment_stock::remaining_stock_context(&context, started)?;
            Some(
                self.shipment_stock
                    .prepare(&budget, &scope)
                    .await
                    .map_err(map_shipment_stock_error)?,
            )
        } else {
            None
        };
        Ok(PreparedFulfillmentStock {
            context,
            scope,
            plan,
            started,
        })
    }

    pub(super) async fn load_stock_items<C: ConnectionTrait>(
        &self,
        db: &C,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
    ) -> FulfillmentResult<Vec<entities::fulfillment_item::Model>> {
        let items = entities::fulfillment_item::Entity::find()
            .join(
                JoinType::InnerJoin,
                entities::fulfillment_item::Relation::Fulfillment.def(),
            )
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
            .filter(entities::fulfillment_item::Column::FulfillmentId.eq(fulfillment_id))
            .order_by_asc(entities::fulfillment_item::Column::CreatedAt)
            .order_by_asc(entities::fulfillment_item::Column::Id)
            .limit((MAX_SHIPMENT_STOCK_ITEMS + 1) as u64)
            .all(db)
            .await?;
        if items.len() > MAX_SHIPMENT_STOCK_ITEMS {
            return Err(item_limit_refusal());
        }
        Ok(items)
    }

    pub(super) async fn apply_prepared_stock(
        &self,
        txn: &DatabaseTransaction,
        prepared: Option<&PreparedFulfillmentStock>,
        fulfillment: &entities::fulfillment::Model,
        items: &[entities::fulfillment_item::Model],
        action: ShipmentStockAction,
        requested: Option<&[FulfillmentItemQuantityInput]>,
    ) -> FulfillmentResult<()> {
        let prepared = prepared.ok_or_else(stale_stock_scope)?;
        let current = stock_scope(fulfillment, items, action, requested)?;
        if current != prepared.scope || stock_tenant(&prepared.context)? != fulfillment.tenant_id {
            return Err(stale_stock_scope());
        }
        let movements = current
            .items
            .iter()
            .filter(|item| item.movement_quantity > 0)
            .map(|item| ShipmentStockRequest {
                fulfillment_item_id: item.fulfillment_item_id,
                order_line_item_id: item.order_line_item_id,
                quantity: item.movement_quantity,
            })
            .collect::<Vec<_>>();
        match (&prepared.plan, requires_stock_preparation(&current)) {
            (Some(plan), true) => {
                prepared
                    .remaining_context()
                    .map_err(FulfillmentError::OwnerPort)?;
                plan.apply(txn, &movements)
                    .await
                    .map_err(map_shipment_stock_error)
            }
            (None, false) => Ok(()),
            _ => Err(stale_stock_scope()),
        }
    }

    /// Receipt-only replay precedes new capability preparation. Validate journal ownership even
    /// for committed acknowledgements; never use provider metadata to authorize stock movement.
    pub(super) async fn provider_stock_replay(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        operation_id: Uuid,
        operation: &str,
        provider_metadata: &Value,
    ) -> FulfillmentResult<Option<FulfillmentResponse>> {
        let txn = self.db.begin().await?;
        let fulfillment = self
            .load_fulfillment_for_update(&txn, tenant_id, fulfillment_id)
            .await?;
        validate_journal_owned_provider_metadata(
            &txn,
            tenant_id,
            fulfillment_id,
            operation_id,
            operation,
            provider_metadata,
            None,
            &fulfillment.status,
        )
        .await?;
        let journaled = entities::provider_operation::Entity::find_by_id(operation_id)
            .filter(entities::provider_operation::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or_else(stale_stock_scope)?;
        if journaled.status == PROVIDER_OPERATION_COMMITTED
            || has_matching_provider_operation(&fulfillment.metadata, operation_id, operation)
        {
            FulfillmentProviderOperationJournal::new(self.db.clone())
                .mark_committed_in_txn(&txn, tenant_id, operation_id)
                .await?;
            let response = self
                .build_fulfillment_response(&txn, tenant_id, fulfillment)
                .await?;
            txn.commit().await?;
            Ok(Some(response))
        } else {
            validate_retained_provider_precondition(&journaled, &fulfillment.status)?;
            txn.rollback().await?;
            Ok(None)
        }
    }
}

fn stock_scope(
    fulfillment: &entities::fulfillment::Model,
    items: &[entities::fulfillment_item::Model],
    action: ShipmentStockAction,
    requested: Option<&[FulfillmentItemQuantityInput]>,
) -> FulfillmentResult<ShipmentStockScope> {
    let permitted = match action {
        ShipmentStockAction::Ship => {
            matches!(fulfillment.status.as_str(), STATUS_PENDING | STATUS_SHIPPED)
        }
        ShipmentStockAction::Restore => {
            matches!(fulfillment.status.as_str(), STATUS_PENDING | STATUS_SHIPPED)
        }
        ShipmentStockAction::Reconsume => matches!(
            fulfillment.status.as_str(),
            STATUS_CANCELLED | STATUS_DELIVERED
        ),
    };
    if !permitted {
        return Err(FulfillmentError::InvalidTransition {
            from: fulfillment.status.clone(),
            to: match action {
                ShipmentStockAction::Ship => STATUS_SHIPPED,
                ShipmentStockAction::Restore => STATUS_CANCELLED,
                ShipmentStockAction::Reconsume => "reopened",
            }
            .to_string(),
        });
    }
    if action == ShipmentStockAction::Ship
        && items.is_empty()
        && fulfillment.status != STATUS_PENDING
    {
        return Err(FulfillmentError::InvalidTransition {
            from: fulfillment.status.clone(),
            to: STATUS_SHIPPED.into(),
        });
    }
    if fulfillment.order_id.is_nil() || items.len() > MAX_SHIPMENT_STOCK_ITEMS {
        return Err(stale_stock_scope());
    }
    let mut ids = BTreeSet::new();
    for item in items {
        validate_item_progress_snapshot(item)?;
        if item.id.is_nil()
            || item.order_line_item_id.is_nil()
            || item.fulfillment_id != fulfillment.id
            || !ids.insert(item.id)
        {
            return Err(stale_stock_scope());
        }
        if fulfillment.status == STATUS_PENDING
            && (item.shipped_quantity != 0 || item.delivered_quantity != 0)
        {
            return Err(stock_refusal(
                "fulfillment.shipment_stock_history_unverified",
                "pending fulfillment has contradictory shipped progress; reconcile without stock inference",
            ));
        }
    }
    let quantities = if action == ShipmentStockAction::Ship && !items.is_empty() {
        validate_item_quantity_adjustments(requested)?;
        resolve_item_adjustments(items, requested, FulfillmentItemAction::Ship)?
            .into_iter()
            .collect::<BTreeMap<_, _>>()
    } else {
        if requested.is_some_and(|items| !items.is_empty()) {
            return Err(stale_stock_scope());
        }
        BTreeMap::new()
    };
    let mut scoped_items = items
        .iter()
        .map(|item| {
            let movement_quantity = match action {
                ShipmentStockAction::Ship => quantities.get(&item.id).copied().unwrap_or_default(),
                ShipmentStockAction::Restore if fulfillment.status == STATUS_SHIPPED => {
                    item.shipped_quantity - item.delivered_quantity
                }
                ShipmentStockAction::Reconsume if fulfillment.status == STATUS_CANCELLED => {
                    item.shipped_quantity - item.delivered_quantity
                }
                _ => 0,
            };
            ShipmentStockItemScope {
                fulfillment_item_id: item.id,
                order_line_item_id: item.order_line_item_id,
                quantity: item.quantity,
                shipped_quantity: item.shipped_quantity,
                delivered_quantity: item.delivered_quantity,
                movement_quantity,
            }
        })
        .collect::<Vec<_>>();
    scoped_items.sort_by_key(|item| item.fulfillment_item_id);
    Ok(ShipmentStockScope {
        tenant_id: fulfillment.tenant_id,
        order_id: fulfillment.order_id,
        fulfillment_id: fulfillment.id,
        status: fulfillment.status.clone(),
        updated_at: fulfillment.updated_at,
        action,
        items: scoped_items,
    })
}

pub(super) fn item_limit_refusal() -> FulfillmentError {
    stock_refusal(
        "fulfillment.shipment_stock_item_limit",
        "complete fulfillment projection exceeds 1024 items; split or reconcile owner work without truncating history",
    )
}

fn requires_stock_preparation(scope: &ShipmentStockScope) -> bool {
    // Missing typed shipment facts must not become an implicit no-stock opt-out. The native
    // adapter rejects unknown scope; only an explicitly configured Noop accepts an empty batch.
    (scope.action == ShipmentStockAction::Ship && scope.items.is_empty())
        || scope.items.iter().any(|item| item.movement_quantity > 0)
}

pub(super) fn stock_tenant(context: &PortContext) -> FulfillmentResult<Uuid> {
    Uuid::parse_str(context.tenant_id.trim())
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| {
            stock_refusal(
                "fulfillment.shipment_stock_tenant_invalid",
                "shipment stock requires canonical nonempty tenant context",
            )
        })
}
fn stale_stock_scope() -> FulfillmentError {
    stock_refusal(
        "fulfillment.shipment_stock_snapshot_conflict",
        "prepared shipment stock facts changed; re-read owner state and reconcile any retained provider result",
    )
}
fn stock_refusal(code: &str, message: &str) -> FulfillmentError {
    FulfillmentError::OwnerPort(PortError::invariant_violation(code, message))
}

/// The carrier's retained request, not caller/provider metadata, fixes local progress inputs.
/// A missing historical request projection refuses fresh movement; a committed/applied replay
/// remains receipt-only. Never reconstruct these facts from current counters or stock quantity.
pub(super) fn validate_journal_local_input(
    journal: &entities::provider_operation::Model,
    input: &Value,
    owner_status: &str,
) -> FulfillmentResult<()> {
    let invalid = || {
        FulfillmentError::ProviderResultInvalid("local lifecycle input does not match the retained provider request/result; reconcile original scope".into())
    };
    let request = journal.request_payload.as_object().ok_or_else(invalid)?;
    if request
        .get("tenant_id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
        != Some(journal.tenant_id)
        || request
            .get("fulfillment_id")
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
            != Some(journal.fulfillment_id)
        || request.get("idempotency_key").and_then(Value::as_str)
            != Some(journal.idempotency_key.as_str())
    {
        return Err(invalid());
    }
    let metadata = request
        .get("metadata")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    let scope = metadata
        .get("commerce_orchestration")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    if scope.get("operation").and_then(Value::as_str) != Some(journal.operation.as_str()) {
        return Err(invalid());
    }
    let clean_metadata = |value: &Value| -> Option<Map<String, Value>> {
        let mut metadata = value.as_object()?.clone();
        metadata.remove("commerce_orchestration");
        metadata.remove("provider_operation");
        Some(metadata)
    };
    if clean_metadata(request.get("metadata").ok_or_else(invalid)?)
        != clean_metadata(input.get("metadata").ok_or_else(invalid)?)
    {
        return Err(invalid());
    }
    match journal.operation.as_str() {
        "ship" | "reship" => {
            let expected_carrier = scope
                .get("carrier")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?
                .trim();
            let requested_tracking = scope
                .get("tracking_number")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?
                .trim();
            let result = journal.provider_result.as_ref().ok_or_else(invalid)?;
            let expected_tracking = match result.get("tracking_number") {
                Some(Value::String(value)) => value.trim(),
                Some(Value::Null) | None => requested_tracking,
                _ => return Err(invalid()),
            };
            if input.get("carrier").and_then(Value::as_str).map(str::trim) != Some(expected_carrier)
                || input
                    .get("tracking_number")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    != Some(expected_tracking)
                || scope.get("items").is_none()
                || scope.get("items") != input.get("items")
            {
                return Err(invalid());
            }
        }
        "cancel" => {
            match scope.get("required_status") {
                Some(Value::String(status))
                    if status == STATUS_PENDING && owner_status == STATUS_PENDING => {}
                Some(Value::Null) | None => {}
                _ => return Err(invalid()),
            }
            let normalized_reason =
                |value: Option<&Value>| -> Result<Option<String>, FulfillmentError> {
                    match value {
                        Some(Value::Null) => Ok(None),
                        Some(Value::String(value)) => Ok(Some(value.trim().to_string())),
                        _ => Err(invalid()),
                    }
                };
            if normalized_reason(scope.get("reason"))? != normalized_reason(input.get("reason"))? {
                return Err(invalid());
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}

impl FulfillmentService {
    pub(super) async fn validate_provider_stock_context(
        &self,
        context: &PortContext,
        fulfillment_id: Uuid,
        operation_id: Uuid,
        operation: &str,
    ) -> FulfillmentResult<()> {
        let tenant_id = stock_tenant(context)?;
        validate_fulfillment_id(fulfillment_id)?;
        let saved = FulfillmentProviderOperationJournal::new(self.db.clone())
            .get(tenant_id, operation_id)
            .await?;
        if saved.fulfillment_id != fulfillment_id
            || saved.operation != operation
            || context.idempotency_key.as_deref() != Some(saved.idempotency_key.as_str())
        {
            return Err(stock_refusal(
                "fulfillment.shipment_stock_recovery_identity_unverified",
                "stock recovery requires the retained provider operation identity and scope; do not rotate keys",
            ));
        }
        Ok(())
    }
}

fn validate_retained_provider_precondition(
    journal: &entities::provider_operation::Model,
    owner_status: &str,
) -> FulfillmentResult<()> {
    if journal.operation == "cancel" {
        match journal
            .request_payload
            .get("metadata")
            .and_then(|metadata| metadata.get("commerce_orchestration"))
            .and_then(|scope| scope.get("required_status"))
        {
            Some(Value::String(status))
                if status == STATUS_PENDING && owner_status == STATUS_PENDING => {}
            Some(Value::Null) | None => {}
            _ => return Err(FulfillmentError::ProviderResultInvalid(
                "retained pending-only cancellation cannot be widened to shipped stock restoration"
                    .into(),
            )),
        }
    }
    Ok(())
}
