use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add `scheduled_at` to `blog_posts`: when set on a Draft post, the
        // scheduler worker publishes it automatically once the timestamp is
        // reached. The field is cleared when the post is manually published
        // or when the schedule is cancelled.
        manager
            .alter_table(
                Table::alter()
                    .table(BlogPosts::Table)
                    .add_column(
                        ColumnDef::new(BlogPosts::ScheduledAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for the scheduler worker: it scans Draft posts whose
        // scheduled_at is in the past. The index avoids a full table scan
        // on every tick.
        manager
            .create_index(
                Index::create()
                    .name("idx_blog_posts_scheduled")
                    .table(BlogPosts::Table)
                    .col(BlogPosts::TenantId)
                    .col(BlogPosts::Status)
                    .col(BlogPosts::ScheduledAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_blog_posts_scheduled")
                    .table(BlogPosts::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BlogPosts::Table)
                    .drop_column(BlogPosts::ScheduledAt)
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
    ScheduledAt,
    TenantId,
    Status,
}
