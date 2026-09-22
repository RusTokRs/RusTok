use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CartLineItemTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartLineItemTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemTranslations::CartLineItemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemTranslations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemTranslations::Title)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartLineItemTranslations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CartLineItemTranslations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                CartLineItemTranslations::Table,
                                CartLineItemTranslations::CartLineItemId,
                            )
                            .to(CartLineItems::Table, CartLineItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_line_item_translations_line_item")
                    .table(CartLineItemTranslations::Table)
                    .col(CartLineItemTranslations::CartLineItemId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_cart_line_item_translations_locale")
                    .table(CartLineItemTranslations::Table)
                    .col(CartLineItemTranslations::Locale)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_cart_line_item_translations_unique")
                    .table(CartLineItemTranslations::Table)
                    .col(CartLineItemTranslations::CartLineItemId)
                    .col(CartLineItemTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CartLineItems::Table)
                    .drop_column(CartLineItems::Title)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CartLineItems::Table)
                    .add_column(
                        ColumnDef::new(CartLineItems::Title)
                            .string_len(255)
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(CartLineItemTranslations::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum CartLineItems {
    Table,
    Id,
    Title,
}

#[derive(Iden)]
enum CartLineItemTranslations {
    Table,
    Id,
    CartLineItemId,
    Locale,
    Title,
    CreatedAt,
    UpdatedAt,
}
