use chrono::Utc;
use rustok_commerce_foundation::entities::{inventory_item, reservation_item};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, EntityTrait, Set, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::checkout_inventory_reservation;

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
}

pub type CheckoutInventoryOrderAdoptionResult<T> = Result<T, CheckoutInventoryOrderAdoptionError>;

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
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            reservation_journal: CheckoutInventoryReservationJournal::new(db.clone()),
            operation_journal: CheckoutOperationJournal::new(db.clone()),
            db,
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
        }
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    /// Rebinds inventory owner rows from immutable cart-line identity to the
    /// corresponding pending order line, then checkpoints `order_created`.
    ///
    /// A retry accepts rows already bound to the same order line, but fails
    /// closed when reservation identity, tenant, quantity, variant or source
    /// provenance differs.
    pub async fn adopt_and_checkpoint(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        order: &rustok_order::OrderResponse,
    ) -> CheckoutInventoryOrderAdoptionResult<Vec<CheckoutInventoryOrderAdoption>> {
        let lease_owner = lease_owner.into();
        let operation = self.operation_journal.get(tenant_id, operation_id).await?;
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
                .await;
        }

        if operation.stage != CheckoutOperationStage::InventoryReserved.as_str() {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                "checkout operation {} cannot adopt order inventory from stage `{}`",
                operation.id, operation.stage
            )));
        }
        if order.status != "pending" {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                "order {} must remain pending while checkout reservations are adopted, not `{}`",
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
        self.adopt_rows(tenant_id, operation_id, order.id, &bindings, &mappings)
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
            .await?;

        self.validate_adopted_rows(tenant_id, operation_id, order)
            .await
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
        tenant_id: Uuid,
        operation_id: Uuid,
        order_id: Uuid,
        bindings: &[OrderLineBinding],
        mappings: &[checkout_inventory_reservation::Model],
    ) -> CheckoutInventoryOrderAdoptionResult<()> {
        let binding_by_cart_line = bindings
            .iter()
            .map(|binding| (binding.cart_line_item_id, *binding))
            .collect::<HashMap<_, _>>();
        let txn = self.db.begin().await?;

        for mapping in mappings {
            let binding = binding_by_cart_line
                .get(&mapping.cart_line_item_id)
                .expect("reservation set was validated before transaction");
            validate_mapping(mapping, tenant_id, operation_id, binding)?;

            if let Some(existing_order_line_id) =
                load_order_line_adoption(&txn, tenant_id, mapping.reservation_id).await?
            {
                if existing_order_line_id != binding.order_line_item_id {
                    return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                        "reservation {} is already adopted by another order line",
                        mapping.reservation_id
                    )));
                }
            }

            let reservation = reservation_item::Entity::find_by_id(mapping.reservation_id)
                .one(&txn)
                .await?
                .ok_or_else(|| {
                    CheckoutInventoryOrderAdoptionError::Conflict(format!(
                        "inventory reservation {} is missing",
                        mapping.reservation_id
                    ))
                })?;
            if reservation.deleted_at.is_some() || reservation.quantity != mapping.quantity {
                return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                    "inventory reservation {} is inactive or has a different quantity",
                    mapping.reservation_id
                )));
            }
            if !matches!(
                reservation.line_item_id,
                Some(line_item_id)
                    if line_item_id == mapping.cart_line_item_id
                        || line_item_id == binding.order_line_item_id
            ) {
                return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                    "inventory reservation {} is already bound to another line item",
                    mapping.reservation_id
                )));
            }

            let inventory = inventory_item::Entity::find_by_id(reservation.inventory_item_id)
                .one(&txn)
                .await?
                .ok_or_else(|| {
                    CheckoutInventoryOrderAdoptionError::Conflict(format!(
                        "inventory item {} for reservation {} is missing",
                        reservation.inventory_item_id, mapping.reservation_id
                    ))
                })?;
            if inventory.variant_id != mapping.variant_id {
                return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                    "inventory reservation {} resolves to another variant",
                    mapping.reservation_id
                )));
            }

            if reservation.line_item_id != Some(binding.order_line_item_id) {
                let reservation_metadata = merge_metadata(
                    reservation.metadata.clone(),
                    json!({
                        "source": "checkout_operation",
                        "checkout_operation_id": operation_id,
                        "order_id": order_id,
                        "order_line_item_id": binding.order_line_item_id,
                        "cart_line_item_id": binding.cart_line_item_id,
                    }),
                );
                let mut reservation_active: reservation_item::ActiveModel = reservation.into();
                reservation_active.line_item_id = Set(Some(binding.order_line_item_id));
                reservation_active.description =
                    Set(Some("Checkout order inventory reservation".to_string()));
                reservation_active.metadata = Set(reservation_metadata);
                reservation_active.updated_at = Set(Utc::now().into());
                reservation_active.update(&txn).await?;
            }

            let result = txn
                .execute(Statement::from_sql_and_values(
                    txn.get_database_backend(),
                    r#"
                    UPDATE checkout_inventory_reservations
                    SET order_line_item_id = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE reservation_id = ?
                      AND tenant_id = ?
                      AND checkout_operation_id = ?
                      AND status = 'reserved'
                      AND (order_line_item_id IS NULL OR order_line_item_id = ?)
                    "#,
                    vec![
                        binding.order_line_item_id.into(),
                        mapping.reservation_id.into(),
                        tenant_id.into(),
                        operation_id.into(),
                        binding.order_line_item_id.into(),
                    ],
                ))
                .await?;
            if result.rows_affected() != 1
                && load_order_line_adoption(&txn, tenant_id, mapping.reservation_id).await?
                    != Some(binding.order_line_item_id)
            {
                return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                    "reservation {} adoption lost a compare-and-set race",
                    mapping.reservation_id
                )));
            }
        }

        txn.commit().await?;
        Ok(())
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

async fn load_order_line_adoption<C>(
    conn: &C,
    tenant_id: Uuid,
    reservation_id: Uuid,
) -> Result<Option<Uuid>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    conn.query_one(Statement::from_sql_and_values(
        conn.get_database_backend(),
        r#"
        SELECT order_line_item_id
        FROM checkout_inventory_reservations
        WHERE tenant_id = ? AND reservation_id = ?
        "#,
        vec![tenant_id.into(), reservation_id.into()],
    ))
    .await
    .map(|row| row.and_then(|row| row.try_get("", "order_line_item_id").ok()))
}

async fn load_operation_adoptions<C>(
    conn: &C,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> Result<Vec<CheckoutInventoryOrderAdoption>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let rows = conn
        .query_all(Statement::from_sql_and_values(
            conn.get_database_backend(),
            r#"
            SELECT reservation_id, cart_line_item_id, order_line_item_id
            FROM checkout_inventory_reservations
            WHERE tenant_id = ?
              AND checkout_operation_id = ?
              AND order_line_item_id IS NOT NULL
            ORDER BY cart_line_item_id
            "#,
            vec![tenant_id.into(), operation_id.into()],
        ))
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(CheckoutInventoryOrderAdoption {
                reservation_id: row.try_get("", "reservation_id")?,
                cart_line_item_id: row.try_get("", "cart_line_item_id")?,
                order_line_item_id: row.try_get("", "order_line_item_id")?,
            })
        })
        .collect()
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
    for line in &order.line_items {
        let Some(variant_id) = line.variant_id else {
            continue;
        };
        if line.quantity <= 0 {
            return Err(CheckoutInventoryOrderAdoptionError::Conflict(format!(
                "order line {} has a non-positive quantity",
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
        if !cart_line_ids.insert(cart_line_item_id) {
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

fn merge_metadata(base: Value, patch: Value) -> Value {
    match (base, patch) {
        (Value::Object(mut base), Value::Object(patch)) => {
            for (key, value) in patch {
                base.insert(key, value);
            }
            Value::Object(base)
        }
        (_, patch) => patch,
    }
}
