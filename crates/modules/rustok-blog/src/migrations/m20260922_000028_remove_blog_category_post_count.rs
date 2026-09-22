use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // post_count was legacy state with no writer and no API contract.
        // Blog post membership is canonically derived from blog_posts.category_id.
        manager
            .alter_table(
                Table::alter()
                    .table(BlogCategories::Table)
                    .drop_column(BlogCategories::PostCount)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Intentionally irreversible under Zero-Legacy Policy: restoring the
        // column would recreate a non-authoritative Category state field.
        Ok(())
    }
}

#[derive(Iden)]
enum BlogCategories {
    #[iden = "blog_categories"]
    Table,
    #[iden = "post_count"]
    PostCount,
}
