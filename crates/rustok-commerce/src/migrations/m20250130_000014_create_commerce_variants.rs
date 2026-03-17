use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProductVariants::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProductVariants::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ProductVariants::ProductId).uuid().not_null())
                    .col(ColumnDef::new(ProductVariants::Sku).string_len(100))
                    .col(ColumnDef::new(ProductVariants::Barcode).string_len(100))
                    .col(ColumnDef::new(ProductVariants::Ean).string_len(20))
                    .col(ColumnDef::new(ProductVariants::Upc).string_len(20))
                    .col(ColumnDef::new(ProductVariants::Weight).integer())
                    .col(ColumnDef::new(ProductVariants::Length).integer())
                    .col(ColumnDef::new(ProductVariants::Height).integer())
                    .col(ColumnDef::new(ProductVariants::Width).integer())
                    .col(ColumnDef::new(ProductVariants::HsCode).string_len(20))
                    .col(ColumnDef::new(ProductVariants::OriginCountry).string_len(2))
                    .col(ColumnDef::new(ProductVariants::MidCode).string_len(50))
                    .col(
                        ColumnDef::new(ProductVariants::ManageInventory)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(ProductVariants::AllowBackorder)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(ProductVariants::VariantRank)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ProductVariants::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(ProductVariants::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ProductVariants::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(ProductVariants::DeletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProductVariants::Table, ProductVariants::ProductId)
                            .to(Products::Table, Products::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProductVariantTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProductVariantTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProductVariantTranslations::VariantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductVariantTranslations::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ProductVariantTranslations::Title).string_len(255))
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                ProductVariantTranslations::Table,
                                ProductVariantTranslations::VariantId,
                            )
                            .to(ProductVariants::Table, ProductVariants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(VariantOptionValues::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(VariantOptionValues::VariantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VariantOptionValues::OptionValueId)
                            .uuid()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(VariantOptionValues::VariantId)
                            .col(VariantOptionValues::OptionValueId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(VariantOptionValues::Table, VariantOptionValues::VariantId)
                            .to(ProductVariants::Table, ProductVariants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                VariantOptionValues::Table,
                                VariantOptionValues::OptionValueId,
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
                    .name("idx_variants_product")
                    .table(ProductVariants::Table)
                    .col(ProductVariants::ProductId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_variants_sku")
                    .table(ProductVariants::Table)
                    .col(ProductVariants::Sku)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_variants_barcode")
                    .table(ProductVariants::Table)
                    .col(ProductVariants::Barcode)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_variant_trans_unique")
                    .table(ProductVariantTranslations::Table)
                    .col(ProductVariantTranslations::VariantId)
                    .col(ProductVariantTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(VariantOptionValues::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(ProductVariantTranslations::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(ProductVariants::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum ProductVariants {
    Table,
    Id,
    ProductId,
    Sku,
    Barcode,
    Ean,
    Upc,
    Weight,
    Length,
    Height,
    Width,
    HsCode,
    OriginCountry,
    MidCode,
    ManageInventory,
    AllowBackorder,
    VariantRank,
    Metadata,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

#[derive(Iden)]
enum ProductVariantTranslations {
    Table,
    Id,
    VariantId,
    Locale,
    Title,
}

#[derive(Iden)]
enum VariantOptionValues {
    Table,
    VariantId,
    OptionValueId,
}

#[derive(Iden)]
enum Products {
    Table,
    Id,
}

#[derive(Iden)]
enum ProductOptionValues {
    Table,
    Id,
}
