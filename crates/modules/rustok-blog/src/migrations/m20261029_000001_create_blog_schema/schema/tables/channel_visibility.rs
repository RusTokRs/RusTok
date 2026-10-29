use super::super::*;
use sea_orm_migration::prelude::*;

pub(super) async fn create(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    // 3. Create blog_post_channel_visibility table
    manager
        .create_table(
            Table::create()
                .table(BlogPostChannelVisibility::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(BlogPostChannelVisibility::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(BlogPostChannelVisibility::PostId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(BlogPostChannelVisibility::TenantId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(BlogPostChannelVisibility::ChannelSlug)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(BlogPostChannelVisibility::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_blog_post_channel_visibility_post")
                        .from(
                            BlogPostChannelVisibility::Table,
                            BlogPostChannelVisibility::PostId,
                        )
                        .to(BlogPosts::Table, BlogPosts::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .on_update(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_blog_post_channel_visibility_unique")
                .table(BlogPostChannelVisibility::Table)
                .col(BlogPostChannelVisibility::TenantId)
                .col(BlogPostChannelVisibility::PostId)
                .col(BlogPostChannelVisibility::ChannelSlug)
                .unique()
                .if_not_exists()
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_blog_post_channel_visibility_tenant_channel")
                .table(BlogPostChannelVisibility::Table)
                .col(BlogPostChannelVisibility::TenantId)
                .col(BlogPostChannelVisibility::ChannelSlug)
                .if_not_exists()
                .to_owned(),
        )
        .await?;
    Ok(())
}
