use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // view_count was never incremented by any writer and always stored 0.
        // Under the Zero-Legacy Policy the field is removed instead of being
        // kept as a misleading public contract. Real view analytics belong to
        // a dedicated analytics module, not to a counter on blog_posts.
        manager
            .alter_table(
                Table::alter()
                    .table(BlogPosts::Table)
                    .drop_column(BlogPosts::ViewCount)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Intentionally irreversible under Zero-Legacy Policy: restoring the
        // column would recreate a non-authoritative counter with no writer.
        Ok(())
    }
}

#[derive(Iden)]
enum BlogPosts {
    #[iden = "blog_posts"]
    Table,
    #[iden = "view_count"]
    ViewCount,
}
