//! Focused owner regressions; definitions only, not executed in the source handoff.

use super::*;

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

fn availability(levels: &[Level], item: Uuid) -> WriteResult<i32> {
    available_quantity(
        levels,
        item,
        &levels.iter().map(|level| level.location_id).collect(),
    )
}

fn rejects<T>(result: WriteResult<T>, expected: QuantityWriteBlocker) {
    assert!(matches!(result, Err(QuantityWriteError::Rejected(actual)) if actual == expected));
}

#[test]
fn quantity_scope_requires_each_current_positive_identity_before_database_work() {
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let variant = Uuid::new_v4();
    for (t, a, v) in [
        (Uuid::nil(), actor, variant),
        (tenant, Uuid::nil(), variant),
        (tenant, actor, Uuid::nil()),
    ] {
        rejects(
            validate_request(t, a, v),
            QuantityWriteBlocker::InvalidRequest,
        );
    }
    assert!(validate_request(tenant, actor, variant).is_ok());
}

#[test]
fn quantity_adjustment_extremes_use_checked_addition_without_negating_i32_min() {
    rejects(
        target_quantity(i32::MAX, Change::Adjust(1)),
        QuantityWriteBlocker::QuantityUnrepresentable,
    );
    rejects(
        target_quantity(i32::MIN, Change::Adjust(-1)),
        QuantityWriteBlocker::QuantityUnrepresentable,
    );
    assert_eq!(
        target_quantity(0, Change::Adjust(i32::MIN)).unwrap(),
        i32::MIN
    );
    assert_eq!(
        target_quantity(i32::MAX, Change::Adjust(i32::MIN)).unwrap(),
        -1
    );
    rejects(
        stocked_target(0, i32::MIN, 0, 0, true),
        QuantityWriteBlocker::InsufficientStock,
    );
}

#[test]
fn quantity_target_is_global_available_and_preserves_other_location_and_reserved_units() {
    let item = Uuid::new_v4();
    let original = [level(item, 10, 4, 8), level(item, 7, 5, 2)];
    let before = availability(&original, item).unwrap();
    assert_eq!(before, 8);
    let target = stocked_target(
        before,
        12,
        original[0].stocked_quantity,
        original[0].reserved_quantity,
        false,
    )
    .unwrap();
    assert_eq!(target, 14);
    let mut changed = original.clone();
    changed[0].stocked_quantity = target;
    assert_eq!(availability(&changed, item).unwrap(), 12);
    assert_eq!(changed[0].reserved_quantity, 4);
    assert_eq!(changed[0].incoming_quantity, 8);
    assert_eq!(changed[1], original[1]);
}

#[test]
fn quantity_sum_is_wide_checked_and_never_saturates_signed_locations() {
    let item = Uuid::new_v4();
    let mut levels = vec![level(item, i32::MAX, 0, 0), level(item, i32::MAX, 0, 0)];
    rejects(
        availability(&levels, item),
        QuantityWriteBlocker::QuantityUnrepresentable,
    );
    levels.push(level(item, 0, i32::MAX, 0));
    assert_eq!(availability(&levels, item).unwrap(), i32::MAX);
    let negative = [level(item, 0, 4, 1), level(item, 1, 0, 2)];
    assert_eq!(availability(&negative, item).unwrap(), -3);
}

#[test]
fn quantity_requires_nonempty_complete_bounded_positive_unique_level_bindings() {
    let item = Uuid::new_v4();
    rejects(
        availability(&[], item),
        QuantityWriteBlocker::IdentityUnavailable,
    );
    let mut levels = (0..MAX_QUANTITY_LEVELS)
        .map(|_| level(item, 0, 0, 0))
        .collect::<Vec<_>>();
    assert_eq!(availability(&levels, item).unwrap(), 0);
    levels.push(level(item, 0, 0, 0));
    rejects(
        availability(&levels, item),
        QuantityWriteBlocker::LimitExceeded,
    );
    for field in [
        "id",
        "item",
        "location",
        "duplicate_id",
        "duplicate_location",
    ] {
        let mut rows = [level(item, 0, 0, 0), level(item, 0, 0, 0)];
        match field {
            "id" => rows[0].id = Uuid::nil(),
            "item" => rows[0].inventory_item_id = Uuid::new_v4(),
            "location" => rows[0].location_id = Uuid::nil(),
            "duplicate_id" => rows[0].id = rows[1].id,
            "duplicate_location" => rows[0].location_id = rows[1].location_id,
            _ => unreachable!(),
        }
        rejects(
            availability(&rows, item),
            QuantityWriteBlocker::StateUnverified,
        );
    }
    let rows = [level(item, 0, 0, 0)];
    rejects(
        available_quantity(&rows, item, &BTreeSet::new()),
        QuantityWriteBlocker::StateUnverified,
    );
}

#[test]
fn quantity_refuses_each_negative_saved_counter_without_using_net_availability_as_proof() {
    let item = Uuid::new_v4();
    for (stocked, reserved, incoming) in [(-1, 0, 0), (0, -1, 0), (0, 0, -1), (-1, -1, 0)] {
        rejects(
            availability(&[level(item, stocked, reserved, incoming)], item),
            QuantityWriteBlocker::StateUnverified,
        );
    }
}

#[test]
fn quantity_backorder_never_creates_negative_physical_stock_or_clamps_default_allocation() {
    rejects(
        stocked_target(5, 0, 0, 0, true),
        QuantityWriteBlocker::InsufficientStock,
    );
    rejects(
        stocked_target(2, 0, 3, 4, false),
        QuantityWriteBlocker::InsufficientStock,
    );
    assert_eq!(stocked_target(2, 0, 3, 4, true).unwrap(), 1);
    rejects(
        stocked_target(0, 1, i32::MAX, 0, true),
        QuantityWriteBlocker::QuantityUnrepresentable,
    );
}

#[test]
fn quantity_real_change_refuses_unrepresentable_v1_notification_but_signed_noop_is_valid() {
    for (before, after) in [(-1, 0), (0, -1), (-2, -1)] {
        rejects(
            validate_event_quantities(before, after),
            QuantityWriteBlocker::EventUnrepresentable,
        );
    }
    assert!(validate_event_quantities(0, i32::MAX).is_ok());
    assert_eq!(target_quantity(-3, Change::Adjust(0)).unwrap(), -3);
    assert_eq!(target_quantity(-3, Change::Set(-3)).unwrap(), -3);
}

#[test]
fn quantity_error_native_policy_keeps_provider_text_private_and_unknown_publication_nonterminal() {
    for blocker in QuantityWriteBlocker::ALL {
        let error = QuantityWriteError::Rejected(blocker);
        let policy = error.port_error();
        assert_eq!(policy.kind, blocker.kind());
        assert_eq!(policy.code, blocker.code());
        if blocker.kind() == PortErrorKind::InvariantViolation {
            assert_eq!(
                policy.message,
                "the requested operation could not be completed safely"
            );
        } else {
            assert_eq!(policy.message, blocker.message());
        }
        assert!(!policy.retryable);
    }
    for error in [
        QuantityWriteError::Database(DbErr::Custom("private-provider-details".into())),
        QuantityWriteError::Publication(CoreError::Database(DbErr::Custom(
            "private-provider-details".into(),
        ))),
    ] {
        let policy = error.port_error();
        assert_eq!(policy.kind, PortErrorKind::Unavailable);
        assert!(policy.retryable);
        assert!(!policy.message.contains("private-provider-details"));
        assert!(!error.to_string().contains("private-provider-details"));
    }
    let policy =
        QuantityWriteError::Publication(CoreError::Validation("private-provider-details".into()))
            .port_error();
    assert_eq!(policy.kind, PortErrorKind::InvariantViolation);
    assert!(!policy.retryable);
    assert!(!policy.message.contains("private-provider-details"));
}

#[test]
fn quantity_product_response_and_visible_projection_sum_without_order_dependent_saturation() {
    let item = Uuid::new_v4();
    let variant = Uuid::new_v4();
    let binding = std::collections::HashMap::from([(item, variant)]);
    let mut rows = [(i32::MAX, 0), (i32::MAX, 0), (0, i32::MAX)]
        .into_iter()
        .map(|(stocked, reserved)| inventory_level::Model {
            id: Uuid::new_v4(),
            inventory_item_id: item,
            location_id: Uuid::new_v4(),
            stocked_quantity: stocked,
            reserved_quantity: reserved,
            incoming_quantity: 0,
            low_stock_threshold: None,
            updated_at: Utc::now().into(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        available_by_variant(&binding, &rows).unwrap()[&variant],
        i32::MAX
    );
    rows.reverse();
    assert_eq!(
        available_by_variant(&binding, &rows).unwrap()[&variant],
        i32::MAX
    );
    assert!(
        available_by_variant(
            &binding,
            rows.iter().filter(|row| row.reserved_quantity == 0)
        )
        .is_err()
    );
    rows[0].incoming_quantity = -1;
    assert!(available_by_variant(&binding, &rows).is_err());
    assert!(available_by_variant(&std::collections::HashMap::new(), &rows).is_err());
}
