use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    .col(
                        ColumnDef::new(Regions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
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
        manager
            .create_index(
                Index::create()
                    .name("idx_regions_tenant_currency")
                    .table(Regions::Table)
                    .col(Regions::TenantId)
                    .col(Regions::CurrencyCode)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Regions::Table).to_owned())
            .await
    }
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
    UpdatedAt,
}
