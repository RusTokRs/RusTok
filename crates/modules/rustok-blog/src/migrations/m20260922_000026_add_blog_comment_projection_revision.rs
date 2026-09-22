use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BlogCommentProjectionDeliveries::Table)
                    .add_column(
                        ColumnDef::new(BlogCommentProjectionDeliveries::ProjectionRevision)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_comment_projection_deliveries_tenant_post_revision")
                    .table(BlogCommentProjectionDeliveries::Table)
                    .col(BlogCommentProjectionDeliveries::TenantId)
                    .col(BlogCommentProjectionDeliveries::PostId)
                    .col(BlogCommentProjectionDeliveries::ProjectionRevision)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_blog_comment_projection_deliveries_tenant_post_revision")
                    .table(BlogCommentProjectionDeliveries::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BlogCommentProjectionDeliveries::Table)
                    .drop_column(BlogCommentProjectionDeliveries::ProjectionRevision)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum BlogCommentProjectionDeliveries {
    Table,
    TenantId,
    PostId,
    ProjectionRevision,
}
