use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RbacLocalizedPresentations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::ResourceKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::ResourceKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::Name)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(RbacLocalizedPresentations::Description).text())
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::CopyRevision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(RbacLocalizedPresentations::TenantId)
                            .col(RbacLocalizedPresentations::ResourceKind)
                            .col(RbacLocalizedPresentations::ResourceKey)
                            .col(RbacLocalizedPresentations::Locale),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_rbac_localized_presentations_resource")
                    .table(RbacLocalizedPresentations::Table)
                    .col(RbacLocalizedPresentations::TenantId)
                    .col(RbacLocalizedPresentations::ResourceKind)
                    .col(RbacLocalizedPresentations::ResourceKey)
                    .to_owned(),
            )
            .await?;

        // The current built-in role and permission presentation is computed
        // inline rather than persisted owner data. There is therefore no
        // truthful legacy source locale to backfill here. Future migrations of
        // genuinely persisted legacy copy may use storage-only `und`, but new
        // canonical authoring is required to provide a concrete RuntimeLocale.
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RbacLocalizedPresentations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum RbacLocalizedPresentations {
    #[iden = "rbac_localized_presentations"]
    Table,
    TenantId,
    ResourceKind,
    ResourceKey,
    Locale,
    Name,
    Description,
    CopyRevision,
    CreatedAt,
    UpdatedAt,
}
