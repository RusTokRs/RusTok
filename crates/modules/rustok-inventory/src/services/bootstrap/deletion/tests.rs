use super::*;

fn level() -> Level {
    Level {
        id: Uuid::from_u128(1),
        inventory_item_id: Uuid::from_u128(2),
        location_id: Uuid::from_u128(3),
        stocked_quantity: 0,
        reserved_quantity: 0,
        incoming_quantity: 0,
    }
}

fn assert_rejected<T: std::fmt::Debug>(
    result: Result<T, CatalogDeletionError>,
    expected: CatalogDeletionBlocker,
) {
    assert!(matches!(
        result.expect_err("owner proof must refuse"),
        CatalogDeletionError::Rejected(actual) if actual == expected
    ));
}

#[test]
fn current_tenant_and_distinct_nonnil_identities_precede_storage() {
    let tenant = Uuid::from_u128(10);
    assert_rejected(
        validate_request(Uuid::nil(), &[]),
        CatalogDeletionBlocker::InvalidRequest,
    );
    assert_rejected(
        validate_request(tenant, &[Uuid::nil()]),
        CatalogDeletionBlocker::InvalidRequest,
    );
    let id = Uuid::from_u128(1);
    assert_rejected(
        validate_request(tenant, &[id, id]),
        CatalogDeletionBlocker::InvalidRequest,
    );
    assert!(validate_request(tenant, &[]).unwrap().is_empty());
}

#[test]
fn variant_request_bound_is_inclusive_and_locks_use_variant_order() {
    let tenant = Uuid::from_u128(1000);
    let ids = (1..=MAX_CATALOG_DELETE_VARIANTS)
        .rev()
        .map(|id| Uuid::from_u128(id as u128))
        .collect::<Vec<_>>();
    let admitted = validate_request(tenant, &ids).unwrap();
    assert_eq!(admitted.len(), MAX_CATALOG_DELETE_VARIANTS);
    assert!(admitted.windows(2).all(|pair| pair[0] < pair[1]));
    let mut oversized = ids;
    oversized.push(Uuid::from_u128(2000));
    assert_rejected(
        validate_request(tenant, &oversized),
        CatalogDeletionBlocker::LimitExceeded,
    );
}

#[test]
fn every_individual_counter_must_be_zero_not_net_availability() {
    let item_ids = BTreeSet::from([level().inventory_item_id]);
    let locations = BTreeSet::from([level().location_id]);
    for (stocked, reserved, incoming) in [
        (1, 0, 0),
        (0, 1, 0),
        (0, 0, 1),
        (5, 5, 0),
        (-1, 0, 0),
        (0, -1, 0),
        (0, 0, -1),
        (i32::MAX, i32::MAX, 0),
        (i32::MIN, 0, 0),
    ] {
        let mut candidate = level();
        candidate.stocked_quantity = stocked;
        candidate.reserved_quantity = reserved;
        candidate.incoming_quantity = incoming;
        assert_rejected(
            validate_levels(&[candidate], &item_ids, &locations),
            CatalogDeletionBlocker::StockPresent,
        );
    }
    validate_levels(&[level()], &item_ids, &locations).unwrap();
}

#[test]
fn offsetting_locations_never_prove_empty_inventory() {
    let mut positive = level();
    positive.stocked_quantity = 7;
    let mut negative = level();
    negative.id = Uuid::from_u128(4);
    negative.location_id = Uuid::from_u128(5);
    negative.stocked_quantity = -7;
    assert_rejected(
        validate_levels(
            &[positive.clone(), negative.clone()],
            &BTreeSet::from([positive.inventory_item_id]),
            &BTreeSet::from([positive.location_id, negative.location_id]),
        ),
        CatalogDeletionBlocker::StockPresent,
    );
}

#[test]
fn duplicate_level_identity_or_item_location_binding_refuses() {
    let first = level();
    let item_ids = BTreeSet::from([first.inventory_item_id]);
    let mut duplicate_binding = first.clone();
    duplicate_binding.id = Uuid::from_u128(4);
    assert_rejected(
        validate_levels(
            &[first.clone(), duplicate_binding],
            &item_ids,
            &BTreeSet::from([first.location_id]),
        ),
        CatalogDeletionBlocker::StateUnverified,
    );
    let mut duplicate_identity = first.clone();
    duplicate_identity.location_id = Uuid::from_u128(5);
    assert_rejected(
        validate_levels(
            &[first.clone(), duplicate_identity.clone()],
            &item_ids,
            &BTreeSet::from([first.location_id, duplicate_identity.location_id]),
        ),
        CatalogDeletionBlocker::StateUnverified,
    );
}

#[test]
fn missing_foreign_or_nil_level_bindings_refuse() {
    let candidate = level();
    for (items, locations) in [
        (BTreeSet::new(), BTreeSet::from([candidate.location_id])),
        (
            BTreeSet::from([candidate.inventory_item_id]),
            BTreeSet::new(),
        ),
    ] {
        assert_rejected(
            validate_levels(&[candidate.clone()], &items, &locations),
            CatalogDeletionBlocker::StateUnverified,
        );
    }
    let mut nil_id = candidate.clone();
    nil_id.id = Uuid::nil();
    assert_rejected(
        validate_levels(
            &[nil_id],
            &BTreeSet::from([candidate.inventory_item_id]),
            &BTreeSet::from([candidate.location_id]),
        ),
        CatalogDeletionBlocker::StateUnverified,
    );
}

#[test]
fn whole_batch_level_bound_is_inclusive_and_never_truncates() {
    let item = Uuid::from_u128(1);
    let mut levels = (0..MAX_DELETE_LEVELS)
        .map(|index| Level {
            id: Uuid::from_u128(10_000 + index as u128),
            inventory_item_id: item,
            location_id: Uuid::from_u128(20_000 + index as u128),
            stocked_quantity: 0,
            reserved_quantity: 0,
            incoming_quantity: 0,
        })
        .collect::<Vec<_>>();
    let items = BTreeSet::from([item]);
    let locations = levels
        .iter()
        .map(|row| row.location_id)
        .collect::<BTreeSet<_>>();
    validate_levels(&levels, &items, &locations).unwrap();
    levels.push(Level {
        id: Uuid::from_u128(50_000),
        location_id: Uuid::from_u128(50_001),
        ..level()
    });
    assert_rejected(
        validate_levels(&levels, &items, &locations),
        CatalogDeletionBlocker::LimitExceeded,
    );
}

#[test]
fn empty_child_set_only_validates_levels_not_missing_inventory_identity() {
    // DeletionPlan::load separately requires every positive tenant-proved item identity.
    validate_levels(&[], &BTreeSet::from([Uuid::from_u128(1)]), &BTreeSet::new()).unwrap();
}

#[test]
fn closed_blocker_copy_contains_no_dynamic_identity_or_retry_advice() {
    let messages = CatalogDeletionBlocker::ALL
        .iter()
        .map(|reason| reason.message())
        .collect::<BTreeSet<_>>();
    assert_eq!(messages.len(), CatalogDeletionBlocker::ALL.len());
    for message in messages {
        assert!(!message.is_empty());
        assert!(message.len() <= 256);
        assert!(!message.contains("retryable"));
        assert!(!message.contains("no changes were made"));
    }
}
