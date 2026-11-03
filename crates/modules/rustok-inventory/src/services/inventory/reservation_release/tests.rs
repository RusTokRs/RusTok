//! Source definitions only: no test execution or runtime claim in this continuation.

use super::*;
use crate::services::inventory::{checked_available_quantity, STOCK_LEVEL_READ_LIMIT};
use serde_json::json;

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn row(value: u128, location: u128, quantity: i32) -> reservation_item::Model {
    let now = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    reservation_item::Model {
        id: id(value),
        inventory_item_id: id(10),
        location_id: id(location),
        quantity,
        line_item_id: None,
        description: Some("Retained anonymous allocation".to_string()),
        external_id: None,
        metadata: json!({"source": "legacy_inventory_service", "variant_id": id(20)}),
        created_at: now.into(),
        updated_at: now.into(),
        deleted_at: None,
    }
}

fn location(value: u128) -> stock_location::Model {
    let now = Utc::now();
    stock_location::Model {
        id: id(value),
        tenant_id: id(30),
        code: None,
        address_line1: None,
        address_line2: None,
        city: None,
        province: None,
        postal_code: None,
        country_code: None,
        phone: None,
        metadata: json!({"channel_visibility": {"allowed_channel_slugs": ["hidden"]}}),
        created_at: now.into(),
        updated_at: now.into(),
        deleted_at: None,
    }
}

fn level(value: u128, reserved: i32) -> inventory_level::Model {
    inventory_level::Model {
        id: id(value + 100),
        inventory_item_id: id(10),
        location_id: id(value),
        stocked_quantity: 10,
        reserved_quantity: reserved,
        incoming_quantity: 0,
        low_stock_threshold: None,
        updated_at: Utc::now().into(),
    }
}

fn selected(quantity: i32, rows: Vec<reservation_item::Model>) -> SelectedQuantityRelease {
    SelectedQuantityRelease::select(id(10), id(20), quantity, rows).unwrap()
}

fn bind(
    selected: SelectedQuantityRelease,
    tenant_id: Uuid,
    locations: Vec<stock_location::Model>,
    levels: Vec<inventory_level::Model>,
) -> CommerceResult<QuantityReleasePlan> {
    // Fixture rows are the complete ledger in these pure cases. The runtime adapter instead
    // supplies the grouped database aggregate, including keyed rows not materialized in the pool.
    let claims = selected
        .active_claims_by_location
        .iter()
        .filter(|(location, _)| selected.released_by_location.contains_key(*location))
        .map(|(location, quantity)| (*location, *quantity))
        .collect();
    selected.bind_levels(tenant_id, locations, levels, claims)
}

#[test]
fn only_anonymous_rows_are_selected_and_released_at_original_locations() {
    let anonymous = row(2, 42, 3);
    let mut keyed = row(1, 41, 8);
    keyed.external_id = Some("checkout:retained".to_string());
    keyed.line_item_id = Some(id(90));
    let selected = selected(2, vec![anonymous.clone(), keyed]);
    assert_eq!(selected.location_ids(), vec![id(42)]);
    let plan = bind(selected, id(30), vec![location(42)], vec![level(42, 3)]).unwrap();
    assert_eq!(plan.reservations, vec![(anonymous, 1)]);
    assert_eq!(plan.levels.len(), 1);
    assert_eq!(plan.levels[0].0.location_id, id(42));
    assert_eq!(plan.levels[0].1, 1);
}

#[test]
fn anonymous_fifo_ties_and_grouped_level_order_are_deterministic() {
    let selected = selected(4, vec![row(3, 42, 2), row(2, 41, 3)]);
    let plan = bind(
        selected,
        id(30),
        vec![location(42), location(41)],
        vec![level(42, 2), level(41, 3)],
    )
    .unwrap();
    assert_eq!(plan.reservations[0].0.id, id(2));
    assert_eq!(plan.reservations[0].1, 0);
    assert_eq!(plan.reservations[1].0.id, id(3));
    assert_eq!(plan.reservations[1].1, 1);
    assert_eq!(plan.levels[0].0.location_id, id(41));
    assert_eq!(plan.levels[0].1, 0);
    assert_eq!(plan.levels[1].0.location_id, id(42));
    assert_eq!(plan.levels[1].1, 1);
}

#[test]
fn missing_unknown_or_checkout_provenance_is_not_an_anonymous_release_capability() {
    for metadata in [
        json!({}),
        json!({"variant_id": id(20)}),
        json!({"source": "checkout_order_adoption", "variant_id": id(20)}),
        json!({"source": "legacy_inventory_service", "variant_id": id(20), "cart_id": id(91)}),
        json!({"source": "legacy_inventory_service", "variant_id": id(20), "reservation_id": id(92)}),
        json!({"source": "legacy_inventory_service", "variant_id": id(20), "order_line_item_id": id(93)}),
    ] {
        let mut candidate = row(1, 41, 5);
        candidate.metadata = metadata;
        assert!(SelectedQuantityRelease::select(id(10), id(20), 1, vec![candidate]).is_err());
    }
}

#[test]
fn keyed_or_line_bound_rows_never_join_the_quantity_pool() {
    for external in [true, false] {
        let mut candidate = row(1, 41, 5);
        if external {
            candidate.external_id = Some("retained-key".to_string());
        } else {
            candidate.line_item_id = Some(id(90));
        }
        assert!(SelectedQuantityRelease::select(id(10), id(20), 1, vec![candidate]).is_err());
    }
}

#[test]
fn contradictory_variant_and_all_five_terminal_markers_refuse_before_any_plan() {
    let mut wrong = row(2, 41, 3);
    wrong.metadata["variant_id"] = json!(id(99));
    assert!(
        SelectedQuantityRelease::select(id(10), id(20), 1, vec![row(1, 41, 3), wrong]).is_err()
    );
    for marker in [
        "inventory_disposition",
        "superseded_by",
        "released_at",
        "disposed_at",
        "superseded_at",
    ] {
        for value in [json!(false), json!(0)] {
            let mut terminal = row(2, 41, 3);
            terminal.metadata[marker] = value;
            assert!(SelectedQuantityRelease::select(
                id(10),
                id(20),
                1,
                vec![row(1, 41, 3), terminal],
            )
            .is_err());
        }
    }
}

#[test]
fn every_live_claim_at_a_touched_level_is_protected_not_only_requested_units() {
    let mut keyed = row(1, 41, 7);
    keyed.external_id = Some("another-consumer".to_string());
    let selected = selected(2, vec![keyed, row(2, 41, 3)]);
    assert!(bind(selected, id(30), vec![location(41)], vec![level(41, 5)]).is_err());
}

#[test]
fn missing_foreign_duplicate_and_relocated_levels_refuse_without_fallback() {
    let mut foreign = location(41);
    foreign.tenant_id = id(99);
    let cases = [
        (vec![], vec![level(41, 3)]),
        (vec![foreign], vec![level(41, 3)]),
        (vec![location(41)], vec![]),
        (vec![location(41)], vec![level(42, 3)]),
        (vec![location(41)], vec![level(41, 3), level(41, 3)]),
    ];
    for (locations, levels) in cases {
        assert!(bind(selected(1, vec![row(1, 41, 3)]), id(30), locations, levels).is_err());
    }
    assert!(bind(
        selected(4, vec![row(1, 41, 3), row(2, 42, 3)]),
        id(30),
        vec![location(41), location(42)],
        vec![level(41, 3), level(41, 3)],
    )
    .is_err());
}

#[test]
fn archived_original_location_can_only_be_left_completely_zero() {
    let mut archived = location(41);
    archived.deleted_at = Some(Utc::now().into());
    let mut empty_stock = level(41, 3);
    empty_stock.stocked_quantity = 0;
    assert!(bind(
        selected(2, vec![row(1, 41, 3)]),
        id(30),
        vec![archived.clone()],
        vec![empty_stock.clone()],
    )
    .is_err());
    assert!(bind(
        selected(3, vec![row(1, 41, 3)]),
        id(30),
        vec![archived.clone()],
        vec![level(41, 3)],
    )
    .is_err());
    let plan = bind(
        selected(3, vec![row(1, 41, 3)]),
        id(30),
        vec![archived],
        vec![empty_stock],
    )
    .unwrap();
    assert_eq!(plan.levels[0].1, 0);
}

#[test]
fn active_row_bound_and_large_claim_totals_never_truncate_or_overflow() {
    let rows = (0..QUANTITY_RESERVATION_ROW_LIMIT)
        .map(|index| row(index as u128 + 1, 41, 1))
        .collect::<Vec<_>>();
    assert!(SelectedQuantityRelease::select(id(10), id(20), 1, rows.clone()).is_ok());
    let mut excessive = rows;
    excessive.push(row(999_999, 41, 1));
    assert!(SelectedQuantityRelease::select(id(10), id(20), 1, excessive).is_err());
    let selected = selected(1, vec![row(1, 41, i32::MAX), row(2, 41, i32::MAX)]);
    assert!(bind(
        selected,
        id(30),
        vec![location(41)],
        vec![level(41, i32::MAX)]
    )
    .is_err());
}

#[test]
fn invalid_active_rows_and_nonpositive_plan_requests_refuse() {
    for quantity in [0, -1] {
        assert!(
            SelectedQuantityRelease::select(id(10), id(20), quantity, vec![row(1, 41, 3)]).is_err()
        );
    }
    let mut deleted = row(1, 41, 3);
    deleted.deleted_at = Some(Utc::now().into());
    let mut foreign = row(1, 41, 3);
    foreign.inventory_item_id = id(99);
    for invalid in [row(1, 41, 0), deleted, foreign] {
        assert!(SelectedQuantityRelease::select(id(10), id(20), 1, vec![invalid]).is_err());
    }
    assert!(
        SelectedQuantityRelease::select(id(10), id(20), 1, vec![row(1, 41, 3), row(1, 41, 3)])
            .is_err()
    );
}

#[test]
fn signed_availability_preserves_backorders_and_wide_intermediate_cancellation() {
    let mut backorder = level(41, 8);
    backorder.stocked_quantity = 3;
    assert_eq!(checked_available_quantity(&[backorder]).unwrap(), -5);
    let mut negative = level(41, i32::MAX);
    negative.stocked_quantity = i32::MIN;
    let mut positive = level(42, 0);
    positive.stocked_quantity = i32::MAX;
    assert_eq!(
        checked_available_quantity(&[negative, positive]).unwrap(),
        i32::MIN
    );
}

#[test]
fn availability_overflow_negative_reserved_and_level_bound_refuse_without_saturation() {
    let mut huge = level(41, 0);
    huge.stocked_quantity = i32::MAX;
    assert!(checked_available_quantity(&[huge.clone(), huge]).is_err());
    let mut too_negative = level(41, 1);
    too_negative.stocked_quantity = i32::MIN;
    assert!(checked_available_quantity(&[too_negative]).is_err());
    assert!(checked_available_quantity(&[level(41, -1)]).is_err());
    assert!(checked_available_quantity(&vec![level(41, 0); STOCK_LEVEL_READ_LIMIT + 1]).is_err());
    assert_eq!(checked_available_quantity(&[]).unwrap(), 0);
}

#[test]
fn database_claims_include_keyed_rows_not_materialized_in_the_anonymous_pool() {
    let pool = selected(2, vec![row(1, 41, 3)]);
    assert!(pool
        .bind_levels(
            id(30),
            vec![location(41)],
            vec![level(41, 5)],
            BTreeMap::from([(id(41), 7)])
        )
        .is_err());
    let pool = selected(2, vec![row(1, 41, 3)]);
    let plan = pool
        .bind_levels(
            id(30),
            vec![location(41)],
            vec![level(41, 7)],
            BTreeMap::from([(id(41), 7)]),
        )
        .unwrap();
    assert_eq!(plan.levels[0].1, 5);
}

#[test]
fn fresh_anonymous_reserve_cannot_grow_beyond_the_releasable_pool_or_over_terminal_history() {
    let rows = (0..QUANTITY_RESERVATION_ROW_LIMIT - 1)
        .map(|index| row(index as u128 + 1, 41, 1))
        .collect::<Vec<_>>();
    assert!(admit_new_reservation(id(10), id(20), rows.clone()).is_ok());
    let mut full = rows;
    full.push(row(999_999, 41, 1));
    assert!(admit_new_reservation(id(10), id(20), full).is_err());
    let mut terminal = row(1, 41, 1);
    terminal.metadata["disposed_at"] = json!(false);
    assert!(admit_new_reservation(id(10), id(20), vec![terminal]).is_err());
}
