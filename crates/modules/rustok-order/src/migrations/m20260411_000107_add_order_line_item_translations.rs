use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OrderLineItemTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OrderLineItemTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(OrderLineItemTranslations::OrderLineItemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderLineItemTranslations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderLineItemTranslations::Title)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderLineItemTranslations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(OrderLineItemTranslations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                OrderLineItemTranslations::Table,
                                OrderLineItemTranslations::OrderLineItemId,
                            )
                            .to(OrderLineItems::Table, OrderLineItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_order_line_item_translations_line_item")
                    .table(OrderLineItemTranslations::Table)
                    .col(OrderLineItemTranslations::OrderLineItemId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_order_line_item_translations_locale")
                    .table(OrderLineItemTranslations::Table)
                    .col(OrderLineItemTranslations::Locale)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_order_line_item_translations_unique")
                    .table(OrderLineItemTranslations::Table)
                    .col(OrderLineItemTranslations::OrderLineItemId)
                    .col(OrderLineItemTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(OrderLineItems::Table)
                    .drop_column(OrderLineItems::Title)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrderLineItems::Table)
                    .add_column(
                        ColumnDef::new(OrderLineItems::Title)
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
                    .table(OrderLineItemTranslations::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum OrderLineItems {
    Table,
    Id,
    Title,
}

#[derive(Iden)]
enum OrderLineItemTranslations {
    Table,
    Id,
    OrderLineItemId,
    Locale,
    Title,
    CreatedAt,
    UpdatedAt,
}
