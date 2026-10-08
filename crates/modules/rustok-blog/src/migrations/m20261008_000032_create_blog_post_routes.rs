use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Blog owns its redirect routes. Blog slugs are global, so the source
        // route is locale-neutral and unique per tenant. The canonical route of
        // a post is derived from `blog_posts.slug` and is never stored here.
        manager
            .create_table(
                Table::create()
                    .table(BlogPostRoutes::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(BlogPostRoutes::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(BlogPostRoutes::SourceRoute)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogPostRoutes::TargetKind)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(BlogPostRoutes::TargetId).uuid().not_null())
                    .col(
                        ColumnDef::new(BlogPostRoutes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogPostRoutes::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_blog_post_routes")
                            .col(BlogPostRoutes::TenantId)
                            .col(BlogPostRoutes::SourceRoute),
                    )
                    .to_owned(),
            )
            .await?;

        // Removing a target (post deletion, demotion) finds its redirects here.
        manager
            .create_index(
                Index::create()
                    .name("idx_blog_post_routes_target")
                    .table(BlogPostRoutes::Table)
                    .col(BlogPostRoutes::TenantId)
                    .col(BlogPostRoutes::TargetKind)
                    .col(BlogPostRoutes::TargetId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BlogPostRoutes::Table).if_exists().to_owned())
            .await
    }
}

#[derive(Iden)]
enum BlogPostRoutes {
    #[iden = "blog_post_routes"]
    Table,
    TenantId,
    SourceRoute,
    TargetKind,
    TargetId,
    CreatedAt,
    UpdatedAt,
}
