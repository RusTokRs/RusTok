use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OrderReturns::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OrderReturns::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OrderReturns::TenantId).uuid().not_null())
                    .col(ColumnDef::new(OrderReturns::OrderId).uuid().not_null())
                    .col(ColumnDef::new(OrderReturns::Reason).string_len(255))
                    .col(ColumnDef::new(OrderReturns::Note).text())
                    .col(
                        ColumnDef::new(OrderReturns::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderReturns::Metadata)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderReturns::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderReturns::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OrderReturns::CompletedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(OrderReturns::CancelledAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_order_returns_order_id")
                            .from(OrderReturns::Table, OrderReturns::OrderId)
                            .to(Orders::Table, Orders::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OrderReturns::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OrderReturns {
    Table,
    Id,
    TenantId,
    OrderId,
    Reason,
    Note,
    Status,
    Metadata,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
    CancelledAt,
}

#[derive(DeriveIden)]
enum Orders {
    Table,
    Id,
}
