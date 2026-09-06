use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Carts::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Carts::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Carts::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Carts::CustomerId).uuid())
                    .col(ColumnDef::new(Carts::Email).string_len(255))
                    .col(ColumnDef::new(Carts::Status).string_len(32).not_null())
                    .col(ColumnDef::new(Carts::CurrencyCode).string_len(3).not_null())
                    .col(ColumnDef::new(Carts::TotalAmount).decimal().not_null())
                    .col(
                        ColumnDef::new(Carts::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Carts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Carts::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Carts::CompletedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CartLineItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartLineItems::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CartLineItems::CartId).uuid().not_null())
                    .col(ColumnDef::new(CartLineItems::ProductId).uuid())
                    .col(ColumnDef::new(CartLineItems::VariantId).uuid())
                    .col(ColumnDef::new(CartLineItems::Sku).string_len(100))
                    .col(
                        ColumnDef::new(CartLineItems::Title)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(CartLineItems::Quantity).integer().not_null())
                    .col(
                        ColumnDef::new(CartLineItems::UnitPrice)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItems::TotalPrice)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItems::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItems::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(CartLineItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CartLineItems::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CartLineItems::Table, CartLineItems::CartId)
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_carts_tenant_status")
                    .table(Carts::Table)
                    .col(Carts::TenantId)
                    .col(Carts::Status)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_cart_line_items_cart")
                    .table(CartLineItems::Table)
                    .col(CartLineItems::CartId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CartLineItems::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Carts::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Carts {
    Table,
    Id,
    TenantId,
    CustomerId,
    Email,
    Status,
    CurrencyCode,
    TotalAmount,
    Metadata,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(Iden)]
enum CartLineItems {
    Table,
    Id,
    CartId,
    ProductId,
    VariantId,
    Sku,
    Title,
    Quantity,
    UnitPrice,
    TotalPrice,
    CurrencyCode,
    Metadata,
    CreatedAt,
    UpdatedAt,
}
