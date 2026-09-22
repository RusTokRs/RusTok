use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .add_column(
                            ColumnDef::new(Orders::TaxTotal)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .add_column(
                            ColumnDef::new(Orders::TaxIncluded)
                                .boolean()
                                .not_null()
                                .default(false),
                        )
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .add_column(
                            ColumnDef::new(Orders::TaxTotal)
                                .decimal()
                                .not_null()
                                .default(0),
                        )
                        .add_column(
                            ColumnDef::new(Orders::TaxIncluded)
                                .boolean()
                                .not_null()
                                .default(false),
                        )
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(OrderTaxLines::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OrderTaxLines::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OrderTaxLines::OrderId).uuid().not_null())
                    .col(ColumnDef::new(OrderTaxLines::OrderLineItemId).uuid())
                    .col(ColumnDef::new(OrderTaxLines::ShippingOptionId).uuid())
                    .col(ColumnDef::new(OrderTaxLines::Description).string_len(255))
                    .col(ColumnDef::new(OrderTaxLines::Rate).decimal().not_null())
                    .col(ColumnDef::new(OrderTaxLines::Amount).decimal().not_null())
                    .col(
                        ColumnDef::new(OrderTaxLines::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderTaxLines::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(OrderTaxLines::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(OrderTaxLines::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(OrderTaxLines::Table, OrderTaxLines::OrderId)
                            .to(Orders::Table, Orders::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(OrderTaxLines::Table, OrderTaxLines::OrderLineItemId)
                            .to(OrderLineItems::Table, OrderLineItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_order_tax_lines_order")
                    .table(OrderTaxLines::Table)
                    .col(OrderTaxLines::OrderId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_order_tax_lines_line_item")
                    .table(OrderTaxLines::Table)
                    .col(OrderTaxLines::OrderLineItemId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_order_tax_lines_shipping_option")
                    .table(OrderTaxLines::Table)
                    .col(OrderTaxLines::ShippingOptionId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OrderTaxLines::Table).to_owned())
            .await?;
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .drop_column(Orders::TaxTotal)
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .drop_column(Orders::TaxIncluded)
                        .to_owned(),
                )
                .await
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(Orders::Table)
                        .drop_column(Orders::TaxTotal)
                        .drop_column(Orders::TaxIncluded)
                        .to_owned(),
                )
                .await
        }
    }
}

#[derive(Iden)]
enum Orders {
    Table,
    Id,
    TaxTotal,
    TaxIncluded,
}

#[derive(Iden)]
enum OrderLineItems {
    Table,
    Id,
}

#[derive(Iden)]
enum OrderTaxLines {
    Table,
    Id,
    OrderId,
    OrderLineItemId,
    ShippingOptionId,
    Description,
    Rate,
    Amount,
    CurrencyCode,
    Metadata,
    CreatedAt,
    UpdatedAt,
}
