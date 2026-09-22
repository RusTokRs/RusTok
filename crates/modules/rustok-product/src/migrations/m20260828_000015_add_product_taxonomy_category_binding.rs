use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .create_table(
                Table::create()
                    .table(ProductCatalogCategoryTaxonomyBindings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProductCatalogCategoryTaxonomyBindings::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductCatalogCategoryTaxonomyBindings::CatalogCategoryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductCatalogCategoryTaxonomyBindings::TaxonomyCategoryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductCatalogCategoryTaxonomyBindings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(ProductCatalogCategoryTaxonomyBindings::TenantId)
                            .col(ProductCatalogCategoryTaxonomyBindings::CatalogCategoryId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_catalog_category_taxonomy_binding_product")
                            .from(
                                ProductCatalogCategoryTaxonomyBindings::Table,
                                (
                                    ProductCatalogCategoryTaxonomyBindings::TenantId,
                                    ProductCatalogCategoryTaxonomyBindings::CatalogCategoryId,
                                ),
                            )
                            .to(
                                CatalogCategories::Table,
                                (CatalogCategories::TenantId, CatalogCategories::Id),
                            )
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_product_catalog_category_taxonomy_binding_taxonomy")
                            .from(
                                ProductCatalogCategoryTaxonomyBindings::Table,
                                (
                                    ProductCatalogCategoryTaxonomyBindings::TenantId,
                                    ProductCatalogCategoryTaxonomyBindings::TaxonomyCategoryId,
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
                    .name("uq_product_catalog_category_taxonomy_binding_taxonomy")
                    .table(ProductCatalogCategoryTaxonomyBindings::Table)
                    .col(ProductCatalogCategoryTaxonomyBindings::TenantId)
                    .col(ProductCatalogCategoryTaxonomyBindings::TaxonomyCategoryId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .drop_table(
                Table::drop()
                    .table(ProductCatalogCategoryTaxonomyBindings::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum ProductCatalogCategoryTaxonomyBindings {
    #[iden = "product_catalog_category_taxonomy_bindings"]
    Table,
    TenantId,
    CatalogCategoryId,
    TaxonomyCategoryId,
    CreatedAt,
}

#[derive(Iden)]
enum CatalogCategories {
    #[iden = "catalog_categories"]
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
