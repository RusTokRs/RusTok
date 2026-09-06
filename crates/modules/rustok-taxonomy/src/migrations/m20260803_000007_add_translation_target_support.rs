use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(TaxonomyTerms::Table)
                    .add_column(
                        ColumnDef::new(TaxonomyTerms::Revision)
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
                    .table(TaxonomyTermTranslations::Table)
                    .add_column(
                        ColumnDef::new(TaxonomyTermTranslations::Revision)
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
                    .table(TranslationChanges::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TranslationChanges::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TranslationChanges::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TranslationChanges::TermId).uuid().not_null())
                    .col(
                        ColumnDef::new(TranslationChanges::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TranslationChanges::ResourceRevision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TranslationChanges::TargetRevision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TranslationChanges::Operation)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TranslationChanges::Lifecycle)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TranslationChanges::CreatedAt)
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
                    .name("idx_taxonomy_translation_changes_tenant_id")
                    .table(TranslationChanges::Table)
                    .col(TranslationChanges::TenantId)
                    .col(TranslationChanges::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TranslationChanges::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(TaxonomyTermTranslations::Table)
                    .drop_column(TaxonomyTermTranslations::Revision)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(TaxonomyTerms::Table)
                    .drop_column(TaxonomyTerms::Revision)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum TaxonomyTerms {
    #[iden = "taxonomy_terms"]
    Table,
    Revision,
}

#[derive(Iden)]
enum TaxonomyTermTranslations {
    #[iden = "taxonomy_term_translations"]
    Table,
    Revision,
}

#[derive(Iden)]
enum TranslationChanges {
    #[iden = "taxonomy_translation_changes"]
    Table,
    Id,
    TenantId,
    TermId,
    Locale,
    ResourceRevision,
    TargetRevision,
    Operation,
    Lifecycle,
    CreatedAt,
}
