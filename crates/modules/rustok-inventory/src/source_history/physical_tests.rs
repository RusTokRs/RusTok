//! Isolated storage definition for the real common shipment/restore/return event plan.
//! Definitions only, never executed in the agent handoff.
use super::*;
use crate::stock_events::{StockEventPlan, StockLevelChange};
use sea_orm::{Database, Schema, TransactionTrait};
use std::time::Duration;

#[tokio::test]
async fn source_physical_multi_location_equal_availability_advances_once_and_preparation_does_not()
{
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let backend = DbBackend::Sqlite;
    // Only the narrow source-owned read columns are doubled; the source migration and root
    // outbox storage are real. Full Product/native transactions live in Commerce regressions.
    for sql in [
        "CREATE TABLE products (id BLOB PRIMARY KEY, tenant_id BLOB NOT NULL)",
        "CREATE TABLE product_variants (id BLOB PRIMARY KEY, product_id BLOB NOT NULL, tenant_id BLOB NOT NULL, sku TEXT, inventory_policy TEXT NOT NULL DEFAULT 'deny')",
        "CREATE TABLE inventory_items (id BLOB PRIMARY KEY, variant_id BLOB NOT NULL)",
        "CREATE TABLE stock_locations (id BLOB PRIMARY KEY, tenant_id BLOB NOT NULL)",
        "CREATE TABLE inventory_levels (id BLOB PRIMARY KEY, inventory_item_id BLOB NOT NULL, location_id BLOB NOT NULL, stocked_quantity INTEGER NOT NULL, reserved_quantity INTEGER NOT NULL, incoming_quantity INTEGER NOT NULL)",
    ] {
        db.execute_raw(Statement::from_string(backend, sql))
            .await
            .unwrap();
    }
    let schema = Schema::new(backend);
    db.execute_raw(backend.build(&schema.create_table_from_entity(rustok_outbox::SysEvents)))
        .await
        .unwrap();
    crate::migrations::migrations()
        .into_iter()
        .find(|m| m.name() == "m20261101_000027_create_inventory_source_history")
        .unwrap()
        .up(&sea_orm_migration::SchemaManager::new(&db))
        .await
        .unwrap();
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let product = Uuid::new_v4();
    let variant = Uuid::new_v4();
    let item = Uuid::new_v4();
    let locations = [Uuid::new_v4(), Uuid::new_v4()];
    let levels = [Uuid::new_v4(), Uuid::new_v4()];
    for (sql, values) in [
        (
            "INSERT INTO products VALUES (?, ?)",
            vec![product.into(), tenant.into()],
        ),
        (
            "INSERT INTO product_variants (id, product_id, tenant_id) VALUES (?, ?, ?)",
            vec![variant.into(), product.into(), tenant.into()],
        ),
        (
            "INSERT INTO inventory_items VALUES (?, ?)",
            vec![item.into(), variant.into()],
        ),
        (
            "INSERT INTO stock_locations VALUES (?, ?)",
            vec![locations[0].into(), tenant.into()],
        ),
        (
            "INSERT INTO stock_locations VALUES (?, ?)",
            vec![locations[1].into(), tenant.into()],
        ),
        (
            "INSERT INTO inventory_levels VALUES (?, ?, ?, 10, 4, 0)",
            vec![levels[0].into(), item.into(), locations[0].into()],
        ),
        (
            "INSERT INTO inventory_levels VALUES (?, ?, ?, 4, 1, 0)",
            vec![levels[1].into(), item.into(), locations[1].into()],
        ),
    ] {
        db.execute_raw(Statement::from_sql_and_values(backend, sql, values))
            .await
            .unwrap();
    }
    let context = PortContext::new(
        tenant.to_string(),
        rustok_api::PortActor::user(actor.to_string()),
        "en",
        "physical-source",
    )
    .with_claim(Permission::INVENTORY_MANAGE.to_string())
    .with_deadline(Duration::from_secs(10))
    .with_idempotency_key("diagnostic-only");
    let history = InventorySourceHistory::new(db.clone());
    let baseline = Uuid::new_v4();
    history.start_baseline(&context, baseline).await.unwrap();
    history
        .advance_baseline(
            &context,
            InventoryBaselinePageRequest {
                baseline_id: baseline,
                page_id: Uuid::new_v4(),
                after_variant_id: None,
                limit: 1,
            },
        )
        .await
        .unwrap();
    let original = history
        .load(&context, baseline, &[variant])
        .await
        .unwrap()
        .remove(0);
    let changes = vec![
        StockLevelChange {
            inventory_item_id: item,
            variant_id: variant,
            product_id: product,
            location_id: locations[0],
            old_stocked: 10,
            old_reserved: 4,
            new_stocked: 8,
            new_reserved: 2,
        },
        StockLevelChange {
            inventory_item_id: item,
            variant_id: variant,
            product_id: product,
            location_id: locations[1],
            old_stocked: 4,
            old_reserved: 1,
            new_stocked: 3,
            new_reserved: 0,
        },
    ];
    let txn = db.begin().await.unwrap();
    let _prepared =
        StockEventPlan::prepare(&txn, &context, "source_physical_test", changes.clone())
            .await
            .unwrap();
    txn.rollback().await.unwrap();
    assert_eq!(
        history.load(&context, baseline, &[variant]).await.unwrap()[0],
        original
    );
    let txn = db.begin().await.unwrap();
    let plan = StockEventPlan::prepare(&txn, &context, "source_physical_test", changes)
        .await
        .unwrap();
    for (level, stocked, reserved) in [(levels[0], 8, 2), (levels[1], 3, 0)] {
        txn.execute_raw(Statement::from_sql_and_values(
            backend,
            "UPDATE inventory_levels SET stocked_quantity = ?, reserved_quantity = ? WHERE id = ?",
            vec![stocked.into(), reserved.into(), level.into()],
        ))
        .await
        .unwrap();
    }
    plan.publish(&txn, &context, "source_physical_test")
        .await
        .unwrap();
    txn.commit().await.unwrap();
    let current = history
        .load(&context, baseline, &[variant])
        .await
        .unwrap()
        .remove(0);
    assert_eq!(current.available, original.available);
    assert_eq!(current.source_version, 2);
    assert_eq!((current.stocked, current.reserved), (11, 2));
    let saved = rustok_outbox::SysEvents::find_by_id(current.change_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let envelope: rustok_events::ContractEventEnvelope =
        serde_json::from_value(saved.payload).unwrap();
    assert_eq!(envelope.id(), current.change_id);
    assert_eq!(envelope.actor_id(), Some(actor));
    assert_eq!(
        envelope.payload().unwrap(),
        &rustok_events::ContractEventPayload::InventorySource(
            rustok_events::InventorySourceEvent::Changed {
                baseline_id: baseline,
                variant_id: variant,
                source_version: 2,
            }
        )
    );
    assert_eq!(
        history
            .changes(&context, baseline, variant, 0, 128)
            .await
            .unwrap()
            .len(),
        2
    );
}
