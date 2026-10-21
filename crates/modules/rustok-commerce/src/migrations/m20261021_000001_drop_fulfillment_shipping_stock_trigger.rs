use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

/// Removes the fulfillment-shipping stock consumption trigger.
///
/// `m20260713_000007` installs `consume_inventory_on_fulfillment_shipping()` and the
/// `fulfillment_inventory_consume_on_ship` trigger. On growth of
/// `fulfillment_items.shipped_quantity` they read `order_line_items`, then read and write
/// `reservation_items` and `inventory_levels`: a cross-module business rule inside a trigger
/// and a second writer of tables the inventory module owns. The repository contract keeps such
/// rules in typed Rust code inside the owner's transaction.
///
/// The rule now lives in Rust, strictly wider than the trigger:
///
/// * `FulfillmentService` calls its `ShipmentStockPort` inside the shipment transaction;
/// * `rustok-commerce` composes that port with `rustok_inventory::shipment_stock`, which
///   applies the same ledger arithmetic (active reservation, quantity ≤ remaining reservation,
///   level reserved/stocked ≥ quantity, soft-delete at zero) and skips lines without a variant;
/// * cancelling a shipped fulfillment returns its undelivered units to stock and reopening it
///   takes them off again — neither was covered by the trigger.
///
/// A `FulfillmentService` built without a stock port fails the movement instead of silently
/// shipping without consuming stock.
///
/// `down()` restores the exact text installed by `m20260713_000007`.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for statement in uninstall_statements(manager.get_database_backend()) {
            manager
                .get_connection()
                .execute_unprepared(statement)
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        super::m20260713_000007_consume_inventory_on_fulfillment_shipping::Migration
            .up(manager)
            .await
    }
}

/// One statement per `execute_unprepared` call: SQLite drivers prepare a single statement at
/// a time.
fn uninstall_statements(backend: DatabaseBackend) -> &'static [&'static str] {
    match backend {
        DatabaseBackend::Postgres => &[
            "DROP TRIGGER IF EXISTS fulfillment_inventory_consume_on_ship ON fulfillment_items;",
            "DROP FUNCTION IF EXISTS consume_inventory_on_fulfillment_shipping();",
        ],
        DatabaseBackend::Sqlite => {
            &["DROP TRIGGER IF EXISTS fulfillment_inventory_consume_on_ship;"]
        }
        _ => &[],
    }
}
