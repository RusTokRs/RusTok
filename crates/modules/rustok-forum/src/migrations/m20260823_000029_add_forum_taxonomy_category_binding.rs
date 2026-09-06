use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ForumCategoryTaxonomyBindings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ForumCategoryTaxonomyBindings::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumCategoryTaxonomyBindings::ForumCategoryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumCategoryTaxonomyBindings::TaxonomyCategoryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumCategoryTaxonomyBindings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(ForumCategoryTaxonomyBindings::TenantId)
                            .col(ForumCategoryTaxonomyBindings::ForumCategoryId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_category_taxonomy_binding_forum")
                            .from(
                                ForumCategoryTaxonomyBindings::Table,
                                (
                                    ForumCategoryTaxonomyBindings::TenantId,
                                    ForumCategoryTaxonomyBindings::ForumCategoryId,
                                ),
                            )
                            .to(
                                ForumCategories::Table,
                                (ForumCategories::TenantId, ForumCategories::Id),
                            )
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_category_taxonomy_binding_taxonomy")
                            .from(
                                ForumCategoryTaxonomyBindings::Table,
                                (
                                    ForumCategoryTaxonomyBindings::TenantId,
                                    ForumCategoryTaxonomyBindings::TaxonomyCategoryId,
                                ),
                            )
                            .to(
                                TaxonomyTerms::Table,
                                (TaxonomyTerms::TenantId, TaxonomyTerms::Id),
                            )
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_forum_category_taxonomy_binding_taxonomy")
                    .table(ForumCategoryTaxonomyBindings::Table)
                    .col(ForumCategoryTaxonomyBindings::TenantId)
                    .col(ForumCategoryTaxonomyBindings::TaxonomyCategoryId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ForumCategoryTaxonomyBindings::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum ForumCategoryTaxonomyBindings {
    #[iden = "forum_category_taxonomy_bindings"]
    Table,
    TenantId,
    ForumCategoryId,
    TaxonomyCategoryId,
    CreatedAt,
}

#[derive(Iden)]
enum ForumCategories {
    #[iden = "forum_categories"]
    Table,
    TenantId,
    Id,
}

#[derive(Iden)]
enum TaxonomyTerms {
    #[iden = "taxonomy_terms"]
    Table,
    TenantId,
    Id,
}
