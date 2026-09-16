use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Drop table forum_category_taxonomy_bindings
        manager
            .drop_table(
                Table::drop()
                    .table(ForumCategoryTaxonomyBindings::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        // 2. If PostgreSQL or MySQL, drop foreign keys
        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            let _ = manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .table(ForumCategories::Table)
                        .name("fk_forum_categories_parent_tenant")
                        .to_owned(),
                )
                .await;
            let _ = manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .table(ForumCategories::Table)
                        .name("fk_forum_categories_parent")
                        .to_owned(),
                )
                .await;
        }

        // 3. Drop redundant columns parent_id, position, icon, color
        manager
            .alter_table(
                Table::alter()
                    .table(ForumCategories::Table)
                    .drop_column(ForumCategories::ParentId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ForumCategories::Table)
                    .drop_column(ForumCategories::Position)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ForumCategories::Table)
                    .drop_column(ForumCategories::Icon)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ForumCategories::Table)
                    .drop_column(ForumCategories::Color)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Intentionally irreversible under Zero-Legacy Policy.
        // Taxonomy is the single canonical source of truth for Category hierarchy and presentation.
        Ok(())
    }
}

#[derive(Iden)]
enum ForumCategoryTaxonomyBindings {
    #[iden = "forum_category_taxonomy_bindings"]
    Table,
}

#[derive(Iden)]
enum ForumCategories {
    #[iden = "forum_categories"]
    Table,
    #[iden = "parent_id"]
    ParentId,
    #[iden = "position"]
    Position,
    #[iden = "icon"]
    Icon,
    #[iden = "color"]
    Color,
}
