use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Drop table blog_category_taxonomy_bindings
        manager
            .drop_table(
                Table::drop()
                    .table(BlogCategoryTaxonomyBindings::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        // 2. If PostgreSQL or MySQL, drop foreign key fk_blog_categories_tenant_parent
        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .table(BlogCategories::Table)
                        .name("fk_blog_categories_tenant_parent")
                        .to_owned(),
                )
                .await?;
        }

        // 3. Drop redundant columns parent_id, position, depth from blog_categories.
        // Taxonomy is the canonical owner of Category hierarchy.
        manager
            .alter_table(
                Table::alter()
                    .table(BlogCategories::Table)
                    .drop_column(BlogCategories::ParentId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BlogCategories::Table)
                    .drop_column(BlogCategories::Position)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BlogCategories::Table)
                    .drop_column(BlogCategories::Depth)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Intentionally irreversible under Zero-Legacy Policy.
        // Taxonomy is the single canonical source of truth for Category hierarchy and placement.
        Ok(())
    }
}

#[derive(Iden)]
enum BlogCategoryTaxonomyBindings {
    #[iden = "blog_category_taxonomy_bindings"]
    Table,
}

#[derive(Iden)]
enum BlogCategories {
    #[iden = "blog_categories"]
    Table,
    #[iden = "parent_id"]
    ParentId,
    #[iden = "position"]
    Position,
    #[iden = "depth"]
    Depth,
}
