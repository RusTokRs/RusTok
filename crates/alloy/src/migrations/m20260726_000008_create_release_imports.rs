use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AlloyReleaseImports::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AlloyReleaseImports::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AlloyReleaseImports::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyReleaseImports::IdempotencyKey)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyReleaseImports::RequestDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyReleaseImports::ScriptId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyReleaseImports::ParentReleaseSlug)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyReleaseImports::ParentReleaseVersion)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyReleaseImports::ParentReleaseDigest)
                            .string_len(71)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlloyReleaseImports::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("uidx_alloy_release_imports_tenant_idempotency")
                            .col(AlloyReleaseImports::TenantId)
                            .col(AlloyReleaseImports::IdempotencyKey),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("uidx_alloy_release_imports_script")
                            .col(AlloyReleaseImports::ScriptId),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AlloyReleaseImports::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AlloyReleaseImports {
    Table,
    Id,
    TenantId,
    IdempotencyKey,
    RequestDigest,
    ScriptId,
    ParentReleaseSlug,
    ParentReleaseVersion,
    ParentReleaseDigest,
    CreatedAt,
}
