use sea_orm_migration::prelude::*;

/// Inventory-owned ledger of restocked return items. The primary key on `return_item_id` is the
/// idempotency key of `return_restock::restock_returned_items`. No foreign keys: the ledger
/// records facts about rows owned by other modules (return items, order lines) without
/// coupling their lifecycles to inventory storage.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(InventoryReturnRestocks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InventoryReturnRestocks::ReturnItemId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(InventoryReturnRestocks::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReturnRestocks::ReturnId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReturnRestocks::OrderLineItemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReturnRestocks::InventoryItemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReturnRestocks::LocationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReturnRestocks::Quantity)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryReturnRestocks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .check(Expr::col(InventoryReturnRestocks::Quantity).gt(0))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_inventory_return_restocks_tenant_return")
                    .table(InventoryReturnRestocks::Table)
                    .col(InventoryReturnRestocks::TenantId)
                    .col(InventoryReturnRestocks::ReturnId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(InventoryReturnRestocks::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum InventoryReturnRestocks {
    Table,
    ReturnItemId,
    TenantId,
    ReturnId,
    OrderLineItemId,
    InventoryItemId,
    LocationId,
    Quantity,
    CreatedAt,
}
