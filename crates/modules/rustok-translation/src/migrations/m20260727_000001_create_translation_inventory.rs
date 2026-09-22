use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(InventoryResources::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InventoryResources::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(InventoryResources::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryResources::OwnerSlug)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryResources::ResourceKind)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryResources::ResourceId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryResources::SubresourceKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryResources::ResourceRevision)
                            .string_len(256)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryResources::Lifecycle)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryResources::ObservedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_inventory_resources_tenant")
                            .from(InventoryResources::Table, InventoryResources::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_translation_inventory_resource_identity")
                    .table(InventoryResources::Table)
                    .col(InventoryResources::TenantId)
                    .col(InventoryResources::OwnerSlug)
                    .col(InventoryResources::ResourceKind)
                    .col(InventoryResources::ResourceId)
                    .col(InventoryResources::SubresourceKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProviderCheckpoints::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProviderCheckpoints::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProviderCheckpoints::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderCheckpoints::OwnerSlug)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderCheckpoints::ResourceKind)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ProviderCheckpoints::Cursor).string_len(512))
                    .col(
                        ColumnDef::new(ProviderCheckpoints::Revision)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ProviderCheckpoints::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_provider_checkpoints_tenant")
                            .from(ProviderCheckpoints::Table, ProviderCheckpoints::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_translation_provider_checkpoint")
                    .table(ProviderCheckpoints::Table)
                    .col(ProviderCheckpoints::TenantId)
                    .col(ProviderCheckpoints::OwnerSlug)
                    .col(ProviderCheckpoints::ResourceKind)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: projection checkpoints are recoverable, but silently
        // deleting them during rollback can hide inventory lag.
        Ok(())
    }
}

#[derive(Iden)]
enum InventoryResources {
    #[iden = "translation_inventory_resources"]
    Table,
    Id,
    TenantId,
    OwnerSlug,
    ResourceKind,
    ResourceId,
    SubresourceKey,
    ResourceRevision,
    Lifecycle,
    ObservedAt,
}

#[derive(Iden)]
enum ProviderCheckpoints {
    #[iden = "translation_provider_checkpoints"]
    Table,
    Id,
    TenantId,
    OwnerSlug,
    ResourceKind,
    Cursor,
    Revision,
    UpdatedAt,
}

#[derive(Iden)]
enum Tenants {
    #[iden = "tenants"]
    Table,
    Id,
}
