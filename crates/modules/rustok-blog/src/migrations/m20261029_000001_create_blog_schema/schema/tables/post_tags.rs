use super::super::*;
use sea_orm_migration::prelude::*;

pub(super) async fn create(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
// 5. Create blog_post_tags table
        manager
            .create_table(
                Table::create()
                    .table(BlogPostTags::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(BlogPostTags::PostId).uuid().not_null())
                    .col(ColumnDef::new(BlogPostTags::TagId).uuid().not_null())
                    .col(ColumnDef::new(BlogPostTags::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(BlogPostTags::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(BlogPostTags::PostId)
                            .col(BlogPostTags::TagId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blog_post_tags_post")
                            .from(BlogPostTags::Table, BlogPostTags::PostId)
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
                    .name("idx_blog_post_tags_tenant_tag_id")
                    .table(BlogPostTags::Table)
                    .col(BlogPostTags::TenantId)
                    .col(BlogPostTags::TagId)
                    .col(BlogPostTags::PostId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
    Ok(())
}
