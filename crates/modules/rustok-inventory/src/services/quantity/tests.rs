//! Focused owner regressions; definitions only, not executed in the source handoff.

use super::*;

use sea_orm::{ConnectOptions, Database, TransactionTrait};

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

#[test]
fn availability_keeps_existing_items_at_zero_when_they_have_no_levels() {
    let item = Uuid::new_v4();
    let variant = Uuid::new_v4();
    let binding = std::collections::HashMap::from([(item, variant)]);

    let existing_item_without_levels = available_by_variant(&binding, std::iter::empty())
        .expect("an existing item with no levels has an authoritative zero");
    assert_eq!(existing_item_without_levels.get(&variant), Some(&0));

    let missing_item = available_by_variant(&std::collections::HashMap::new(), std::iter::empty())
        .expect("a missing item has no Inventory-owned quantity");
    assert!(missing_item.is_empty());
}

#[test]
fn expected_quantity_snapshot_rejects_stale_values() {
    assert!(ensure_expected_quantity_matches(8, Some(8)).is_ok());
    assert!(ensure_expected_quantity_matches(9, Some(8)).is_err());
    assert!(ensure_expected_quantity_matches(8, None).is_ok());
}

#[test]
fn projection_refuses_duplicate_item_and_level_identities() {
    let item = Uuid::new_v4();
    let variant = Uuid::new_v4();
    let binding = std::collections::HashMap::from([(item, variant)]);
    let row = inventory_level::Model {
        id: Uuid::new_v4(),
        inventory_item_id: item,
        location_id: Uuid::new_v4(),
        stocked_quantity: 5,
        reserved_quantity: 8,
        incoming_quantity: 0,
        low_stock_threshold: None,
        updated_at: Utc::now().into(),
    };
    assert_eq!(
        available_by_variant(&binding, [&row]).unwrap()[&variant],
        -3
    );
    assert!(available_by_variant(&binding, [&row, &row]).is_err());
    let mut duplicate_location = row.clone();
    duplicate_location.id = Uuid::new_v4();
    assert!(available_by_variant(&binding, [&row, &duplicate_location]).is_err());
    let duplicate_items =
        std::collections::HashMap::from([(item, variant), (Uuid::new_v4(), variant)]);
    assert!(available_by_variant(&duplicate_items, std::iter::empty()).is_err());
    let nil_binding = std::collections::HashMap::from([(Uuid::nil(), variant)]);
    assert!(available_by_variant(&nil_binding, std::iter::empty()).is_err());
    let excessive: Vec<_> = (0..=MAX_QUANTITY_LEVELS)
        .map(|_| {
            let mut level = row.clone();
            level.id = Uuid::new_v4();
            level.location_id = Uuid::new_v4();
            level
        })
        .collect();
    assert!(available_by_variant(&binding, &excessive).is_err());
}

#[tokio::test]
async fn projection_loader_rejects_ambiguous_items_and_excess_original_levels() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    for sql in [
        "CREATE TABLE inventory_items (id TEXT PRIMARY KEY NOT NULL, variant_id TEXT NOT NULL, sku TEXT, requires_shipping BOOLEAN NOT NULL, metadata TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
        "CREATE TABLE inventory_levels (id TEXT PRIMARY KEY NOT NULL, inventory_item_id TEXT NOT NULL, location_id TEXT NOT NULL, stocked_quantity INTEGER NOT NULL, reserved_quantity INTEGER NOT NULL, incoming_quantity INTEGER NOT NULL, low_stock_threshold INTEGER, updated_at TEXT NOT NULL)",
    ] {
        db.execute_raw(Statement::from_string(DbBackend::Sqlite, sql))
            .await
            .unwrap();
    }
    let variant = Uuid::new_v4();
    let item = Uuid::new_v4();
    let duplicate = Uuid::new_v4();
    for id in [item, duplicate] {
        inventory_item::ActiveModel {
            id: Set(id),
            variant_id: Set(variant),
            sku: Set(None),
            requires_shipping: Set(true),
            metadata: Set(serde_json::json!({})),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&db)
        .await
        .unwrap();
    }
    assert!(
        load_projection_item_bindings(&db, &[variant])
            .await
            .is_err()
    );
    inventory_item::Entity::delete_by_id(duplicate)
        .exec(&db)
        .await
        .unwrap();
    let bindings = load_projection_item_bindings(&db, &[variant, variant])
        .await
        .unwrap();
    assert_eq!(bindings.get(&item), Some(&variant));
    assert_eq!(bindings.len(), 1);
    assert!(
        load_projection_levels(&db, &bindings)
            .await
            .unwrap()
            .is_empty()
    );

    // Use bounded inserts to avoid SQLite bind-parameter limits.
    let rows: Vec<_> = (0..=MAX_QUANTITY_LEVELS)
        .map(|_| inventory_level::ActiveModel {
            id: Set(Uuid::new_v4()),
            inventory_item_id: Set(item),
            location_id: Set(Uuid::new_v4()),
            stocked_quantity: Set(1),
            reserved_quantity: Set(0),
            incoming_quantity: Set(0),
            low_stock_threshold: Set(None),
            updated_at: Set(Utc::now().into()),
        })
        .collect();
    for chunk in rows[..MAX_QUANTITY_LEVELS].chunks(64) {
        inventory_level::Entity::insert_many(chunk.iter().cloned())
            .exec(&db)
            .await
            .unwrap();
    }
    assert_eq!(
        load_projection_levels(&db, &bindings).await.unwrap().len(),
        MAX_QUANTITY_LEVELS
    );
    rows[MAX_QUANTITY_LEVELS].clone().insert(&db).await.unwrap();
    assert!(load_projection_levels(&db, &bindings).await.is_err());
}

#[tokio::test]
async fn sqlite_quantity_snapshot_reserves_writer_before_plan_reads() {
    let path = std::env::temp_dir().join(format!(
        "rustok-inventory-quantity-writer-{}.sqlite",
        Uuid::new_v4()
    ));
    let url = format!("sqlite://{}?mode=rwc", path.display());

    let mut owner_options = ConnectOptions::new(url.clone());
    owner_options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let owner = Database::connect(owner_options)
        .await
        .expect("owner database should connect");
    owner
        .execute_unprepared(
            "CREATE TABLE inventory_items (id TEXT PRIMARY KEY NOT NULL, updated_at TEXT NOT NULL)",
        )
        .await
        .expect("inventory item lock table should exist");

    let mut observer_options = ConnectOptions::new(url);
    observer_options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let observer = Database::connect(observer_options)
        .await
        .expect("independent observer database should connect");
    observer
        .execute_unprepared("PRAGMA busy_timeout = 0")
        .await
        .expect("observer lock timeout should be disabled");

    let transaction = owner.begin().await.expect("owner transaction should begin");
    require_current_snapshot(&transaction)
        .await
        .expect("quantity writer should reserve SQLite before reading the plan");

    let concurrent_write = observer
        .execute_unprepared("UPDATE inventory_items SET updated_at = updated_at WHERE 1 = 0")
        .await;
    assert!(
        concurrent_write.is_err(),
        "another SQLite writer must not pass the quantity snapshot reservation"
    );

    transaction
        .rollback()
        .await
        .expect("owner rollback should release the writer reservation");
    observer
        .execute_unprepared("UPDATE inventory_items SET updated_at = updated_at WHERE 1 = 0")
        .await
        .expect("another writer should proceed after rollback");

    drop(observer);
    drop(owner);
    let _ = std::fs::remove_file(path);
}
