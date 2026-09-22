use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CartAdjustments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartAdjustments::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CartAdjustments::CartId).uuid().not_null())
                    .col(ColumnDef::new(CartAdjustments::CartLineItemId).uuid())
                    .col(
                        ColumnDef::new(CartAdjustments::SourceType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(CartAdjustments::SourceId).string_len(191))
                    .col(ColumnDef::new(CartAdjustments::Amount).decimal().not_null())
                    .col(
                        ColumnDef::new(CartAdjustments::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartAdjustments::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(CartAdjustments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CartAdjustments::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CartAdjustments::Table, CartAdjustments::CartId)
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CartAdjustments::Table, CartAdjustments::CartLineItemId)
                            .to(CartLineItems::Table, CartLineItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_adjustments_cart")
                    .table(CartAdjustments::Table)
                    .col(CartAdjustments::CartId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_cart_adjustments_line_item")
                    .table(CartAdjustments::Table)
                    .col(CartAdjustments::CartLineItemId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CartAdjustments::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum CartAdjustments {
    Table,
    Id,
    CartId,
    CartLineItemId,
    SourceType,
    SourceId,
    Amount,
    CurrencyCode,
    Metadata,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Carts {
    Table,
    Id,
}

#[derive(Iden)]
enum CartLineItems {
    Table,
    Id,
}
