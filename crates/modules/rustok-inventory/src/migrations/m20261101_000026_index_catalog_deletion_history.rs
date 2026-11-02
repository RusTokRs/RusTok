//! Additive lookup indexes for the native empty-bookkeeping deletion guard.
//! No history, receipt identity, quantity or historical migration is rewritten.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const REFERENCE_INDEXES: &[(&str, &str, &str)] = &[
    (
        "idx_inventory_catalog_delete_returns",
        "inventory_return_restocks",
        "inventory_item_id",
    ),
    (
        "idx_inventory_catalog_delete_shipment_variant",
        "inventory_shipment_stock_proofs",
        "variant_id",
    ),
    (
        "idx_inventory_catalog_delete_shipment_item",
        "inventory_shipment_stock_proofs",
        "inventory_item_id",
    ),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Reservation item-reference lookup already has idx_reservation_items_inventory
        // from the original Inventory migration. These three references had no such index.
        for &(name, table, column) in REFERENCE_INDEXES {
            manager
                .create_index(
                    Index::create()
                        .name(name)
                        .table(Alias::new(table))
                        .col(Alias::new(column))
                        .if_not_exists()
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for &(name, table, _) in REFERENCE_INDEXES.iter().rev() {
            manager
                .drop_index(Index::drop().name(name).table(Alias::new(table)).to_owned())
                .await?;
        }
        Ok(())
    }
}
