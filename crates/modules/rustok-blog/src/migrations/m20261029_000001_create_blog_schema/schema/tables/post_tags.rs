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

                    // Enforce tenant-correct Taxonomy ownership on both PostgreSQL and SQLite.
                    // A dictionary deletion must also delete its Blog attachments.
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blog_post_tags_tag_tenant")
                            .from_tbl(BlogPostTags::Table)
                            .from_col(BlogPostTags::TenantId)
                            .from_col(BlogPostTags::TagId)
                            .to_tbl(Alias::new("taxonomy_terms"))
                            .to_col(Alias::new("tenant_id"))
                            .to_col(Alias::new("id"))
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
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
