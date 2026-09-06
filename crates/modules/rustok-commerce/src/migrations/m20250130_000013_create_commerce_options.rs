use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProductOptions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProductOptions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ProductOptions::ProductId).uuid().not_null())
                    .col(
                        ColumnDef::new(ProductOptions::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ProductOptions::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(ProductOptions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProductOptions::Table, ProductOptions::ProductId)
                            .to(Products::Table, Products::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProductOptionTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProductOptionTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProductOptionTranslations::OptionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductOptionTranslations::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductOptionTranslations::Title)
                            .string_len(100)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                ProductOptionTranslations::Table,
                                ProductOptionTranslations::OptionId,
                            )
                            .to(ProductOptions::Table, ProductOptions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProductOptionValues::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProductOptionValues::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProductOptionValues::OptionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductOptionValues::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ProductOptionValues::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProductOptionValues::Table, ProductOptionValues::OptionId)
                            .to(ProductOptions::Table, ProductOptions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProductOptionValueTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProductOptionValueTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProductOptionValueTranslations::ValueId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductOptionValueTranslations::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductOptionValueTranslations::Value)
                            .string_len(100)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                ProductOptionValueTranslations::Table,
                                ProductOptionValueTranslations::ValueId,
                            )
                            .to(ProductOptionValues::Table, ProductOptionValues::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_product_options_product")
                    .table(ProductOptions::Table)
                    .col(ProductOptions::ProductId)
                    .col(ProductOptions::Position)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_option_trans_unique")
                    .table(ProductOptionTranslations::Table)
                    .col(ProductOptionTranslations::OptionId)
                    .col(ProductOptionTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_option_values_option")
                    .table(ProductOptionValues::Table)
                    .col(ProductOptionValues::OptionId)
                    .col(ProductOptionValues::Position)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_option_value_trans_unique")
                    .table(ProductOptionValueTranslations::Table)
                    .col(ProductOptionValueTranslations::ValueId)
                    .col(ProductOptionValueTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ProductOptionValueTranslations::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(ProductOptionValues::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(ProductOptionTranslations::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(ProductOptions::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum ProductOptions {
    Table,
    Id,
    ProductId,
    Position,
    Metadata,
    CreatedAt,
}

#[derive(Iden)]
enum ProductOptionTranslations {
    Table,
    Id,
    OptionId,
    Locale,
    Title,
}

#[derive(Iden)]
enum ProductOptionValues {
    Table,
    Id,
    OptionId,
    Position,
    Metadata,
}

#[derive(Iden)]
enum ProductOptionValueTranslations {
    Table,
    Id,
    ValueId,
    Locale,
    Value,
}

#[derive(Iden)]
enum Products {
    Table,
    Id,
}
