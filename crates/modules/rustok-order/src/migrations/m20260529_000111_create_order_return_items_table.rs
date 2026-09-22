use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OrderReturnItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OrderReturnItems::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OrderReturnItems::TenantId).uuid().not_null())
                    .col(ColumnDef::new(OrderReturnItems::ReturnId).uuid().not_null())
                    .col(ColumnDef::new(OrderReturnItems::OrderId).uuid().not_null())
                    .col(
                        ColumnDef::new(OrderReturnItems::LineItemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderReturnItems::Quantity)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OrderReturnItems::Reason).string_len(255))
                    .col(ColumnDef::new(OrderReturnItems::Note).text())
                    .col(
                        ColumnDef::new(OrderReturnItems::Metadata)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderReturnItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderReturnItems::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_order_return_items_return_id")
                            .from(OrderReturnItems::Table, OrderReturnItems::ReturnId)
                            .to(OrderReturns::Table, OrderReturns::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_order_return_items_order_id")
                            .from(OrderReturnItems::Table, OrderReturnItems::OrderId)
                            .to(Orders::Table, Orders::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_order_return_items_line_item_id")
                            .from(OrderReturnItems::Table, OrderReturnItems::LineItemId)
                            .to(OrderLineItems::Table, OrderLineItems::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_order_return_items_return_id")
                    .table(OrderReturnItems::Table)
                    .col(OrderReturnItems::ReturnId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_order_return_items_order_id")
                    .table(OrderReturnItems::Table)
                    .col(OrderReturnItems::OrderId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_order_return_items_line_item_id")
                    .table(OrderReturnItems::Table)
                    .col(OrderReturnItems::LineItemId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OrderReturnItems::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OrderReturnItems {
    Table,
    Id,
    TenantId,
    ReturnId,
    OrderId,
    LineItemId,
    Quantity,
    Reason,
    Note,
    Metadata,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum OrderReturns {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Orders {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum OrderLineItems {
    Table,
    Id,
}
