use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OrderAdjustments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OrderAdjustments::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OrderAdjustments::OrderId).uuid().not_null())
                    .col(ColumnDef::new(OrderAdjustments::OrderLineItemId).uuid())
                    .col(
                        ColumnDef::new(OrderAdjustments::SourceType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(OrderAdjustments::SourceId).string_len(191))
                    .col(
                        ColumnDef::new(OrderAdjustments::Amount)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderAdjustments::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderAdjustments::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(OrderAdjustments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(OrderAdjustments::Table, OrderAdjustments::OrderId)
                            .to(Orders::Table, Orders::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(OrderAdjustments::Table, OrderAdjustments::OrderLineItemId)
                            .to(OrderLineItems::Table, OrderLineItems::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_order_adjustments_order")
                    .table(OrderAdjustments::Table)
                    .col(OrderAdjustments::OrderId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_order_adjustments_line_item")
                    .table(OrderAdjustments::Table)
                    .col(OrderAdjustments::OrderLineItemId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OrderAdjustments::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum OrderAdjustments {
    Table,
    Id,
    OrderId,
    OrderLineItemId,
    SourceType,
    SourceId,
    Amount,
    CurrencyCode,
    Metadata,
    CreatedAt,
}

#[derive(Iden)]
enum Orders {
    Table,
    Id,
}

#[derive(Iden)]
enum OrderLineItems {
    Table,
    Id,
}
