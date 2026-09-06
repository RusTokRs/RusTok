use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OrderChanges::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OrderChanges::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OrderChanges::TenantId).uuid().not_null())
                    .col(ColumnDef::new(OrderChanges::OrderId).uuid().not_null())
                    .col(ColumnDef::new(OrderChanges::CreatedBy).uuid().not_null())
                    .col(
                        ColumnDef::new(OrderChanges::ChangeType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderChanges::Status)
                            .string_len(32)
                            .not_null()
                            .default("pending"),
                    )
                    .col(ColumnDef::new(OrderChanges::Description).text())
                    .col(
                        ColumnDef::new(OrderChanges::Preview)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'")),
                    )
                    .col(
                        ColumnDef::new(OrderChanges::Metadata)
                            .json_binary()
                            .not_null()
                            .default(Expr::cust("'{}'")),
                    )
                    .col(
                        ColumnDef::new(OrderChanges::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(OrderChanges::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(OrderChanges::AppliedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(OrderChanges::CancelledAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_order_changes_order")
                            .from(OrderChanges::Table, OrderChanges::OrderId)
                            .to(Orders::Table, Orders::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_order_changes_tenant_order")
                    .table(OrderChanges::Table)
                    .col(OrderChanges::TenantId)
                    .col(OrderChanges::OrderId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_order_changes_status")
                    .table(OrderChanges::Table)
                    .col(OrderChanges::TenantId)
                    .col(OrderChanges::Status)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OrderChanges::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OrderChanges {
    Table,
    Id,
    TenantId,
    OrderId,
    CreatedBy,
    ChangeType,
    Status,
    Description,
    Preview,
    Metadata,
    CreatedAt,
    UpdatedAt,
    AppliedAt,
    CancelledAt,
}

#[derive(DeriveIden)]
enum Orders {
    Table,
    Id,
}
