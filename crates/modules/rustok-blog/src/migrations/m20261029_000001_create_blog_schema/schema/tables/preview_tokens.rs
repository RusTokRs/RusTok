use super::super::*;
use sea_orm_migration::prelude::*;

pub(super) async fn create(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    // 8. Create blog_preview_tokens table
    manager
        .create_table(
            Table::create()
                .table(BlogPreviewTokens::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(BlogPreviewTokens::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(BlogPreviewTokens::TenantId)
                        .uuid()
                        .not_null(),
                )
                .col(ColumnDef::new(BlogPreviewTokens::PostId).uuid().not_null())
                .col(
                    ColumnDef::new(BlogPreviewTokens::Token)
                        .string_len(128)
                        .not_null()
                        .unique_key(),
                )
                .col(ColumnDef::new(BlogPreviewTokens::CreatedBy).uuid())
                .col(
                    ColumnDef::new(BlogPreviewTokens::ExpiresAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(BlogPreviewTokens::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_blog_preview_tokens_post")
                        .from(BlogPreviewTokens::Table, BlogPreviewTokens::PostId)
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
                .name("idx_blog_preview_tokens_token")
                .table(BlogPreviewTokens::Table)
                .col(BlogPreviewTokens::Token)
                .if_not_exists()
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_blog_preview_tokens_tenant_post")
                .table(BlogPreviewTokens::Table)
                .col(BlogPreviewTokens::TenantId)
                .col(BlogPreviewTokens::PostId)
                .if_not_exists()
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_blog_preview_tokens_tenant_expires")
                .table(BlogPreviewTokens::Table)
                .col(BlogPreviewTokens::TenantId)
                .col(BlogPreviewTokens::ExpiresAt)
                .if_not_exists()
                .to_owned(),
        )
        .await?;
    Ok(())
}
