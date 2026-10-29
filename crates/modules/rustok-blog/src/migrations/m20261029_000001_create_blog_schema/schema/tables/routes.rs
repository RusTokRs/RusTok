use super::super::*;

pub(super) async fn create(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    // 7. Create blog_post_routes table
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
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(BlogPostRoutes::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(BlogPostRoutes::TenantId)
                        .col(BlogPostRoutes::SourceRoute),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_blog_post_routes_target")
                .table(BlogPostRoutes::Table)
                .col(BlogPostRoutes::TenantId)
                .col(BlogPostRoutes::TargetKind)
                .col(BlogPostRoutes::TargetId)
                .if_not_exists()
                .to_owned(),
        )
        .await?;
    Ok(())
}
