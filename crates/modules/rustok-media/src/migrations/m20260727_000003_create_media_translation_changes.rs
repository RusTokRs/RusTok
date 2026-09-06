use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MediaTranslationChanges::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MediaTranslationChanges::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MediaTranslationChanges::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MediaTranslationChanges::AssetId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MediaTranslationChanges::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MediaTranslationChanges::ResourceRevision)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MediaTranslationChanges::TargetRevision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MediaTranslationChanges::Operation)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MediaTranslationChanges::Lifecycle)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MediaTranslationChanges::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_translation_changes_tenant")
                            .from(
                                MediaTranslationChanges::Table,
                                MediaTranslationChanges::TenantId,
                            )
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_translation_changes_tenant_asset")
                            .from_tbl(MediaTranslationChanges::Table)
                            .from_col(MediaTranslationChanges::TenantId)
                            .from_col(MediaTranslationChanges::AssetId)
                            .to_tbl(MediaAssets::Table)
                            .to_col(MediaAssets::TenantId)
                            .to_col(MediaAssets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_media_translation_changes_tenant_cursor")
                    .table(MediaTranslationChanges::Table)
                    .col(MediaTranslationChanges::TenantId)
                    .col(MediaTranslationChanges::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_media_translation_changes_tenant_asset_cursor")
                    .table(MediaTranslationChanges::Table)
                    .col(MediaTranslationChanges::TenantId)
                    .col(MediaTranslationChanges::AssetId)
                    .col(MediaTranslationChanges::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: this owner change log is durable projection-repair
        // evidence and must not be discarded by a schema rollback.
        Ok(())
    }
}

#[derive(Iden)]
enum MediaTranslationChanges {
    #[iden = "media_translation_changes"]
    Table,
    Id,
    TenantId,
    AssetId,
    Locale,
    ResourceRevision,
    TargetRevision,
    Operation,
    Lifecycle,
    CreatedAt,
}

#[derive(Iden)]
enum MediaAssets {
    #[iden = "media_assets"]
    Table,
    TenantId,
    Id,
}

#[derive(Iden)]
enum Tenants {
    #[iden = "tenants"]
    Table,
    Id,
}
