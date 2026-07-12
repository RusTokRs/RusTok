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
                    .table(TenantLocales::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TenantLocales::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TenantLocales::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(TenantLocales::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TenantLocales::Name)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TenantLocales::NativeName)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TenantLocales::IsDefault)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(TenantLocales::IsEnabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(ColumnDef::new(TenantLocales::FallbackLocale).string_len(5))
                    .col(
                        ColumnDef::new(TenantLocales::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TenantLocales::Table, TenantLocales::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tenant_locales_unique")
                    .table(TenantLocales::Table)
                    .col(TenantLocales::TenantId)
                    .col(TenantLocales::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TenantLocales::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum TenantLocales {
    Table,
    Id,
    TenantId,
    Locale,
    Name,
    NativeName,
    IsDefault,
    IsEnabled,
    FallbackLocale,
    CreatedAt,
}
