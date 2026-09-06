use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BlogCategories::Table)
                    .add_column(
                        ColumnDef::new(BlogCategories::Revision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(BlogCategoryTranslations::Table)
                    .add_column(
                        ColumnDef::new(BlogCategoryTranslations::Revision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(BlogTranslationChanges::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogTranslationChanges::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BlogTranslationChanges::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogTranslationChanges::ResourceKind)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogTranslationChanges::ResourceId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogTranslationChanges::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogTranslationChanges::ResourceRevision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogTranslationChanges::TargetRevision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogTranslationChanges::Operation)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogTranslationChanges::Lifecycle)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogTranslationChanges::CreatedAt)
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
                    .name("idx_blog_translation_changes_tenant_id")
                    .table(BlogTranslationChanges::Table)
                    .col(BlogTranslationChanges::TenantId)
                    .col(BlogTranslationChanges::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(BlogTranslationChanges::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(BlogCategoryTranslations::Table)
                    .drop_column(BlogCategoryTranslations::Revision)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(BlogCategories::Table)
                    .drop_column(BlogCategories::Revision)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum BlogCategories {
    #[iden = "blog_categories"]
    Table,
    Revision,
}

#[derive(Iden)]
enum BlogCategoryTranslations {
    #[iden = "blog_category_translations"]
    Table,
    Revision,
}

#[derive(Iden)]
enum BlogTranslationChanges {
    #[iden = "blog_translation_changes"]
    Table,
    Id,
    TenantId,
    ResourceKind,
    ResourceId,
    Locale,
    ResourceRevision,
    TargetRevision,
    Operation,
    Lifecycle,
    CreatedAt,
}
