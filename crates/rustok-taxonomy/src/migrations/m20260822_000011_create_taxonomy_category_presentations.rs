use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TaxonomyCategoryPresentations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::TermId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::IconKey)
                            .string_len(64)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::Color)
                            .string_len(9)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::ImageMediaId)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::CoverMediaId)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::Revision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TaxonomyCategoryPresentations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_taxonomy_category_presentations")
                            .col(TaxonomyCategoryPresentations::TenantId)
                            .col(TaxonomyCategoryPresentations::TermId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_taxonomy_category_presentations_term")
                            .from_tbl(TaxonomyCategoryPresentations::Table)
                            .from_col(TaxonomyCategoryPresentations::TenantId)
                            .from_col(TaxonomyCategoryPresentations::TermId)
                            .to_tbl(TaxonomyTerms::Table)
                            .to_col(TaxonomyTerms::TenantId)
                            .to_col(TaxonomyTerms::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_taxonomy_category_presentations_image_media")
                    .table(TaxonomyCategoryPresentations::Table)
                    .col(TaxonomyCategoryPresentations::TenantId)
                    .col(TaxonomyCategoryPresentations::ImageMediaId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_taxonomy_category_presentations_cover_media")
                    .table(TaxonomyCategoryPresentations::Table)
                    .col(TaxonomyCategoryPresentations::TenantId)
                    .col(TaxonomyCategoryPresentations::CoverMediaId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(TaxonomyCategoryPresentations::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum TaxonomyCategoryPresentations {
    Table,
    TenantId,
    TermId,
    IconKey,
    Color,
    ImageMediaId,
    CoverMediaId,
    Revision,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum TaxonomyTerms {
    Table,
    TenantId,
    Id,
}
