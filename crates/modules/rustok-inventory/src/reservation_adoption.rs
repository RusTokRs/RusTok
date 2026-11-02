//! Rebinding of a durable checkout allocation; no stock allocation or level movement occurs.
//! Commerce owns its separate mapping/checkpoint. An interrupted handoff must replay these same
//! identities rather than compensate, relocate or infer an outcome from a transport error.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use chrono::Utc;
use rustok_api::{PortContext, PortError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    FromQueryResult, QueryFilter, QuerySelect, Set, Statement, TransactionTrait,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::entities::{inventory_item, inventory_level, reservation_item, stock_location};
use crate::ports::{
    InventoryIdentityReservationAdoptionItem, InventoryIdentityReservationAdoptionRequest,
    InventoryIdentityReservationAdoptionSnapshot, MAX_RESERVATION_ADOPTION_ITEMS,
};
use crate::ports_impl::{
    load_inventory_item_for_update, load_tenant_variant, normalize_external_id,
    parse_port_tenant_id, storage_unavailable_with_context, validate_non_nil_uuid,
};

const ADOPT_OPERATION: &str = "adopt_inventory_by_identity";
// Mirrors the physical check in applied migration 000021; do not import or rewrite history.
const MAX_RESERVATION_METADATA_BYTES: i64 = 32 * 1024;

#[derive(FromQueryResult)]
struct AdoptionMetadataSize {
    max_metadata_bytes: i64,
}

pub(super) async fn adopt_inventory_by_identity(
    db: &DatabaseConnection,
    context: PortContext,
    mut request: InventoryIdentityReservationAdoptionRequest,
) -> Result<InventoryIdentityReservationAdoptionSnapshot, PortError> {
    let tenant_id = parse_port_tenant_id(&context, ADOPT_OPERATION)?;
    validate_request(&mut request)?;

    let txn = db
        .begin()
        .await
        .map_err(|error| storage_unavailable_with_context(&context, ADOPT_OPERATION, error))?;

    let variant_ids = request
        .items
        .iter()
        .map(|binding| binding.variant_id)
        .collect::<BTreeSet<_>>();
    for variant_id in &variant_ids {
        // Tenant proof only: do not refresh Product publication, pricing or eligibility.
        load_tenant_variant(&context, ADOPT_OPERATION, &txn, tenant_id, *variant_id).await?;
    }
    let mut locked_items = BTreeMap::new();
    for variant_id in variant_ids {
        // INVARIANT: every requested variant was tenant-scoped above. Lock only its item,
        // never an item inferred from an untrusted reservation UUID. Identity reserve/release
        // use this same item lock; sorted Variant UUIDs give deterministic batch lock order.
        let item = load_inventory_item_for_update(&context, ADOPT_OPERATION, &txn, variant_id)
            .await?
            .ok_or_else(|| {
                PortError::invariant_violation(
                    "inventory.reservation_item_missing",
                    "reservation inventory item is missing",
                )
            })?;
        locked_items.insert(variant_id, item);
    }

    let bindings = request
        .items
        .iter()
        .map(|binding| (binding.reservation_id, binding))
        .collect::<BTreeMap<_, _>>();
    let mut checked_locations = HashSet::new();
    let mut changes = Vec::new();
    let mut required_by_level = BTreeMap::<(Uuid, Uuid), i32>::new();
    for (reservation_id, binding) in bindings {
        let item = locked_items
            .get(&binding.variant_id)
            .ok_or_else(identity_conflict)?;
        // INVARIANT: item belongs to the tenant-proved Variant above. A foreign or mismatched
        // reservation cannot be read or locked merely by guessing its globally unique UUID.
        let query = reservation_item::Entity::find_by_id(reservation_id)
            .filter(reservation_item::Column::InventoryItemId.eq(item.id));
        let query = if txn.get_database_backend() == DbBackend::Sqlite {
            query
        } else {
            query.lock_exclusive()
        };
        let reservation = query
            .one(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, ADOPT_OPERATION, error))?
            .ok_or_else(reservation_not_found)?;
        validate_saved_reservation(&request, binding, &reservation, item)?;
        if checked_locations.insert(reservation.location_id) {
            // A saved allocation may now be hidden/archived, but never foreign-tenant.
            stock_location::Entity::find_by_id(reservation.location_id)
                .filter(stock_location::Column::TenantId.eq(tenant_id))
                .one(&txn)
                .await
                .map_err(|error| {
                    storage_unavailable_with_context(&context, ADOPT_OPERATION, error)
                })?
                .ok_or_else(identity_conflict)?;
        }
        let required = required_by_level
            .entry((item.id, reservation.location_id))
            .or_default();
        *required = required
            .checked_add(reservation.quantity)
            .ok_or_else(ledger_inconsistent)?;
        if reservation.line_item_id != Some(binding.order_line_item_id) {
            let metadata = adoption_metadata(
                &reservation.metadata,
                request.order_id,
                binding.order_line_item_id,
            )?;
            changes.push((reservation, binding.order_line_item_id, metadata));
        }
    }

    // Existing reserved counters must still cover this batch at its original locations. This
    // is a ledger consistency check, not a fresh availability/backorder/stock selection. Do not
    // require on-hand stock: an existing backorder allocation is still a valid reservation.
    for ((item_id, location_id), required) in required_by_level {
        // INVARIANT: item IDs came only from tenant-proved, locked Variant inventory above.
        let query = inventory_level::Entity::find()
            .filter(inventory_level::Column::InventoryItemId.eq(item_id))
            .filter(inventory_level::Column::LocationId.eq(location_id))
            .limit(2);
        let query = if txn.get_database_backend() == DbBackend::Sqlite {
            query
        } else {
            query.lock_exclusive()
        };
        let levels = query
            .all(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, ADOPT_OPERATION, error))?;
        let [level] = levels.as_slice() else {
            return Err(ledger_inconsistent());
        };
        if level.reserved_quantity < required {
            return Err(ledger_inconsistent());
        }
    }

    // Size is a deterministic capacity refusal, not a transient update/DB failure. Check all
    // prepared rows using the physical backend representation before the first binding write.
    validate_adoption_metadata_capacity(&context, &txn, &changes).await?;

    // No mutation starts until every row has passed validation while locked. Replay of an
    // already-adopted batch writes nothing, including description/metadata/updated_at.
    for (reservation, order_line_item_id, metadata) in changes {
        let mut active: reservation_item::ActiveModel = reservation.into();
        active.line_item_id = Set(Some(order_line_item_id));
        active.description = Set(Some("Checkout order inventory reservation".to_string()));
        active.metadata = Set(metadata);
        active.updated_at = Set(Utc::now().into());
        active
            .update(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, ADOPT_OPERATION, error))?;
    }
    txn.commit()
        .await
        .map_err(|error| storage_unavailable_with_context(&context, ADOPT_OPERATION, error))?;

    Ok(InventoryIdentityReservationAdoptionSnapshot {
        checkout_operation_id: request.checkout_operation_id,
        cart_id: request.cart_id,
        order_id: request.order_id,
        items: request.items,
    })
}

fn adoption_metadata(
    saved: &Value,
    order_id: Uuid,
    order_line_item_id: Uuid,
) -> Result<Value, PortError> {
    let mut metadata = saved.as_object().cloned().ok_or_else(identity_conflict)?;
    metadata.insert("order_id".to_string(), json!(order_id));
    metadata.insert("order_line_item_id".to_string(), json!(order_line_item_id));
    Ok(Value::Object(metadata))
}

async fn validate_adoption_metadata_capacity(
    context: &PortContext,
    txn: &impl ConnectionTrait,
    changes: &[(reservation_item::Model, Uuid, Value)],
) -> Result<(), PortError> {
    // Exact replay must not resize, compact or write the saved payload.
    if changes.is_empty() {
        return Ok(());
    }
    let backend = txn.get_database_backend();
    let sql = match backend {
        DbBackend::Postgres => {
            "SELECT COALESCE(MAX(octet_length(value::text)), 0)::bigint AS max_metadata_bytes \
             FROM jsonb_array_elements($1::jsonb)"
        }
        DbBackend::Sqlite => {
            "SELECT COALESCE(MAX(length(CAST(value AS BLOB))), 0) AS max_metadata_bytes \
             FROM json_each(?)"
        }
        _ => {
            return Err(PortError::validation(
                "inventory.adoption_backend_unsupported",
                "reservation adoption metadata validation requires PostgreSQL or SQLite",
            ));
        }
    };
    // One parameter and one aggregate for at most 1024 changed rows, not per-row probes.
    // Serialize references so measuring does not duplicate every saved JSON tree.
    let metadata = serde_json::to_string(
        &changes
            .iter()
            .map(|(_, _, metadata)| metadata)
            .collect::<Vec<_>>(),
    )
    .map_err(|_| metadata_size_unverified())?;
    let size = AdoptionMetadataSize::find_by_statement(Statement::from_sql_and_values(
        backend,
        sql,
        vec![metadata.into()],
    ))
    .one(txn)
    .await
    .map_err(|error| storage_unavailable_with_context(context, ADOPT_OPERATION, error))?
    .ok_or_else(metadata_size_unverified)?;
    validate_adoption_metadata_bytes(size.max_metadata_bytes)
}

fn validate_adoption_metadata_bytes(bytes: i64) -> Result<(), PortError> {
    // This aggregate runs only for changed objects with both nonempty target UUID fields.
    // Zero means no row was measured, not proof that an empty payload fits.
    if bytes <= 0 {
        return Err(metadata_size_unverified());
    }
    if bytes > MAX_RESERVATION_METADATA_BYTES {
        return Err(PortError::conflict(
            "inventory.adoption_metadata_too_large",
            "saved metadata has no capacity for order adoption; explicit reconciliation is required",
        ));
    }
    Ok(())
}

fn metadata_size_unverified() -> PortError {
    PortError::invariant_violation(
        "inventory.adoption_metadata_size_unverified",
        "reservation adoption metadata size could not be verified",
    )
}

fn validate_request(
    request: &mut InventoryIdentityReservationAdoptionRequest,
) -> Result<(), PortError> {
    validate_non_nil_uuid(request.checkout_operation_id, "checkout_operation_id")?;
    validate_non_nil_uuid(request.cart_id, "cart_id")?;
    validate_non_nil_uuid(request.order_id, "order_id")?;
    if request.items.is_empty() || request.items.len() > MAX_RESERVATION_ADOPTION_ITEMS {
        return Err(PortError::validation(
            "inventory.adoption_batch_invalid",
            "reservation adoption requires 1 to 1024 bindings",
        ));
    }
    let mut reservation_ids = HashSet::new();
    let mut external_ids = HashSet::new();
    let mut cart_line_ids = HashSet::new();
    let mut order_line_ids = HashSet::new();
    for binding in &mut request.items {
        for (id, name) in [
            (binding.reservation_id, "reservation_id"),
            (binding.variant_id, "variant_id"),
            (binding.location_id, "location_id"),
            (binding.cart_line_item_id, "cart_line_item_id"),
            (binding.order_line_item_id, "order_line_item_id"),
        ] {
            validate_non_nil_uuid(id, name)?;
        }
        binding.external_id = normalize_external_id(std::mem::take(&mut binding.external_id))?;
        if binding.quantity <= 0
            || binding.cart_line_item_id == binding.order_line_item_id
            || !reservation_ids.insert(binding.reservation_id)
            || !external_ids.insert(binding.external_id.clone())
            || !cart_line_ids.insert(binding.cart_line_item_id)
            || !order_line_ids.insert(binding.order_line_item_id)
        {
            return Err(PortError::validation(
                "inventory.adoption_binding_invalid",
                "reservation adoption bindings must have positive quantities and unique identities",
            ));
        }
    }
    // Cross-kind overlap would make a Cart-bound row indistinguishable from an Order-bound row.
    if !cart_line_ids.is_disjoint(&order_line_ids) {
        return Err(PortError::validation(
            "inventory.adoption_binding_invalid",
            "cart and order line identities must not overlap",
        ));
    }
    Ok(())
}

fn validate_saved_reservation(
    request: &InventoryIdentityReservationAdoptionRequest,
    binding: &InventoryIdentityReservationAdoptionItem,
    reservation: &reservation_item::Model,
    item: &inventory_item::Model,
) -> Result<(), PortError> {
    if reservation.id != binding.reservation_id
        || reservation.inventory_item_id != item.id
        || item.variant_id != binding.variant_id
        || reservation.external_id.as_deref() != Some(binding.external_id.as_str())
        || reservation.location_id != binding.location_id
        || reservation.deleted_at.is_some()
        || reservation.quantity != binding.quantity
        || !reservation.metadata.is_object()
        || has_terminal_disposition(&reservation.metadata)
        || metadata_uuid(&reservation.metadata, "reservation_id") != Some(binding.reservation_id)
        || metadata_uuid(&reservation.metadata, "variant_id") != Some(binding.variant_id)
        || reservation
            .metadata
            .get("external_id")
            .and_then(Value::as_str)
            != Some(binding.external_id.as_str())
        || !checkout_provenance_matches(
            &reservation.metadata,
            "checkout_operation_id",
            request.checkout_operation_id,
        )
        || !checkout_provenance_matches(&reservation.metadata, "cart_id", request.cart_id)
        || metadata_uuid(&reservation.metadata, "cart_line_item_id")
            != Some(binding.cart_line_item_id)
    {
        return Err(identity_conflict());
    }
    let source = reservation.metadata.get("source").and_then(Value::as_str);
    if reservation.line_item_id == Some(binding.order_line_item_id) {
        // The previous Commerce adoption wrote checkout_operation; current owner adoption
        // retains inventory_identity_port. Neither spelling authorizes changing a binding.
        if !matches!(
            source,
            Some("inventory_identity_port" | "checkout_operation")
        ) || metadata_uuid(&reservation.metadata, "order_id") != Some(request.order_id)
            || metadata_uuid(&reservation.metadata, "order_line_item_id")
                != Some(binding.order_line_item_id)
        {
            return Err(identity_conflict());
        }
    } else if source != Some("inventory_identity_port")
        || reservation.line_item_id != Some(binding.cart_line_item_id)
        || reservation
            .metadata
            .get("order_id")
            .is_some_and(|value| !value.is_null())
        || reservation
            .metadata
            .get("order_line_item_id")
            .is_some_and(|value| !value.is_null())
    {
        return Err(identity_conflict());
    }
    Ok(())
}

pub(super) fn has_terminal_disposition(metadata: &Value) -> bool {
    // The current release owner and historical disposal/supersession migrations write only
    // terminal facts here. An apparently restored positive row must not erase that history.
    [
        "inventory_disposition",
        "superseded_by",
        "released_at",
        "disposed_at",
        "superseded_at",
    ]
    .into_iter()
    .any(|key| metadata.get(key).is_some_and(|value| !value.is_null()))
}

/// The applied oversize-metadata compaction migration preserves immutable reservation,
/// external, Variant and Cart-line identities but drops Cart/operation labels. Only its
/// explicit marker permits absence; contradictory retained fields still refuse. Commerce
/// owns and validates these labels in its immutable mapping journal. Never backfill history.
fn checkout_provenance_matches(metadata: &Value, key: &str, expected: Uuid) -> bool {
    match metadata.get(key) {
        None | Some(Value::Null) => {
            metadata.get("metadata_truncated").and_then(Value::as_bool) == Some(true)
        }
        Some(_) => metadata_uuid(metadata, key) == Some(expected),
    }
}

fn metadata_uuid(metadata: &Value, key: &str) -> Option<Uuid> {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn identity_conflict() -> PortError {
    PortError::conflict(
        "inventory.adoption_identity_conflict",
        "saved reservation does not match the checkout order adoption identity",
    )
}

fn reservation_not_found() -> PortError {
    PortError::not_found(
        "inventory.reservation_not_found",
        "inventory reservation was not found",
    )
}

fn ledger_inconsistent() -> PortError {
    PortError::invariant_violation(
        "inventory.adoption_ledger_inconsistent",
        "saved reservation allocation does not match its reserved inventory level",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(count: usize) -> InventoryIdentityReservationAdoptionRequest {
        InventoryIdentityReservationAdoptionRequest {
            checkout_operation_id: Uuid::new_v4(),
            cart_id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            items: (0..count)
                .map(|_| InventoryIdentityReservationAdoptionItem {
                    reservation_id: Uuid::new_v4(),
                    external_id: format!("checkout-reservation:{}", Uuid::new_v4()),
                    variant_id: Uuid::new_v4(),
                    quantity: 3,
                    location_id: Uuid::new_v4(),
                    cart_line_item_id: Uuid::new_v4(),
                    order_line_item_id: Uuid::new_v4(),
                })
                .collect(),
        }
    }

    fn saved(
        request: &InventoryIdentityReservationAdoptionRequest,
    ) -> (inventory_item::Model, reservation_item::Model) {
        let binding = &request.items[0];
        let now = Utc::now().fixed_offset();
        let item = inventory_item::Model {
            id: Uuid::new_v4(),
            variant_id: binding.variant_id,
            sku: Some("saved-sku".to_string()),
            requires_shipping: true,
            metadata: json!({}),
            created_at: now,
            updated_at: now,
        };
        let reservation = reservation_item::Model {
            id: binding.reservation_id,
            inventory_item_id: item.id,
            location_id: binding.location_id,
            quantity: binding.quantity,
            line_item_id: Some(binding.cart_line_item_id),
            description: Some("original reservation".to_string()),
            external_id: Some(binding.external_id.clone()),
            metadata: json!({
                "source": "inventory_identity_port",
                "reservation_id": binding.reservation_id,
                "external_id": binding.external_id,
                "variant_id": binding.variant_id,
                "checkout_operation_id": request.checkout_operation_id,
                "cart_id": request.cart_id,
                "cart_line_item_id": binding.cart_line_item_id,
                "snapshot_hash": "frozen-snapshot-hash",
                "allocated_channel_slug": "web",
            }),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        (item, reservation)
    }

    #[test]
    fn adoption_metadata_preserves_all_saved_payload_and_only_adds_target_binding() {
        let request = request(1);
        let (_, reservation) = saved(&request);
        let mut original = reservation.metadata;
        original["custom"] = json!({"nested": ["saved", null, 17], "unicode": "\u{1f642}"});
        let unchanged = original.clone();
        let metadata = adoption_metadata(
            &original,
            request.order_id,
            request.items[0].order_line_item_id,
        )
        .unwrap();
        for (key, value) in original.as_object().unwrap() {
            assert_eq!(metadata.get(key), Some(value));
        }
        assert_eq!(metadata["order_id"], json!(request.order_id));
        assert_eq!(
            metadata["order_line_item_id"],
            json!(request.items[0].order_line_item_id)
        );
        assert_eq!(original, unchanged);
    }

    #[test]
    fn adoption_metadata_size_refusal_is_bounded_nonretryable_and_not_truncation() {
        validate_adoption_metadata_bytes(MAX_RESERVATION_METADATA_BYTES - 1).unwrap();
        validate_adoption_metadata_bytes(MAX_RESERVATION_METADATA_BYTES).unwrap();
        let error =
            validate_adoption_metadata_bytes(MAX_RESERVATION_METADATA_BYTES + 1).unwrap_err();
        assert_eq!(error.code, "inventory.adoption_metadata_too_large");
        assert_eq!(error.kind, rustok_api::PortErrorKind::Conflict);
        assert!(!error.retryable);
        for unverified in [-1, 0] {
            assert_eq!(
                validate_adoption_metadata_bytes(unverified)
                    .unwrap_err()
                    .code,
                "inventory.adoption_metadata_size_unverified"
            );
        }
    }

    #[test]
    fn adoption_request_rejects_empty_and_oversized_batches() {
        assert!(validate_request(&mut request(0)).is_err());
        assert!(validate_request(&mut request(MAX_RESERVATION_ADOPTION_ITEMS)).is_ok());
        let error = validate_request(&mut request(MAX_RESERVATION_ADOPTION_ITEMS + 1)).unwrap_err();
        assert_eq!(error.code, "inventory.adoption_batch_invalid");
    }

    #[test]
    fn adoption_request_rejects_nil_nonpositive_and_duplicate_identities() {
        for field in 0..8 {
            let mut candidate = request(1);
            match field {
                0 => candidate.checkout_operation_id = Uuid::nil(),
                1 => candidate.cart_id = Uuid::nil(),
                2 => candidate.order_id = Uuid::nil(),
                3 => candidate.items[0].reservation_id = Uuid::nil(),
                4 => candidate.items[0].variant_id = Uuid::nil(),
                5 => candidate.items[0].location_id = Uuid::nil(),
                6 => candidate.items[0].cart_line_item_id = Uuid::nil(),
                7 => candidate.items[0].order_line_item_id = Uuid::nil(),
                _ => unreachable!(),
            }
            assert!(validate_request(&mut candidate).is_err());
        }
        for quantity in [0, -1, i32::MIN] {
            let mut candidate = request(1);
            candidate.items[0].quantity = quantity;
            assert!(validate_request(&mut candidate).is_err());
        }
        for field in 0..5 {
            let mut candidate = request(2);
            match field {
                0 => candidate.items[1].reservation_id = candidate.items[0].reservation_id,
                1 => candidate.items[1].external_id = candidate.items[0].external_id.clone(),
                2 => candidate.items[1].cart_line_item_id = candidate.items[0].cart_line_item_id,
                3 => candidate.items[1].order_line_item_id = candidate.items[0].order_line_item_id,
                4 => candidate.items[1].order_line_item_id = candidate.items[0].cart_line_item_id,
                _ => unreachable!(),
            }
            assert!(validate_request(&mut candidate).is_err());
        }
    }

    #[test]
    fn adoption_external_identity_retains_canonical_character_bounds() {
        let mut candidate = request(1);
        candidate.items[0].external_id = format!("  {}  ", "é".repeat(191));
        validate_request(&mut candidate).unwrap();
        assert_eq!(candidate.items[0].external_id, "é".repeat(191));
        for external_id in [" ".to_string(), "é".repeat(192)] {
            candidate.items[0].external_id = external_id;
            let error = validate_request(&mut candidate).unwrap_err();
            assert_eq!(error.code, "inventory.reservation_external_id_invalid");
        }
    }

    #[test]
    fn adoption_saved_binding_requires_exact_cart_provenance_and_allocation() {
        let request = request(1);
        let binding = &request.items[0];
        let (item, original) = saved(&request);
        validate_saved_reservation(&request, binding, &original, &item).unwrap();
        for field in 0..11 {
            let mut candidate = original.clone();
            match field {
                0 => candidate.id = Uuid::new_v4(),
                1 => candidate.inventory_item_id = Uuid::new_v4(),
                2 => candidate.location_id = Uuid::new_v4(),
                3 => candidate.external_id = Some("another-identity".to_string()),
                4 => candidate.quantity -= 1,
                5 => candidate.deleted_at = Some(Utc::now().fixed_offset()),
                6 => candidate.line_item_id = Some(Uuid::new_v4()),
                7 => candidate.metadata["checkout_operation_id"] = json!(Uuid::new_v4()),
                8 => candidate.metadata["cart_id"] = json!(Uuid::new_v4()),
                9 => candidate.metadata["cart_line_item_id"] = json!(Uuid::new_v4()),
                10 => candidate.metadata = Value::Null,
                _ => unreachable!(),
            }
            let error =
                validate_saved_reservation(&request, binding, &candidate, &item).unwrap_err();
            assert_eq!(error.code, "inventory.adoption_identity_conflict");
        }
        let mut another_variant = item.clone();
        another_variant.variant_id = Uuid::new_v4();
        assert!(
            validate_saved_reservation(&request, binding, &original, &another_variant).is_err()
        );
    }

    #[test]
    fn adoption_replay_requires_the_same_order_and_target_line() {
        let request = request(1);
        let binding = &request.items[0];
        let (item, mut reservation) = saved(&request);
        reservation.line_item_id = Some(binding.order_line_item_id);
        reservation.metadata["order_id"] = json!(request.order_id);
        reservation.metadata["order_line_item_id"] = json!(binding.order_line_item_id);
        validate_saved_reservation(&request, binding, &reservation, &item).unwrap();
        let unchanged = reservation.clone();
        let mut another_order = request.clone();
        another_order.order_id = Uuid::new_v4();
        assert!(validate_saved_reservation(&another_order, binding, &reservation, &item).is_err());
        let mut another_line = binding.clone();
        another_line.order_line_item_id = Uuid::new_v4();
        assert!(validate_saved_reservation(&request, &another_line, &reservation, &item).is_err());
        assert_eq!(reservation, unchanged);
    }

    #[test]
    fn adoption_never_repairs_contradictory_historical_order_metadata() {
        let request = request(1);
        let binding = &request.items[0];
        let (item, mut reservation) = saved(&request);
        reservation.metadata["order_id"] = json!(request.order_id);
        assert!(validate_saved_reservation(&request, binding, &reservation, &item).is_err());
        reservation.line_item_id = Some(binding.order_line_item_id);
        reservation.metadata["order_line_item_id"] = json!(binding.order_line_item_id);
        reservation.metadata["cart_line_item_id"] = json!("invalid-source-uuid");
        assert!(validate_saved_reservation(&request, binding, &reservation, &item).is_err());
    }

    #[test]
    fn terminal_metadata_cannot_be_resurrected_by_positive_quantity_or_cleared_during_adoption() {
        let request = request(1);
        let binding = &request.items[0];
        let (item, reservation) = saved(&request);
        for (key, value) in [
            ("inventory_disposition", json!("released")),
            ("inventory_disposition", json!("committed")),
            ("inventory_disposition", json!("superseded")),
            ("inventory_disposition", json!("unrecognized")),
            ("superseded_by", json!("order_confirmation_reservation")),
            ("released_at", json!("retained-release-time")),
            ("disposed_at", json!("retained-disposal-time")),
            ("superseded_at", json!("retained-supersession-time")),
        ] {
            let mut contradictory = reservation.clone();
            contradictory.metadata[key] = value;
            let unchanged = contradictory.clone();
            assert_eq!(
                validate_saved_reservation(&request, binding, &contradictory, &item)
                    .unwrap_err()
                    .code,
                "inventory.adoption_identity_conflict"
            );
            assert_eq!(contradictory, unchanged);
            contradictory.metadata[key] = Value::Null;
            validate_saved_reservation(&request, binding, &contradictory, &item).unwrap();
        }
    }

    #[test]
    fn known_compacted_provenance_and_legacy_adoption_remain_explicit_not_rewritten() {
        let request = request(1);
        let binding = &request.items[0];
        let (item, mut reservation) = saved(&request);
        reservation
            .metadata
            .as_object_mut()
            .unwrap()
            .remove("checkout_operation_id");
        reservation
            .metadata
            .as_object_mut()
            .unwrap()
            .remove("cart_id");
        // The existing compaction migration drops precisely these labels while retaining the
        // owner/reservation/external/Variant/Cart-line identities; Commerce owns its journal.
        reservation.metadata["metadata_truncated"] = json!(true);
        validate_saved_reservation(&request, binding, &reservation, &item).unwrap();
        assert!(reservation.metadata.get("cart_id").is_none());
        reservation.metadata["metadata_truncated"] = json!(false);
        assert!(validate_saved_reservation(&request, binding, &reservation, &item).is_err());
        reservation.metadata["metadata_truncated"] = json!(true);
        reservation.metadata["cart_id"] = json!(Uuid::new_v4());
        assert!(validate_saved_reservation(&request, binding, &reservation, &item).is_err());
        reservation
            .metadata
            .as_object_mut()
            .unwrap()
            .remove("cart_id");
        reservation.line_item_id = Some(binding.order_line_item_id);
        reservation.metadata["source"] = json!("checkout_operation");
        reservation.metadata["order_id"] = json!(request.order_id);
        reservation.metadata["order_line_item_id"] = json!(binding.order_line_item_id);
        let historical = reservation.clone();
        validate_saved_reservation(&request, binding, &reservation, &item).unwrap();
        assert_eq!(reservation, historical);
    }
}
