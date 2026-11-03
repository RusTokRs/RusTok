//! Focused definitions only. Do not execute in the agent's source-only handoff.
use super::snapshot::{summarize, Level};
use super::*;
use std::time::Duration;

fn context() -> PortContext {
    PortContext::new(
        Uuid::new_v4().to_string(),
        rustok_api::PortActor::user(Uuid::new_v4().to_string()),
        "en",
        "inventory-source",
    )
    .with_claim(Permission::INVENTORY_MANAGE.to_string())
    .with_idempotency_key("diagnostic-not-a-receipt")
    .with_deadline(Duration::from_secs(10))
}

fn level(item: Uuid, stocked: i32, reserved: i32, incoming: i32) -> Level {
    Level {
        id: Uuid::new_v4(),
        inventory_item_id: item,
        location_id: Uuid::new_v4(),
        stocked_quantity: stocked,
        reserved_quantity: reserved,
        incoming_quantity: incoming,
    }
}

#[test]
fn source_context_requires_current_permission_positive_scope_and_bounded_deadline() {
    let c = context();
    assert!(scope(&c, true).is_ok());
    let mut bad = c.clone();
    bad.claims.clear();
    assert!(matches!(
        scope(&bad, true),
        Err(InventorySourceError::Forbidden)
    ));
    let mut bad = c.clone();
    bad.tenant_id = Uuid::nil().to_string();
    assert!(matches!(
        scope(&bad, true),
        Err(InventorySourceError::InvalidRequest)
    ));
    let mut bad = c.clone();
    bad.actor.id = Uuid::nil().to_string();
    assert!(scope(&bad, true).is_err());
    let mut bad = c.clone();
    bad.deadline_ms = Some(30_001);
    assert!(scope(&bad, true).is_err());
    let mut bad = c;
    bad.idempotency_key = None;
    assert!(scope(&bad, true).is_err());
}

#[test]
fn source_revisions_never_start_from_zero_wrap_or_substitute_product_revision() {
    assert_eq!(writer::next_version(1).unwrap(), 2);
    for n in [0, -1, i64::MAX] {
        assert!(writer::next_version(n).is_err());
    }
    assert_eq!(writer::next_version(i64::MAX - 1).unwrap(), i64::MAX);
}

#[test]
fn source_snapshot_preserves_wide_signed_availability_without_saturation() {
    let item = Uuid::new_v4();
    let levels = [
        level(item, i32::MAX, 0, 0),
        level(item, i32::MAX, 0, 0),
        level(item, 0, i32::MAX, 0),
    ];
    let s = summarize(Uuid::new_v4(), Uuid::new_v4(), item, &levels).unwrap();
    assert_eq!(s.stocked, 2 * i64::from(i32::MAX));
    assert_eq!(s.stocked - s.reserved, i64::from(i32::MAX));
    let s = summarize(
        Uuid::new_v4(),
        Uuid::new_v4(),
        item,
        &[level(item, 0, 7, 3)],
    )
    .unwrap();
    assert_eq!(s.stocked - s.reserved, -7);
}

#[test]
fn source_snapshot_requires_complete_positive_unique_bindings_and_each_counter() {
    let item = Uuid::new_v4();
    let variant = Uuid::new_v4();
    let parent = Uuid::new_v4();
    assert!(summarize(variant, parent, item, &[]).is_err());
    let row = level(item, 1, 0, 0);
    assert!(summarize(variant, parent, item, &[row.clone(), row.clone()]).is_err());
    let mut bad = row.clone();
    bad.location_id = Uuid::nil();
    assert!(summarize(variant, parent, item, &[bad]).is_err());
    let mut bad = row.clone();
    bad.id = Uuid::nil();
    assert!(summarize(variant, parent, item, &[bad]).is_err());
    let mut bad = row.clone();
    bad.inventory_item_id = Uuid::new_v4();
    assert!(summarize(variant, parent, item, &[bad]).is_err());
    for (stocked, reserved, incoming) in [(-1, 0, 0), (0, -1, 0), (0, 0, -1)] {
        assert!(summarize(
            variant,
            parent,
            item,
            &[level(item, stocked, reserved, incoming)]
        )
        .is_err());
    }
    let mut levels: Vec<_> = (0..MAX_SOURCE_LEVELS)
        .map(|_| level(item, 0, 0, 0))
        .collect();
    assert!(summarize(variant, parent, item, &levels).is_ok());
    levels.push(level(item, 0, 0, 0));
    assert!(matches!(
        summarize(variant, parent, item, &levels),
        Err(InventorySourceError::LimitExceeded)
    ));
}

#[test]
fn source_binding_digest_is_order_independent_but_detects_replaced_identity() {
    let item = Uuid::new_v4();
    let variant = Uuid::new_v4();
    let parent = Uuid::new_v4();
    let rows = [level(item, 3, 1, 0), level(item, 5, 2, 1)];
    let before = summarize(variant, parent, item, &rows)
        .unwrap()
        .binding_digest;
    assert_eq!(
        before,
        summarize(variant, parent, item, &[rows[1].clone(), rows[0].clone()])
            .unwrap()
            .binding_digest
    );
    let mut changed = rows.clone();
    changed[0].location_id = Uuid::new_v4();
    assert_ne!(
        before,
        summarize(variant, parent, item, &changed)
            .unwrap()
            .binding_digest
    );
}

#[test]
fn source_page_limits_and_error_envelopes_do_not_leak_provider_payload() {
    for n in [0, MAX_SOURCE_PAGE + 1, usize::MAX] {
        assert!(reader::page_limit(n).is_err());
    }
    assert!(reader::page_limit(MAX_SOURCE_PAGE).is_ok());
    let e =
        InventorySourceError::Database(DbErr::Custom("password private SQL tenant payload".into()));
    assert!(!e.to_string().contains("password"));
    assert!(!e.port_error().message.contains("password"));
    assert_eq!(
        InventorySourceError::StateUnverified.port_error().kind,
        rustok_api::PortErrorKind::InvariantViolation
    );
    assert_eq!(
        InventorySourceError::PublicationUnavailable
            .port_error()
            .kind,
        rustok_api::PortErrorKind::Unavailable
    );
    assert_eq!(
        InventorySourceError::DeadlineExceeded.port_error().kind,
        rustok_api::PortErrorKind::Timeout
    );
    assert!(!matches!(
        InventorySourceError::StateUnverified.into_db_error(),
        DbErr::RecordNotFound(_)
    ));
}
