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
                    .table(TenantModules::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TenantModules::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TenantModules::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(TenantModules::ModuleSlug)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TenantModules::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(TenantModules::Settings)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(TenantModules::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TenantModules::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tenant_modules_tenant_id")
                            .from(TenantModules::Table, TenantModules::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tenant_modules_unique")
                    .table(TenantModules::Table)
                    .col(TenantModules::TenantId)
                    .col(TenantModules::ModuleSlug)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ModuleStaticTenantLifecycle::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ModuleStaticTenantLifecycle::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModuleStaticTenantLifecycle::ModuleSlug)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModuleStaticTenantLifecycle::Revision)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(ModuleStaticTenantLifecycle::ActiveIdempotencyKey).uuid())
                    .col(
                        ColumnDef::new(ModuleStaticTenantLifecycle::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ModuleStaticTenantLifecycle::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(ModuleStaticTenantLifecycle::TenantId)
                            .col(ModuleStaticTenantLifecycle::ModuleSlug),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_module_static_tenant_lifecycle_tenant_id")
                            .from(
                                ModuleStaticTenantLifecycle::Table,
                                ModuleStaticTenantLifecycle::TenantId,
                            )
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ModuleStaticTenantLifecycle::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(TenantModules::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum TenantModules {
    Table,
    Id,
    TenantId,
    ModuleSlug,
    Enabled,
    Settings,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum ModuleStaticTenantLifecycle {
    Table,
    TenantId,
    ModuleSlug,
    Revision,
    ActiveIdempotencyKey,
    CreatedAt,
    UpdatedAt,
}
