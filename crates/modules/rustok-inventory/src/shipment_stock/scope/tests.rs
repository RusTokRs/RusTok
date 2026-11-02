use super::*;

fn fixture() -> (Uuid, ShipmentStockBatch, proof::Model) {
    let tenant = Uuid::new_v4();
    let batch = ShipmentStockBatch {
        order_id: Uuid::new_v4(),
        fulfillment_id: Uuid::new_v4(),
        lines: vec![ShipmentStockLine {
            fulfillment_item_id: Uuid::new_v4(),
            order_line_item_id: Uuid::new_v4(),
            variant_id: Some(Uuid::new_v4()),
            fulfillment_quantity: 5,
            shipped_quantity: 3,
            delivered_quantity: 1,
            quantity: 2,
        }],
    };
    let line = batch.lines[0];
    let now = Utc::now().fixed_offset();
    let proof = proof::Model {
        tenant_id: tenant,
        fulfillment_item_id: line.fulfillment_item_id,
        order_id: batch.order_id,
        fulfillment_id: batch.fulfillment_id,
        order_line_item_id: line.order_line_item_id,
        variant_id: line.variant_id,
        fulfillment_quantity: line.fulfillment_quantity,
        shipped_quantity: line.shipped_quantity,
        restored_quantity: 0,
        inventory_item_id: Some(Uuid::new_v4()),
        reservation_id: Some(Uuid::new_v4()),
        location_id: Some(Uuid::new_v4()),
        created_at: now,
        updated_at: now,
    };
    (tenant, batch, proof)
}

#[test]
fn native_partial_restore_reconsume_requires_the_exact_outstanding_projection() {
    let (tenant, batch, mut proof) = fixture();
    let line = &batch.lines[0];
    assert_eq!(
        next_progress(
            tenant,
            &batch,
            line,
            Some(&proof),
            ShipmentStockAction::Restore
        )
        .expect("exact restore"),
        (3, 2)
    );
    assert!(
        next_progress(
            tenant,
            &batch,
            line,
            Some(&proof),
            ShipmentStockAction::Reconsume
        )
        .is_err(),
        "cancellation status alone is not a restore receipt"
    );
    proof.restored_quantity = 1;
    assert!(
        next_progress(
            tenant,
            &batch,
            line,
            Some(&proof),
            ShipmentStockAction::Reconsume
        )
        .is_err()
    );
    proof.restored_quantity = 2;
    assert_eq!(
        next_progress(
            tenant,
            &batch,
            line,
            Some(&proof),
            ShipmentStockAction::Reconsume
        )
        .expect("exact reconsumption"),
        (3, 0)
    );
    assert!(
        next_progress(
            tenant,
            &batch,
            line,
            Some(&proof),
            ShipmentStockAction::Restore
        )
        .is_err(),
        "no duplicate credit"
    );
    assert!(
        next_progress(
            tenant,
            &batch,
            line,
            Some(&proof),
            ShipmentStockAction::Ship
        )
        .is_err(),
        "new shipping cannot bypass outstanding restoration"
    );
}

#[test]
fn native_missing_history_never_adopts_nonzero_shipped_counters() {
    let (tenant, batch, _) = fixture();
    for action in [
        ShipmentStockAction::Ship,
        ShipmentStockAction::Restore,
        ShipmentStockAction::Reconsume,
    ] {
        assert!(next_progress(tenant, &batch, &batch.lines[0], None, action).is_err());
    }
    let mut fresh = batch.clone();
    fresh.lines[0].shipped_quantity = 0;
    fresh.lines[0].delivered_quantity = 0;
    assert_eq!(
        next_progress(
            tenant,
            &fresh,
            &fresh.lines[0],
            None,
            ShipmentStockAction::Ship
        )
        .expect("new native shipment"),
        (2, 0)
    );
}

#[test]
fn native_saved_proof_cannot_be_rebound_across_any_owner_identity_or_quantity() {
    let (tenant, batch, proof) = fixture();
    let mut bad = Vec::new();
    let mut row = proof.clone();
    row.tenant_id = Uuid::new_v4();
    bad.push(row);
    let mut row = proof.clone();
    row.order_id = Uuid::new_v4();
    bad.push(row);
    let mut row = proof.clone();
    row.fulfillment_id = Uuid::new_v4();
    bad.push(row);
    let mut row = proof.clone();
    row.fulfillment_item_id = Uuid::new_v4();
    bad.push(row);
    let mut row = proof.clone();
    row.order_line_item_id = Uuid::new_v4();
    bad.push(row);
    let mut row = proof.clone();
    row.variant_id = Some(Uuid::new_v4());
    bad.push(row);
    let mut row = proof.clone();
    row.variant_id = None;
    bad.push(row);
    let mut row = proof.clone();
    row.fulfillment_quantity = 4;
    bad.push(row);
    let mut row = proof.clone();
    row.shipped_quantity = 2;
    bad.push(row);
    let mut row = proof.clone();
    row.restored_quantity = -1;
    bad.push(row);
    let mut row = proof.clone();
    row.restored_quantity = 4;
    bad.push(row);
    let mut row = proof.clone();
    row.inventory_item_id = None;
    bad.push(row);
    let mut row = proof.clone();
    row.reservation_id = Some(Uuid::nil());
    bad.push(row);
    let mut row = proof.clone();
    row.location_id = None;
    bad.push(row);
    for row in bad {
        assert!(
            next_progress(
                tenant,
                &batch,
                &batch.lines[0],
                Some(&row),
                ShipmentStockAction::Restore
            )
            .is_err()
        );
    }
}

#[test]
fn native_batch_rejects_bad_bounds_duplicates_and_counter_snapshots() {
    let (_, batch, _) = fixture();
    validate_batch(&batch).expect("valid batch");
    let mut bad = Vec::new();
    let mut row = batch.clone();
    row.order_id = Uuid::nil();
    bad.push(row);
    let mut row = batch.clone();
    row.fulfillment_id = Uuid::nil();
    bad.push(row);
    let mut row = batch.clone();
    row.lines.clear();
    bad.push(row);
    let mut row = batch.clone();
    row.lines.push(row.lines[0]);
    bad.push(row);
    let mut row = batch.clone();
    row.lines[0].variant_id = Some(Uuid::nil());
    bad.push(row);
    let mut row = batch.clone();
    row.lines[0].quantity = 0;
    bad.push(row);
    let mut row = batch.clone();
    row.lines[0].quantity = -1;
    bad.push(row);
    let mut row = batch.clone();
    row.lines[0].shipped_quantity = 6;
    bad.push(row);
    let mut row = batch.clone();
    row.lines[0].delivered_quantity = 4;
    bad.push(row);
    let mut row = batch.clone();
    row.lines[0].delivered_quantity = -1;
    bad.push(row);
    let mut row = batch.clone();
    row.lines = (0..=MAX_SHIPMENT_STOCK_ITEMS)
        .map(|_| ShipmentStockLine {
            fulfillment_item_id: Uuid::new_v4(),
            ..batch.lines[0]
        })
        .collect();
    bad.push(row);
    for row in bad {
        assert!(validate_batch(&row).is_err());
    }
}

#[test]
fn native_shipped_progress_overflow_is_a_typed_refusal() {
    let (tenant, mut batch, mut proof) = fixture();
    batch.lines[0].fulfillment_quantity = i32::MAX;
    batch.lines[0].shipped_quantity = i32::MAX - 1;
    proof.fulfillment_quantity = i32::MAX;
    proof.shipped_quantity = i32::MAX - 1;
    let error = next_progress(
        tenant,
        &batch,
        &batch.lines[0],
        Some(&proof),
        ShipmentStockAction::Ship,
    )
    .expect_err("checked addition");
    assert_eq!(error.code, "inventory.shipment_stock_quantity_overflow");
    assert!(!error.retryable);
}

#[test]
fn native_allocation_preserves_and_refuses_each_nonnull_terminal_marker() {
    let (_, batch, proof) = fixture();
    let item_id = proof.inventory_item_id.expect("item");
    let location = proof.location_id.expect("location");
    let line = batch.lines[0].order_line_item_id;
    let variant = batch.lines[0].variant_id.expect("variant");
    let now = Utc::now().fixed_offset();
    let allocation = reservation_item::Model {
        id: proof.reservation_id.expect("reservation"),
        inventory_item_id: item_id,
        location_id: location,
        quantity: 2,
        line_item_id: Some(line),
        description: None,
        external_id: None,
        metadata: serde_json::json!({}),
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    validate_allocation(&allocation, item_id, line, variant).expect("retained active allocation");
    for marker in [
        "inventory_disposition",
        "superseded_by",
        "released_at",
        "disposed_at",
        "superseded_at",
    ] {
        for value in [
            serde_json::json!("retained"),
            serde_json::json!(false),
            serde_json::json!(0),
        ] {
            let mut terminal = allocation.clone();
            terminal.metadata[marker] = value;
            let before = terminal.clone();
            assert!(validate_allocation(&terminal, item_id, line, variant).is_err());
            assert_eq!(terminal, before, "refusal never clears terminal provenance");
        }
    }
}

#[test]
fn native_shipment_elapsed_budget_cannot_authorize_writes_after_lock_wait() {
    let context = PortContext::new(
        Uuid::new_v4().to_string(),
        rustok_api::PortActor::system(),
        "fr",
        "budget",
    )
    .with_idempotency_key("original-budget-key")
    .with_deadline(std::time::Duration::from_secs(1));
    let started = std::time::Instant::now();
    super::super::require_remaining_budget(&context, started).expect("fresh budget");
    let expired = started
        .checked_sub(std::time::Duration::from_secs(2))
        .expect("past monotonic instant");
    let error = super::super::require_remaining_budget(&context, expired)
        .expect_err("positive entry budget is not a fresh lease after a lock wait");
    assert_eq!(error.code, "inventory.shipment_stock_deadline_expired");
    assert_eq!(error.kind, rustok_api::PortErrorKind::Timeout);
    assert_eq!(
        context.idempotency_key.as_deref(),
        Some("original-budget-key")
    );
}
