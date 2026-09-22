use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RegionCountryTaxPolicies::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RegionCountryTaxPolicies::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RegionCountryTaxPolicies::RegionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegionCountryTaxPolicies::CountryCode)
                            .string_len(2)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegionCountryTaxPolicies::TaxRate)
                            .decimal_len(5, 2)
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RegionCountryTaxPolicies::TaxIncluded)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_region_country_tax_policies_region")
                            .from(
                                RegionCountryTaxPolicies::Table,
                                RegionCountryTaxPolicies::RegionId,
                            )
                            .to(Regions::Table, Regions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_region_country_tax_policies_unique")
                    .table(RegionCountryTaxPolicies::Table)
                    .col(RegionCountryTaxPolicies::RegionId)
                    .col(RegionCountryTaxPolicies::CountryCode)
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
                    .table(RegionCountryTaxPolicies::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Regions {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum RegionCountryTaxPolicies {
    Table,
    Id,
    RegionId,
    CountryCode,
    TaxRate,
    TaxIncluded,
}
