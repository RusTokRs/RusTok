use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

/// New native movement provenance only. Never infer/backfill it from legacy counters,
/// cancelled status, reservation zero/deletion, provider receipts or caller metadata.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create().table(InventoryShipmentStockProofs::Table).if_not_exists()
                .col(ColumnDef::new(InventoryShipmentStockProofs::FulfillmentItemId).uuid().not_null().primary_key())
                .col(ColumnDef::new(InventoryShipmentStockProofs::TenantId).uuid().not_null())
                .col(ColumnDef::new(InventoryShipmentStockProofs::OrderId).uuid().not_null())
                .col(ColumnDef::new(InventoryShipmentStockProofs::FulfillmentId).uuid().not_null())
                .col(ColumnDef::new(InventoryShipmentStockProofs::OrderLineItemId).uuid().not_null())
                .col(ColumnDef::new(InventoryShipmentStockProofs::VariantId).uuid())
                .col(ColumnDef::new(InventoryShipmentStockProofs::FulfillmentQuantity).integer().not_null())
                .col(ColumnDef::new(InventoryShipmentStockProofs::InventoryItemId).uuid())
                .col(ColumnDef::new(InventoryShipmentStockProofs::ReservationId).uuid())
                .col(ColumnDef::new(InventoryShipmentStockProofs::LocationId).uuid())
                .col(ColumnDef::new(InventoryShipmentStockProofs::ShippedQuantity).integer().not_null())
                .col(ColumnDef::new(InventoryShipmentStockProofs::RestoredQuantity).integer().not_null())
                .col(ColumnDef::new(InventoryShipmentStockProofs::CreatedAt).timestamp_with_time_zone().not_null())
                .col(ColumnDef::new(InventoryShipmentStockProofs::UpdatedAt).timestamp_with_time_zone().not_null())
                .check(Expr::cust("fulfillment_quantity > 0 AND shipped_quantity > 0 AND shipped_quantity <= fulfillment_quantity AND restored_quantity >= 0 AND restored_quantity <= shipped_quantity"))
                .check(Expr::cust("(variant_id IS NULL AND inventory_item_id IS NULL AND reservation_id IS NULL AND location_id IS NULL) OR (variant_id IS NOT NULL AND inventory_item_id IS NOT NULL AND reservation_id IS NOT NULL AND location_id IS NOT NULL)"))
                .to_owned()
        ).await?;
        for (name, columns) in [
            (
                "idx_shipment_stock_proofs_tenant_fulfillment",
                vec![
                    InventoryShipmentStockProofs::TenantId,
                    InventoryShipmentStockProofs::FulfillmentId,
                ],
            ),
            (
                "idx_shipment_stock_proofs_tenant_line",
                vec![
                    InventoryShipmentStockProofs::TenantId,
                    InventoryShipmentStockProofs::OrderLineItemId,
                ],
            ),
        ] {
            let mut index = Index::create();
            index
                .name(name)
                .table(InventoryShipmentStockProofs::Table)
                .if_not_exists();
            for column in columns {
                index.col(column);
            }
            manager.create_index(index.to_owned()).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.has_table("inventory_shipment_stock_proofs").await? {
            let retained = manager
                .get_connection()
                .query_one_raw(Statement::from_string(
                    manager.get_database_backend(),
                    "SELECT 1 FROM inventory_shipment_stock_proofs LIMIT 1".to_string(),
                ))
                .await?;
            if retained.is_some() {
                return Err(DbErr::Custom("shipment movement provenance is retained; rollback cannot discard nonempty inventory_shipment_stock_proofs".into()));
            }
        }
        manager
            .drop_table(
                Table::drop()
                    .table(InventoryShipmentStockProofs::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum InventoryShipmentStockProofs {
    Table,
    FulfillmentItemId,
    TenantId,
    OrderId,
    FulfillmentId,
    OrderLineItemId,
    VariantId,
    FulfillmentQuantity,
    InventoryItemId,
    ReservationId,
    LocationId,
    ShippedQuantity,
    RestoredQuantity,
    CreatedAt,
    UpdatedAt,
}
