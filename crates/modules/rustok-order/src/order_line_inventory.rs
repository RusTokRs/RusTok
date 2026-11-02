//! Bounded, price-free reads of immutable Order-line inventory identities.

use std::collections::BTreeSet;

use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, QueryFilter, QueryOrder,
    QuerySelect,
};
use uuid::Uuid;

use crate::entities::{order, order_line_item};
use crate::order_read::{
    MAX_ORDER_LINE_INVENTORY_BINDINGS, OrderLineInventoryBinding,
    OrderLineInventoryBindingsSnapshot, ReadOrderLineInventoryBindingsRequest,
};
use crate::{OrderError, OrderResult};

#[derive(FromQueryResult)]
struct SavedLine {
    id: Uuid,
    variant_id: Option<Uuid>,
    quantity: i32,
}

pub(crate) async fn read_order_line_inventory_bindings<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    request: ReadOrderLineInventoryBindingsRequest,
) -> OrderResult<OrderLineInventoryBindingsSnapshot> {
    // INVARIANT: the join proves the parent Order's tenant in the same bounded query as
    // its tenant-less lines. No Product/Pricing lookup or current eligibility participates.
    let rows = order_line_item::Entity::find()
        .inner_join(order::Entity)
        .filter(order::Column::TenantId.eq(tenant_id))
        .filter(order_line_item::Column::OrderId.eq(request.order_id))
        .filter(order_line_item::Column::Id.is_in(request.order_line_item_ids.iter().copied()))
        .select_only()
        .column(order_line_item::Column::Id)
        .column(order_line_item::Column::VariantId)
        .column(order_line_item::Column::Quantity)
        .order_by_asc(order_line_item::Column::Id)
        .limit((MAX_ORDER_LINE_INVENTORY_BINDINGS + 1) as u64)
        .into_model::<SavedLine>()
        .all(db)
        .await?;

    let expected = request
        .order_line_item_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let received = rows.iter().map(|row| row.id).collect::<BTreeSet<_>>();
    if rows.len() != expected.len() || received != expected {
        // Do not distinguish a foreign tenant/Order from a missing line.
        return Err(OrderError::OrderNotFound(request.order_id));
    }
    if rows.iter().any(|row| {
        row.id.is_nil() || row.quantity <= 0 || row.variant_id.is_some_and(|id| id.is_nil())
    }) {
        return Err(OrderError::Validation(
            "saved order inventory bindings are inconsistent; operator review is required".into(),
        ));
    }
    Ok(OrderLineInventoryBindingsSnapshot {
        tenant_id,
        order_id: request.order_id,
        items: rows
            .into_iter()
            .map(|row| OrderLineInventoryBinding {
                order_line_item_id: row.id,
                variant_id: row.variant_id,
                quantity: row.quantity,
            })
            .collect(),
    })
}
