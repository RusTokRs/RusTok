use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PageTranslations::Table)
                    .add_column(
                        ColumnDef::new(PageTranslations::Revision)
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
                    .table(PagesTranslationChanges::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PagesTranslationChanges::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PagesTranslationChanges::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagesTranslationChanges::ResourceKind)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagesTranslationChanges::ResourceId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagesTranslationChanges::ResourceRevision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagesTranslationChanges::Operation)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagesTranslationChanges::Lifecycle)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagesTranslationChanges::CreatedAt)
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
                    .name("idx_pages_translation_changes_tenant_id")
                    .table(PagesTranslationChanges::Table)
                    .col(PagesTranslationChanges::TenantId)
                    .col(PagesTranslationChanges::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PagesTranslationChanges::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(PageTranslations::Table)
                    .drop_column(PageTranslations::Revision)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum PageTranslations {
    #[iden = "page_translations"]
    Table,
    Revision,
}

#[derive(Iden)]
enum PagesTranslationChanges {
    #[iden = "pages_translation_changes"]
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
