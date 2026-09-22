use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BlogCommentProjectionDeliveries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogCommentProjectionDeliveries::EventId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentProjectionDeliveries::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentProjectionDeliveries::CommentId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentProjectionDeliveries::PostId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentProjectionDeliveries::Delta)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentProjectionDeliveries::ProcessedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_comment_projection_deliveries_tenant_post")
                    .table(BlogCommentProjectionDeliveries::Table)
                    .col(BlogCommentProjectionDeliveries::TenantId)
                    .col(BlogCommentProjectionDeliveries::PostId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(BlogCommentProjectionDeliveries::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum BlogCommentProjectionDeliveries {
    Table,
    EventId,
    TenantId,
    CommentId,
    PostId,
    Delta,
    ProcessedAt,
}
