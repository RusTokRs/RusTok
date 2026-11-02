use chrono::Utc;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_inventory::{
    InventoryIdentityReservationAdoptionItem, InventoryIdentityReservationAdoptionRequest,
    InventoryIdentityReservationAdoptionSnapshot, InventoryReservationIdentityPort,
    MAX_RESERVATION_ADOPTION_ITEMS,
};
use rustok_order::OrderStatusKind;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{checkout_inventory_reservation, checkout_operation};

use super::{
    CheckoutInventoryReservationError, CheckoutInventoryReservationJournal,
    CheckoutInventoryReservationStatus, CheckoutOperationCheckpoint, CheckoutOperationError,
    CheckoutOperationJournal, CheckoutOperationStage, CheckoutOperationStatus,
    DEFAULT_CHECKOUT_LEASE_SECONDS,
};

#[derive(Debug, Error)]
pub enum CheckoutInventoryOrderAdoptionError {
    #[error(transparent)]
    ReservationJournal(#[from] CheckoutInventoryReservationError),
    #[error(transparent)]
    OperationJournal(#[from] CheckoutOperationError),
    #[error("checkout inventory order adoption conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(
        "checkout inventory adoption boundary failed with `{code}` (retryable={retryable}): {message}"
    )]
    Boundary {
        code: String,
        message: &'static str,
        retryable: bool,
    },
    #[error(
        "checkout inventory adoption handoff requires recovery with `{code}` (retryable={retryable})"
    )]
    AfterOwnerCommit {
        code: &'static str,
        retryable: bool,
        #[source]
        cause: Box<CheckoutInventoryOrderAdoptionError>,
    },
}

pub type CheckoutInventoryOrderAdoptionResult<T> = Result<T, CheckoutInventoryOrderAdoptionError>;

impl CheckoutInventoryOrderAdoptionError {
    pub fn bounded_failure(&self) -> Option<(&str, bool)> {
        match self {
            Self::Boundary {
                code, retryable, ..
            } => Some((code.as_str(), *retryable)),
            Self::AfterOwnerCommit {
                code, retryable, ..
            } => Some((*code, *retryable)),
            // A failed read/provenance check before this invocation is not proof that an
            // earlier same-identity attempt never committed. Never authorize compensation
            // from the absence of a fresh owner call; retain a bounded local classification.
            Self::ReservationJournal(_)
            | Self::OperationJournal(_)
            | Self::Conflict(_)
            | Self::Database(_) => Some(local_adoption_failure(self)),
        }
    }

    /// Retry advice is not an outcome receipt. An unverified owner handoff must not cause
    /// automatic stock release/order cancellation merely because retryable is false.
    pub fn requires_owner_recovery(&self) -> bool {
        self.bounded_failure().is_some()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckoutInventoryOrderAdoption {
    pub reservation_id: Uuid,
    pub cart_line_item_id: Uuid,
    pub order_line_item_id: Uuid,
}

#[derive(Clone)]
pub struct CheckoutInventoryOrderAdoptionService {
    db: DatabaseConnection,
    reservation_journal: CheckoutInventoryReservationJournal,
    operation_journal: CheckoutOperationJournal,
    inventory_port: Arc<dyn InventoryReservationIdentityPort>,
    lease_seconds: i64,
}

#[derive(Clone, Copy, Debug)]
struct OrderLineBinding {
    cart_line_item_id: Uuid,
    order_line_item_id: Uuid,
    variant_id: Uuid,
    quantity: i32,
}

impl CheckoutInventoryOrderAdoptionService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        inventory_port: Arc<dyn InventoryReservationIdentityPort>,
    ) -> Self {
        Self {
            reservation_journal: CheckoutInventoryReservationJournal::new(db.clone()),
            operation_journal: CheckoutOperationJournal::new(db.clone(), event_bus),
            db,
            inventory_port,
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
        }
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    /// Requests owner-local atomic adoption, correlates the complete reply, then updates only
    /// Commerce's mapping journal and checkpoints `order_created` in separate transactions.
    /// A lost response or checkpoint failure must replay the same immutable owner identities;
    /// there is no cross-owner transaction, stock reallocation or local-provider fallback.
    pub async fn adopt_and_checkpoint(
        &self,
        context: PortContext,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        order: &rustok_order::OrderResponse,
    ) -> CheckoutInventoryOrderAdoptionResult<Vec<CheckoutInventoryOrderAdoption>> {
        context
            .require_policy(PortCallPolicy::write())
            .map_err(inventory_adoption_boundary)?;
        context
            .require_write_semantics()
            .map_err(inventory_adoption_boundary)?;
        let tenant_id = Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
            CheckoutInventoryOrderAdoptionError::Conflict(
                "inventory adoption requires a valid tenant context".to_string(),
            )
        })?;
        if tenant_id.is_nil() || operation_id.is_nil() || order.id.is_nil() {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(
                "tenant_id, operation_id, and order.id must not be nil UUID".to_string(),
            ));
        }
        let lease_owner = lease_owner.into();
        tracing::debug!(
            tenant_id = %tenant_id,
            operation_id = %operation_id,
            order_id = %order.id,
            lease_owner = %lease_owner,
            "adopt_and_checkpoint entry"
        );
        let operation = self
            .operation_journal
            .renew_lease(
                tenant_id,
                operation_id,
                lease_owner.as_str(),
                self.lease_seconds,
            )
            .await?;
        validate_order_provenance(tenant_id, operation_id, order)?;

        if operation.status != CheckoutOperationStatus::Executing.as_str() {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                "checkout operation {} must be executing, not `{}`",
                operation.id, operation.status
            )));
        }

        if operation.stage == CheckoutOperationStage::OrderCreated.as_str() {
            if operation.order_id != Some(order.id) {
                return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                    "checkout operation {} is already bound to another order",
                    operation.id
                )));
            }
            return self
                .validate_adopted_rows(tenant_id, operation_id, order)
                .await
                .map_err(after_owner_commit);
        }

        if operation.stage != CheckoutOperationStage::InventoryReserved.as_str() {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                "checkout operation {} cannot adopt order inventory from stage `{}`",
                operation.id, operation.stage
            )));
        }
        if !matches!(
            order.status_kind(),
            OrderStatusKind::Pending | OrderStatusKind::Confirmed
        ) {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                "order {} must be pending or confirmed while checkout reservations are adopted, not `{}`",
                order.id, order.status
            )));
        }
        if operation.order_id.is_some() && operation.order_id != Some(order.id) {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                "checkout operation {} is already bound to another order",
                operation.id
            )));
        }

        let bindings = order_line_bindings(order)?;
        let mappings = self
            .validate_reservation_set(tenant_id, operation_id, &bindings)
            .await?;
        self.adopt_rows(&context, &operation, order.id, &bindings, &mappings)
            .await?;

        self.operation_journal
            .checkpoint(CheckoutOperationCheckpoint {
                tenant_id,
                operation_id,
                lease_owner,
                expected_stage: CheckoutOperationStage::InventoryReserved,
                next_stage: CheckoutOperationStage::OrderCreated,
                snapshot_hash: None,
                order_id: Some(order.id),
                payment_collection_id: operation.payment_collection_id,
                lease_seconds: self.lease_seconds,
            })
            .await
            .map_err(|error| after_owner_commit(error.into()))?;

        self.validate_adopted_rows(tenant_id, operation_id, order)
            .await
            .map_err(after_owner_commit)
    }

    async fn validate_reservation_set(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        bindings: &[OrderLineBinding],
    ) -> CheckoutInventoryOrderAdoptionResult<Vec<checkout_inventory_reservation::Model>> {
        let mappings = self
            .reservation_journal
            .list_by_operation(tenant_id, operation_id)
            .await?;
        if mappings.len() != bindings.len() {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                "checkout operation {} has {} reservation rows for {} variant-backed order lines",
                operation_id,
                mappings.len(),
                bindings.len()
            )));
        }

        let by_cart_line = bindings
            .iter()
            .map(|binding| (binding.cart_line_item_id, *binding))
            .collect::<HashMap<_, _>>();
        for mapping in &mappings {
            let binding = by_cart_line
                .get(&mapping.cart_line_item_id)
                .ok_or_else(|| {
                    CheckoutInventoryOrderAdoptionError::Conflict(format!(
                        "reservation {} has no matching order line for cart line {}",
                        mapping.reservation_id, mapping.cart_line_item_id
                    ))
                })?;
            validate_mapping(mapping, tenant_id, operation_id, binding)?;
        }

        Ok(mappings)
    }

    async fn adopt_rows(
        &self,
        context: &PortContext,
        operation: &checkout_operation::Model,
        order_id: Uuid,
        bindings: &[OrderLineBinding],
        mappings: &[checkout_inventory_reservation::Model],
    ) -> CheckoutInventoryOrderAdoptionResult<()> {
        let tenant_id = operation.tenant_id;
        let operation_id = operation.id;
        let binding_by_cart_line = bindings
            .iter()
            .map(|binding| (binding.cart_line_item_id, *binding))
            .collect::<HashMap<_, _>>();
        // An order without variant-backed rows has no Inventory side effects to adopt.
        // Coverage has already been checked by validate_reservation_set.
        if mappings.is_empty() {
            return Ok(());
        }
        if mappings.len() > MAX_RESERVATION_ADOPTION_ITEMS {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(
                "inventory adoption exceeds 1024 reservation bindings".to_string(),
            ));
        }
        let mut items = Vec::with_capacity(mappings.len());
        for mapping in mappings {
            let binding = binding_by_cart_line
                .get(&mapping.cart_line_item_id)
                .ok_or_else(|| {
                    CheckoutInventoryOrderAdoptionError::Conflict(
                        "reservation has no validated cart-line binding".to_string(),
                    )
                })?;
            validate_mapping(mapping, tenant_id, operation_id, binding)?;
            if mapping
                .order_line_item_id
                .is_some_and(|id| id != binding.order_line_item_id)
            {
                return Err(CheckoutInventoryOrderAdoptionError::Conflict(
                    "reservation journal is already adopted by another order line".to_string(),
                ));
            }
            items.push(InventoryIdentityReservationAdoptionItem {
                reservation_id: mapping.reservation_id,
                external_id: mapping.external_id.clone(),
                variant_id: mapping.variant_id,
                quantity: mapping.quantity,
                location_id: mapping
                    .location_id
                    .filter(|id| !id.is_nil())
                    .ok_or_else(|| {
                        CheckoutInventoryOrderAdoptionError::Conflict(
                            "reserved inventory journal has no saved allocation location"
                                .to_string(),
                        )
                    })?,
                cart_line_item_id: binding.cart_line_item_id,
                order_line_item_id: binding.order_line_item_id,
            });
        }
        items.sort_by_key(|item| item.cart_line_item_id);
        let request = InventoryIdentityReservationAdoptionRequest {
            checkout_operation_id: operation_id,
            cart_id: operation.cart_id,
            order_id,
            items,
        };
        self.request_owner_adoption(context, &request).await?;

        // The owner has committed. A failure below is deliberately recoverable by exact replay
        // of the same request; never roll back/replace that allocation as a transport fallback.
        let journal_result: CheckoutInventoryOrderAdoptionResult<()> = async {
            let txn = self.db.begin().await?;
            for item in &request.items {
                let result = checkout_inventory_reservation::Entity::update_many()
                    .filter(
                        checkout_inventory_reservation::Column::ReservationId
                            .eq(item.reservation_id),
                    )
                    .filter(checkout_inventory_reservation::Column::TenantId.eq(tenant_id))
                    .filter(
                        checkout_inventory_reservation::Column::CheckoutOperationId
                            .eq(operation_id),
                    )
                    .filter(
                        checkout_inventory_reservation::Column::CartLineItemId
                            .eq(item.cart_line_item_id),
                    )
                    .filter(
                        checkout_inventory_reservation::Column::ExternalId
                            .eq(item.external_id.as_str()),
                    )
                    .filter(checkout_inventory_reservation::Column::VariantId.eq(item.variant_id))
                    .filter(checkout_inventory_reservation::Column::Quantity.eq(item.quantity))
                    .filter(checkout_inventory_reservation::Column::LocationId.eq(item.location_id))
                    .filter(
                        checkout_inventory_reservation::Column::Status
                            .eq(CheckoutInventoryReservationStatus::Reserved.as_str()),
                    )
                    .filter(
                        Condition::any()
                            .add(checkout_inventory_reservation::Column::OrderLineItemId.is_null())
                            .add(
                                checkout_inventory_reservation::Column::OrderLineItemId
                                    .eq(item.order_line_item_id),
                            ),
                    )
                    .set(checkout_inventory_reservation::ActiveModel {
                        order_line_item_id: Set(Some(item.order_line_item_id)),
                        updated_at: Set(Utc::now().into()),
                        ..Default::default()
                    })
                    .exec(&txn)
                    .await?;
                if result.rows_affected != 1 {
                    // Do not let an unrelated matching target UUID conceal changed source data.
                    let current =
                        checkout_inventory_reservation::Entity::find_by_id(item.reservation_id)
                            .filter(checkout_inventory_reservation::Column::TenantId.eq(tenant_id))
                            .filter(
                                checkout_inventory_reservation::Column::CheckoutOperationId
                                    .eq(operation_id),
                            )
                            .one(&txn)
                            .await?
                            .ok_or_else(|| {
                                CheckoutInventoryOrderAdoptionError::Conflict(
                                    "reservation journal disappeared after owner adoption"
                                        .to_string(),
                                )
                            })?;
                    let binding = binding_by_cart_line
                        .get(&item.cart_line_item_id)
                        .ok_or_else(|| {
                            CheckoutInventoryOrderAdoptionError::Conflict(
                                "reservation journal lost its validated cart-line binding"
                                    .to_string(),
                            )
                        })?;
                    validate_mapping(&current, tenant_id, operation_id, binding)?;
                    if current.order_line_item_id != Some(item.order_line_item_id)
                        || current.external_id != item.external_id
                        || current.location_id != Some(item.location_id)
                    {
                        return Err(CheckoutInventoryOrderAdoptionError::Conflict(
                            "reservation journal adoption lost a compare-and-set race".to_string(),
                        ));
                    }
                }
            }
            txn.commit().await?;
            Ok(())
        }
        .await;
        journal_result.map_err(after_owner_commit)
    }

    async fn request_owner_adoption(
        &self,
        context: &PortContext,
        request: &InventoryIdentityReservationAdoptionRequest,
    ) -> CheckoutInventoryOrderAdoptionResult<()> {
        // A zero timeout may poll an immediately-ready provider once. Reject it with the
        // canonical policy before constructing/polling the selected owner invocation.
        context
            .require_deadline_semantics()
            .map_err(inventory_adoption_boundary)?;
        // INVARIANT: the immutable context just passed the canonical positive-deadline check.
        let deadline_ms = context.deadline_ms.unwrap_or_default().min(30_000);
        let deadline = Duration::from_millis(deadline_ms);
        let mut owner_context = context.clone();
        owner_context.deadline_ms = Some(deadline_ms);
        let response = tokio::time::timeout(
            deadline,
            self.inventory_port
                .adopt_inventory_by_identity(owner_context, request.clone()),
        )
        .await
        .map_err(|_| {
            inventory_adoption_boundary(PortError::timeout(
                "inventory.adoption_timeout",
                "inventory adoption outcome is unknown; recover the same checkout identity",
            ))
        })?
        .map_err(inventory_adoption_boundary)?;
        validate_inventory_adoption_response(request, &response)
    }

    async fn validate_adopted_rows(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        order: &rustok_order::OrderResponse,
    ) -> CheckoutInventoryOrderAdoptionResult<Vec<CheckoutInventoryOrderAdoption>> {
        let bindings = order_line_bindings(order)?;
        self.validate_reservation_set(tenant_id, operation_id, &bindings)
            .await?;
        let rows = load_operation_adoptions(&self.db, tenant_id, operation_id).await?;
        let expected = bindings
            .iter()
            .map(|binding| (binding.cart_line_item_id, binding.order_line_item_id))
            .collect::<HashSet<_>>();
        let actual = rows
            .iter()
            .map(|row| (row.cart_line_item_id, row.order_line_item_id))
            .collect::<HashSet<_>>();
        if expected != actual {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                "checkout operation {} does not have a complete order-line adoption",
                operation_id
            )));
        }
        Ok(rows)
    }
}

async fn load_operation_adoptions<C>(
    conn: &C,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> Result<Vec<CheckoutInventoryOrderAdoption>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let rows = checkout_inventory_reservation::Entity::find()
        .filter(checkout_inventory_reservation::Column::TenantId.eq(tenant_id))
        .filter(checkout_inventory_reservation::Column::CheckoutOperationId.eq(operation_id))
        .filter(checkout_inventory_reservation::Column::OrderLineItemId.is_not_null())
        .order_by_asc(checkout_inventory_reservation::Column::CartLineItemId)
        .all(conn)
        .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            row.order_line_item_id
                .map(|order_line_item_id| CheckoutInventoryOrderAdoption {
                    reservation_id: row.reservation_id,
                    cart_line_item_id: row.cart_line_item_id,
                    order_line_item_id,
                })
        })
        .collect())
}

fn validate_order_provenance(
    tenant_id: Uuid,
    operation_id: Uuid,
    order: &rustok_order::OrderResponse,
) -> CheckoutInventoryOrderAdoptionResult<()> {
    if order.tenant_id != tenant_id {
        return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
            "order {} belongs to another tenant",
            order.id
        )));
    }
    let source_operation = order
        .metadata
        .get("checkout")
        .and_then(|checkout| checkout.get("operation_id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    if source_operation != Some(operation_id) {
        return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
            "order {} is not attributed to checkout operation {}",
            order.id, operation_id
        )));
    }
    Ok(())
}

fn order_line_bindings(
    order: &rustok_order::OrderResponse,
) -> CheckoutInventoryOrderAdoptionResult<Vec<OrderLineBinding>> {
    let mut bindings = Vec::new();
    let mut cart_line_ids = HashSet::new();
    let mut order_line_ids = HashSet::new();
    for line in &order.line_items {
        let Some(variant_id) = line.variant_id else {
            continue;
        };
        if line.quantity <= 0
            || line.id.is_nil()
            || variant_id.is_nil()
            || !order_line_ids.insert(line.id)
        {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                "order line {} has an invalid quantity or identity",
                line.id
            )));
        }
        let cart_line_item_id = line
            .metadata
            .get("checkout")
            .and_then(|checkout| checkout.get("cart_line_item_id"))
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| {
                CheckoutInventoryOrderAdoptionError::Conflict(format!(
                    "order line {} has no valid checkout cart-line provenance",
                    line.id
                ))
            })?;
        if cart_line_item_id.is_nil() || !cart_line_ids.insert(cart_line_item_id) {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                "order {} maps multiple lines to cart line {}",
                order.id, cart_line_item_id
            )));
        }
        bindings.push(OrderLineBinding {
            cart_line_item_id,
            order_line_item_id: line.id,
            variant_id,
            quantity: line.quantity,
        });
    }
    bindings.sort_by_key(|binding| binding.cart_line_item_id);
    Ok(bindings)
}

fn validate_mapping(
    mapping: &checkout_inventory_reservation::Model,
    tenant_id: Uuid,
    operation_id: Uuid,
    binding: &OrderLineBinding,
) -> CheckoutInventoryOrderAdoptionResult<()> {
    if mapping.tenant_id != tenant_id
        || mapping.checkout_operation_id != operation_id
        || mapping.cart_line_item_id != binding.cart_line_item_id
        || mapping.status != CheckoutInventoryReservationStatus::Reserved.as_str()
        || mapping.variant_id != binding.variant_id
        || mapping.quantity != binding.quantity
    {
        return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
            "reservation {} does not match order line {}",
            mapping.reservation_id, binding.order_line_item_id
        )));
    }
    Ok(())
}

fn validate_inventory_adoption_response(
    request: &InventoryIdentityReservationAdoptionRequest,
    response: &InventoryIdentityReservationAdoptionSnapshot,
) -> CheckoutInventoryOrderAdoptionResult<()> {
    if response.checkout_operation_id != request.checkout_operation_id
        || response.cart_id != request.cart_id
        || response.order_id != request.order_id
        || response.items != request.items
    {
        return Err(inventory_adoption_boundary(PortError::invariant_violation(
            "inventory.adoption_response_mismatch",
            "inventory returned bindings that differ from the immutable checkout adoption request",
        )));
    }
    Ok(())
}

fn inventory_adoption_boundary(error: PortError) -> CheckoutInventoryOrderAdoptionError {
    // Neither private diagnostics nor an arbitrary provider message/code is public metadata.
    let code = if !error.code.is_empty()
        && error.code.len() <= 100
        && error.code.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        }) {
        error.code.clone()
    } else {
        "inventory.adoption_owner_failure".to_string()
    };
    tracing::warn!(
        owner = "rustok_inventory",
        operation = "adopt_inventory_by_identity",
        boundary = "commerce_checkout_inventory_adoption",
        code = %code,
        error_kind = ?error.kind,
        retryable = error.retryable,
        "checkout inventory adoption owner boundary did not return a verified handoff"
    );
    CheckoutInventoryOrderAdoptionError::Boundary {
        code,
        message: "Inventory could not verify adoption of the existing checkout reservations",
        retryable: error.retryable,
    }
}

fn local_adoption_failure(cause: &CheckoutInventoryOrderAdoptionError) -> (&'static str, bool) {
    let retryable = matches!(
        cause,
        CheckoutInventoryOrderAdoptionError::Database(_)
            | CheckoutInventoryOrderAdoptionError::ReservationJournal(
                CheckoutInventoryReservationError::Database(_)
            )
            | CheckoutInventoryOrderAdoptionError::OperationJournal(
                CheckoutOperationError::Database(_)
            )
    );
    (
        if retryable {
            "checkout.inventory_adoption_journal_unavailable"
        } else {
            "checkout.inventory_adoption_handoff_unverified"
        },
        retryable,
    )
}

fn after_owner_commit(
    cause: CheckoutInventoryOrderAdoptionError,
) -> CheckoutInventoryOrderAdoptionError {
    if matches!(
        &cause,
        CheckoutInventoryOrderAdoptionError::Boundary { .. }
            | CheckoutInventoryOrderAdoptionError::AfterOwnerCommit { .. }
    ) {
        return cause;
    }
    let (code, retryable) = local_adoption_failure(&cause);
    CheckoutInventoryOrderAdoptionError::AfterOwnerCommit {
        code,
        retryable,
        cause: Box::new(cause),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortErrorKind;

    fn request() -> InventoryIdentityReservationAdoptionRequest {
        InventoryIdentityReservationAdoptionRequest {
            checkout_operation_id: Uuid::new_v4(),
            cart_id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            items: vec![InventoryIdentityReservationAdoptionItem {
                reservation_id: Uuid::new_v4(),
                external_id: "original-checkout-reservation".to_string(),
                variant_id: Uuid::new_v4(),
                quantity: 3,
                location_id: Uuid::new_v4(),
                cart_line_item_id: Uuid::new_v4(),
                order_line_item_id: Uuid::new_v4(),
            }],
        }
    }

    #[test]
    fn adoption_response_requires_the_entire_immutable_identity_and_exact_coverage() {
        let request = request();
        let expected = InventoryIdentityReservationAdoptionSnapshot {
            checkout_operation_id: request.checkout_operation_id,
            cart_id: request.cart_id,
            order_id: request.order_id,
            items: request.items.clone(),
        };
        validate_inventory_adoption_response(&request, &expected).unwrap();
        for field in 0..12 {
            let mut response = expected.clone();
            match field {
                0 => response.checkout_operation_id = Uuid::new_v4(),
                1 => response.cart_id = Uuid::new_v4(),
                2 => response.order_id = Uuid::new_v4(),
                3 => response.items[0].reservation_id = Uuid::new_v4(),
                4 => response.items[0].external_id.push_str(":different"),
                5 => response.items[0].variant_id = Uuid::new_v4(),
                6 => response.items[0].quantity += 1,
                7 => response.items[0].location_id = Uuid::new_v4(),
                8 => response.items[0].cart_line_item_id = Uuid::new_v4(),
                9 => response.items[0].order_line_item_id = Uuid::new_v4(),
                10 => response.items.clear(),
                11 => response.items.push(response.items[0].clone()),
                _ => unreachable!(),
            }
            let error = validate_inventory_adoption_response(&request, &response).unwrap_err();
            assert_eq!(
                error.bounded_failure(),
                Some(("inventory.adoption_response_mismatch", false))
            );
            assert!(error.requires_owner_recovery());
        }
    }

    #[test]
    fn adoption_boundary_never_copies_private_messages_and_preserves_exact_retry_advice() {
        for retryable in [false, true] {
            let error = inventory_adoption_boundary(PortError {
                kind: PortErrorKind::Unavailable,
                code: "inventory.database_unavailable".to_string(),
                message: "private sql payload buyer@example.invalid secret-marker".to_string(),
                retryable,
            });
            assert_eq!(
                error.bounded_failure(),
                Some(("inventory.database_unavailable", retryable))
            );
            assert!(error.requires_owner_recovery());
            assert!(!format!("{error:?} {error}").contains("secret-marker"));
        }
        for code in ["inventory.secret\nraw-detail".to_string(), "x".repeat(101)] {
            let error = inventory_adoption_boundary(PortError {
                kind: PortErrorKind::Conflict,
                code,
                message: "secret-marker".to_string(),
                retryable: false,
            });
            assert_eq!(
                error.bounded_failure(),
                Some(("inventory.adoption_owner_failure", false))
            );
            assert!(!error.to_string().contains("secret-marker"));
        }
    }

    #[test]
    fn post_owner_journal_failure_retains_a_cause_without_claiming_rollback_or_success() {
        use std::error::Error;
        let unavailable = after_owner_commit(CheckoutInventoryOrderAdoptionError::Database(
            sea_orm::DbErr::Custom("private-storage-diagnostic".to_string()),
        ));
        assert_eq!(
            unavailable.bounded_failure(),
            Some(("checkout.inventory_adoption_journal_unavailable", true))
        );
        assert!(unavailable.requires_owner_recovery());
        assert!(unavailable.source().is_some());
        assert!(
            !unavailable
                .to_string()
                .contains("private-storage-diagnostic")
        );
        let conflict = after_owner_commit(CheckoutInventoryOrderAdoptionError::Conflict(
            "inconsistent journal state".to_string(),
        ));
        assert_eq!(
            conflict.bounded_failure(),
            Some(("checkout.inventory_adoption_handoff_unverified", false))
        );
        assert!(conflict.requires_owner_recovery());
    }

    #[derive(Clone, Copy)]
    enum OwnerOutcome {
        Mismatch,
        Reject(bool),
        Pending,
    }

    type RecordedAdoptions =
        Arc<std::sync::Mutex<Vec<(PortContext, InventoryIdentityReservationAdoptionRequest)>>>;

    struct RecordingInventory {
        calls: RecordedAdoptions,
        outcome: OwnerOutcome,
    }

    #[async_trait::async_trait]
    impl InventoryReservationIdentityPort for RecordingInventory {
        async fn reserve_inventory_by_identity(
            &self,
            _context: PortContext,
            _request: rustok_inventory::InventoryIdentityReservationRequest,
        ) -> Result<rustok_inventory::InventoryIdentityReservationSnapshot, PortError> {
            Err(PortError::forbidden(
                "inventory.test_reserve_not_used",
                "test only exercises adoption",
            ))
        }
        async fn release_inventory_by_identity(
            &self,
            _context: PortContext,
            _request: rustok_inventory::InventoryIdentityReservationReleaseRequest,
        ) -> Result<rustok_inventory::InventoryIdentityReservationReleaseSnapshot, PortError>
        {
            Err(PortError::forbidden(
                "inventory.test_release_not_used",
                "test only exercises adoption",
            ))
        }
        async fn adopt_inventory_by_identity(
            &self,
            context: PortContext,
            request: InventoryIdentityReservationAdoptionRequest,
        ) -> Result<InventoryIdentityReservationAdoptionSnapshot, PortError> {
            self.calls.lock().unwrap().push((context, request.clone()));
            match self.outcome {
                OwnerOutcome::Mismatch => Ok(InventoryIdentityReservationAdoptionSnapshot {
                    checkout_operation_id: request.checkout_operation_id,
                    cart_id: request.cart_id,
                    order_id: request.order_id,
                    items: vec![],
                }),
                OwnerOutcome::Reject(retryable) => Err(PortError {
                    kind: PortErrorKind::Unavailable,
                    code: "inventory.adoption_unavailable".to_string(),
                    message: "private-owner-diagnostic".to_string(),
                    retryable,
                }),
                OwnerOutcome::Pending => std::future::pending().await,
            }
        }
    }

    fn selected_owner(
        outcome: OwnerOutcome,
    ) -> (CheckoutInventoryOrderAdoptionService, RecordedAdoptions) {
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let port = Arc::new(RecordingInventory {
            calls: calls.clone(),
            outcome,
        });
        (
            CheckoutInventoryOrderAdoptionService::new(
                DatabaseConnection::default(),
                rustok_test_utils::mock_transactional_event_bus(),
                port,
            ),
            calls,
        )
    }

    fn adoption_context(deadline: Duration) -> PortContext {
        PortContext::new(
            Uuid::new_v4().to_string(),
            rustok_api::PortActor::user(Uuid::new_v4().to_string()),
            "de",
            "same-adoption",
        )
        .with_channel("web")
        .with_causation_id("original-operation")
        .with_traceparent("original-trace")
        .with_idempotency_key("original-key")
        .with_deadline(deadline)
    }

    #[tokio::test]
    async fn selected_inventory_capability_receives_the_same_context_and_complete_request() {
        let (service, calls) = selected_owner(OwnerOutcome::Mismatch);
        let request = request();
        let mut context = adoption_context(Duration::from_secs(3));
        context.claims = vec!["inventory.write".to_string()];
        context.roles = vec!["buyer".to_string()];
        let error = service
            .request_owner_adoption(&context, &request)
            .await
            .unwrap_err();
        assert_eq!(
            error.bounded_failure(),
            Some(("inventory.adoption_response_mismatch", false))
        );
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, request);
        let delegated = &calls[0].0;
        assert_eq!(delegated.tenant_id, context.tenant_id);
        assert_eq!(delegated.actor, context.actor);
        assert_eq!(delegated.channel, context.channel);
        assert_eq!(delegated.locale, context.locale);
        assert_eq!(delegated.claims, context.claims);
        assert_eq!(delegated.roles, context.roles);
        assert_eq!(delegated.traceparent, context.traceparent);
        assert_eq!(delegated.causation_id, context.causation_id);
        assert_eq!(delegated.idempotency_key, context.idempotency_key);
        assert_eq!(delegated.deadline_ms, context.deadline_ms);
    }

    #[tokio::test]
    async fn selected_owner_advice_survives_without_an_embedded_provider_or_database_fallback() {
        for retryable in [false, true] {
            let (service, calls) = selected_owner(OwnerOutcome::Reject(retryable));
            let context = adoption_context(Duration::from_secs(60));
            let error = service
                .request_owner_adoption(&context, &request())
                .await
                .unwrap_err();
            assert_eq!(
                error.bounded_failure(),
                Some(("inventory.adoption_unavailable", retryable))
            );
            assert!(error.requires_owner_recovery());
            assert!(!error.to_string().contains("private-owner-diagnostic"));
            assert_eq!(calls.lock().unwrap()[0].0.deadline_ms, Some(30_000));
            assert_eq!(calls.lock().unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn missing_or_zero_adoption_deadline_never_invokes_the_selected_provider() {
        for deadline_ms in [None, Some(0)] {
            let (service, calls) = selected_owner(OwnerOutcome::Mismatch);
            let mut context = adoption_context(Duration::from_secs(3));
            context.deadline_ms = deadline_ms;
            let error = service
                .request_owner_adoption(&context, &request())
                .await
                .unwrap_err();
            assert_eq!(
                error.bounded_failure(),
                Some(("port.deadline_required", true))
            );
            assert!(calls.lock().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn adoption_timeout_is_an_unknown_outcome_not_permission_to_allocate_again() {
        let (service, calls) = selected_owner(OwnerOutcome::Pending);
        let request = request();
        let context = adoption_context(Duration::from_millis(1));
        let error = service
            .request_owner_adoption(&context, &request)
            .await
            .unwrap_err();
        assert_eq!(
            error.bounded_failure(),
            Some(("inventory.adoption_timeout", true))
        );
        assert!(error.requires_owner_recovery());
        assert_eq!(calls.lock().unwrap()[0].1, request);
        assert_eq!(
            calls.lock().unwrap()[0].0.idempotency_key,
            context.idempotency_key
        );
        assert_eq!(calls.lock().unwrap().len(), 1);
    }
}
