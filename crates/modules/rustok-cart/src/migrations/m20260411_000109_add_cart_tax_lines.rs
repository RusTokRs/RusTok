use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .add_column(
                        ColumnDef::new(Carts::TaxTotal)
                            .decimal()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CartTaxLines::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartTaxLines::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CartTaxLines::CartId).uuid().not_null())
                    .col(ColumnDef::new(CartTaxLines::CartLineItemId).uuid())
                    .col(ColumnDef::new(CartTaxLines::ShippingOptionId).uuid())
                    .col(ColumnDef::new(CartTaxLines::Description).string_len(255))
                    .col(ColumnDef::new(CartTaxLines::Rate).decimal().not_null())
                    .col(ColumnDef::new(CartTaxLines::Amount).decimal().not_null())
                    .col(
                        ColumnDef::new(CartTaxLines::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartTaxLines::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(CartTaxLines::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CartTaxLines::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CartTaxLines::Table, CartTaxLines::CartId)
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CartTaxLines::Table, CartTaxLines::CartLineItemId)
                            .to(CartLineItems::Table, CartLineItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_tax_lines_cart")
                    .table(CartTaxLines::Table)
                    .col(CartTaxLines::CartId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_cart_tax_lines_line_item")
                    .table(CartTaxLines::Table)
                    .col(CartTaxLines::CartLineItemId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_cart_tax_lines_shipping_option")
                    .table(CartTaxLines::Table)
                    .col(CartTaxLines::ShippingOptionId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CartTaxLines::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .drop_column(Carts::TaxTotal)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Carts {
    Table,
    Id,
    TaxTotal,
}

#[derive(Iden)]
enum CartLineItems {
    Table,
    Id,
}

#[derive(Iden)]
enum CartTaxLines {
    Table,
    Id,
    CartId,
    CartLineItemId,
    ShippingOptionId,
    Description,
    Rate,
    Amount,
    CurrencyCode,
    Metadata,
    CreatedAt,
    UpdatedAt,
}
