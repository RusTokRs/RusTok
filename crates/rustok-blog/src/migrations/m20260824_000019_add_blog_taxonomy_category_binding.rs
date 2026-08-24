use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BlogCategoryTaxonomyBindings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogCategoryTaxonomyBindings::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCategoryTaxonomyBindings::BlogCategoryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCategoryTaxonomyBindings::TaxonomyCategoryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCategoryTaxonomyBindings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(BlogCategoryTaxonomyBindings::TenantId)
                            .col(BlogCategoryTaxonomyBindings::BlogCategoryId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blog_category_taxonomy_binding_blog")
                            .from(
                                BlogCategoryTaxonomyBindings::Table,
                                (
                                    BlogCategoryTaxonomyBindings::TenantId,
                                    BlogCategoryTaxonomyBindings::BlogCategoryId,
                                ),
                            )
                            .to(
                                BlogCategories::Table,
                                (BlogCategories::TenantId, BlogCategories::Id),
                            )
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blog_category_taxonomy_binding_taxonomy")
                            .from(
                                BlogCategoryTaxonomyBindings::Table,
                                (
                                    BlogCategoryTaxonomyBindings::TenantId,
                                    BlogCategoryTaxonomyBindings::TaxonomyCategoryId,
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
                    .name("uq_blog_category_taxonomy_binding_taxonomy")
                    .table(BlogCategoryTaxonomyBindings::Table)
                    .col(BlogCategoryTaxonomyBindings::TenantId)
                    .col(BlogCategoryTaxonomyBindings::TaxonomyCategoryId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(BlogCategoryTaxonomyBindings::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum BlogCategoryTaxonomyBindings {
    #[iden = "blog_category_taxonomy_bindings"]
    Table,
    TenantId,
    BlogCategoryId,
    TaxonomyCategoryId,
    CreatedAt,
}

#[derive(Iden)]
enum BlogCategories {
    #[iden = "blog_categories"]
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
