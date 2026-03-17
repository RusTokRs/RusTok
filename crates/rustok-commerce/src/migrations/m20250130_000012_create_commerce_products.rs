use super::shared::*;
use sea_orm_migration::prelude::*;




#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Products::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Products::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Products::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(Products::Status)
                            .string_len(32)
                            .not_null()
                            .default("draft"),
                    )
                    .col(
                        ColumnDef::new(Products::IsGiftCard)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Products::Discountable)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(ColumnDef::new(Products::Weight).integer())
                    .col(ColumnDef::new(Products::Length).integer())
                    .col(ColumnDef::new(Products::Height).integer())
                    .col(ColumnDef::new(Products::Width).integer())
                    .col(ColumnDef::new(Products::HsCode).string_len(20))
                    .col(ColumnDef::new(Products::OriginCountry).string_len(2))
                    .col(ColumnDef::new(Products::MidCode).string_len(50))
                    .col(ColumnDef::new(Products::ExternalId).string_len(100))
                    .col(
                        ColumnDef::new(Products::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Products::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Products::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Products::PublishedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Products::DeletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Products::Table, Products::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProductTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProductTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProductTranslations::ProductId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductTranslations::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductTranslations::Title)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ProductTranslations::Subtitle).string_len(255))
                    .col(
                        ColumnDef::new(ProductTranslations::Handle)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ProductTranslations::Description).text())
                    .col(ColumnDef::new(ProductTranslations::Material).string_len(255))
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProductTranslations::Table, ProductTranslations::ProductId)
                            .to(Products::Table, Products::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProductImages::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProductImages::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ProductImages::ProductId).uuid().not_null())
                    .col(ColumnDef::new(ProductImages::MediaId).uuid())
                    .col(
                        ColumnDef::new(ProductImages::Url)
                            .string_len(500)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductImages::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ProductImages::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(ProductImages::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProductImages::Table, ProductImages::ProductId)
                            .to(Products::Table, Products::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProductImages::Table, ProductImages::MediaId)
                            .to(Media::Table, Media::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProductImageTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProductImageTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProductImageTranslations::ImageId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductImageTranslations::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ProductImageTranslations::AltText).string_len(255))
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                ProductImageTranslations::Table,
                                ProductImageTranslations::ImageId,
                            )
                            .to(ProductImages::Table, ProductImages::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_products_tenant")
                    .table(Products::Table)
                    .col(Products::TenantId)
                    .col(Products::Status)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_products_external")
                    .table(Products::Table)
                    .col(Products::TenantId)
                    .col(Products::ExternalId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_product_trans_unique")
                    .table(ProductTranslations::Table)
                    .col(ProductTranslations::ProductId)
                    .col(ProductTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_product_trans_handle")
                    .table(ProductTranslations::Table)
                    .col(ProductTranslations::Locale)
                    .col(ProductTranslations::Handle)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_product_images_product")
                    .table(ProductImages::Table)
                    .col(ProductImages::ProductId)
                    .col(ProductImages::Position)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_product_image_trans_unique")
                    .table(ProductImageTranslations::Table)
                    .col(ProductImageTranslations::ImageId)
                    .col(ProductImageTranslations::Locale)
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
                    .table(ProductImageTranslations::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(ProductImages::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ProductTranslations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Products::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Products {
    Table,
    Id,
    TenantId,
    Status,
    IsGiftCard,
    Discountable,
    Weight,
    Length,
    Height,
    Width,
    HsCode,
    OriginCountry,
    MidCode,
    ExternalId,
    Metadata,
    CreatedAt,
    UpdatedAt,
    PublishedAt,
    DeletedAt,
}

#[derive(Iden)]
enum ProductTranslations {
    Table,
    Id,
    ProductId,
    Locale,
    Title,
    Subtitle,
    Handle,
    Description,
    Material,
}

#[derive(Iden)]
enum ProductImages {
    Table,
    Id,
    ProductId,
    MediaId,
    Url,
    Position,
    Metadata,
    CreatedAt,
}

#[derive(Iden)]
enum ProductImageTranslations {
    Table,
    Id,
    ImageId,
    Locale,
    AltText,
}
