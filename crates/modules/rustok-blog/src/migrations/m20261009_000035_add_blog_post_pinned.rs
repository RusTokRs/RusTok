use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add `is_pinned` flag: when true, the post appears at the top of
        // public listings regardless of its published_at date.
        manager
            .alter_table(
                Table::alter()
                    .table(BlogPosts::Table)
                    .add_column(
                        ColumnDef::new(BlogPosts::IsPinned)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // Add `pinned_at` timestamp: records when the post was pinned.
        // Used for ordering among multiple pinned posts (most recently pinned first).
        manager
            .alter_table(
                Table::alter()
                    .table(BlogPosts::Table)
                    .add_column(
                        ColumnDef::new(BlogPosts::PinnedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for efficient pinned-first ordering in public listings.
        manager
            .create_index(
                Index::create()
                    .name("idx_blog_posts_pinned")
                    .table(BlogPosts::Table)
                    .col(BlogPosts::TenantId)
                    .col(BlogPosts::IsPinned)
                    .col(BlogPosts::PinnedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_blog_posts_pinned")
                    .table(BlogPosts::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BlogPosts::Table)
                    .drop_column(BlogPosts::PinnedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BlogPosts::Table)
                    .drop_column(BlogPosts::IsPinned)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum BlogPosts {
    #[iden = "blog_posts"]
    Table,
    IsPinned,
    PinnedAt,
    TenantId,
}
