use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ForumTranslationChanges::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ForumTranslationChanges::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ForumTranslationChanges::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumTranslationChanges::ResourceKind)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumTranslationChanges::ResourceId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumTranslationChanges::ResourceRevision)
                            .string_len(256)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumTranslationChanges::Operation)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumTranslationChanges::Lifecycle)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumTranslationChanges::CreatedAt)
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
                    .if_not_exists()
                    .name("idx_forum_translation_changes_tenant_kind_id")
                    .table(ForumTranslationChanges::Table)
                    .col(ForumTranslationChanges::TenantId)
                    .col(ForumTranslationChanges::ResourceKind)
                    .col(ForumTranslationChanges::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ForumTranslationChanges::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum ForumTranslationChanges {
    #[iden = "forum_translation_changes"]
    Table,
    Id,
    TenantId,
    ResourceKind,
    ResourceId,
    ResourceRevision,
    Operation,
    Lifecycle,
    CreatedAt,
}
