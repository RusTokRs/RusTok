use sea_orm_migration::prelude::*;

use super::m20250101_000001_create_tenants::Tenants;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PriceLists::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PriceLists::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PriceLists::TenantId).uuid().not_null())
                    .col(ColumnDef::new(PriceLists::Name).string_len(100).not_null())
                    .col(ColumnDef::new(PriceLists::Description).text())
                    .col(
                        ColumnDef::new(PriceLists::Type)
                            .string_len(32)
                            .not_null()
                            .default("sale"),
                    )
                    .col(
                        ColumnDef::new(PriceLists::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .col(ColumnDef::new(PriceLists::StartsAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(PriceLists::EndsAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(PriceLists::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PriceLists::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PriceLists::Table, PriceLists::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Prices::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Prices::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Prices::VariantId).uuid().not_null())
                    .col(ColumnDef::new(Prices::PriceListId).uuid())
                    .col(
                        ColumnDef::new(Prices::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Prices::RegionId).uuid())
                    .col(ColumnDef::new(Prices::Amount).decimal_len(19, 4).not_null())
                    .col(ColumnDef::new(Prices::CompareAtAmount).decimal_len(19, 4))
                    .col(ColumnDef::new(Prices::MinQuantity).integer())
                    .col(ColumnDef::new(Prices::MaxQuantity).integer())
                    .col(
                        ColumnDef::new(Prices::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Prices::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Prices::Table, Prices::VariantId)
                            .to(ProductVariants::Table, ProductVariants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Prices::Table, Prices::PriceListId)
                            .to(PriceLists::Table, PriceLists::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Regions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Regions::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Regions::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Regions::Name).string_len(100).not_null())
                    .col(
                        ColumnDef::new(Regions::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Regions::TaxRate)
                            .decimal_len(5, 2)
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Regions::TaxIncluded)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Regions::Countries)
                            .json_binary()
                            .not_null()
                            .default("[]"),
                    )
                    .col(
                        ColumnDef::new(Regions::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Regions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Regions::Table, Regions::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_prices_region")
                    .from(Prices::Table, Prices::RegionId)
                    .to(Regions::Table, Regions::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_price_lists_tenant")
                    .table(PriceLists::Table)
                    .col(PriceLists::TenantId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_prices_variant")
                    .table(Prices::Table)
                    .col(Prices::VariantId)
                    .col(Prices::CurrencyCode)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_prices_list")
                    .table(Prices::Table)
                    .col(Prices::PriceListId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_regions_tenant")
                    .table(Regions::Table)
                    .col(Regions::TenantId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .table(Prices::Table)
                    .name("fk_prices_region")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Regions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Prices::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PriceLists::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum PriceLists {
    Table,
    Id,
    TenantId,
    Name,
    Description,
    Type,
    Status,
    StartsAt,
    EndsAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Prices {
    Table,
    Id,
    VariantId,
    PriceListId,
    CurrencyCode,
    RegionId,
    Amount,
    CompareAtAmount,
    MinQuantity,
    MaxQuantity,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Regions {
    Table,
    Id,
    TenantId,
    Name,
    CurrencyCode,
    TaxRate,
    TaxIncluded,
    Countries,
    Metadata,
    CreatedAt,
}

#[derive(Iden)]
enum ProductVariants {
    Table,
    Id,
}
