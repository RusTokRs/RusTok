use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    .col(ColumnDef::new(BlogPreviewTokens::CreatedBy).uuid().null())
                    .col(
                        ColumnDef::new(BlogPreviewTokens::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogPreviewTokens::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // A preview token has no meaning without its post: deleting a post removes
        // its tokens at the database level, in addition to the in-transaction delete
        // in `PostService::delete_post`.
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_blog_preview_tokens_post")
                    .from(BlogPreviewTokens::Table, BlogPreviewTokens::PostId)
                    .to(BlogPosts::Table, BlogPosts::Id)
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // Index for lookup by token (the unique key already covers this, but
        // an explicit index documents the hot path).
        manager
            .create_index(
                Index::create()
                    .name("idx_blog_preview_tokens_token")
                    .table(BlogPreviewTokens::Table)
                    .col(BlogPreviewTokens::Token)
                    .to_owned(),
            )
            .await?;

        // Index for cleanup: find expired tokens and tokens of a deleted post.
        manager
            .create_index(
                Index::create()
                    .name("idx_blog_preview_tokens_post")
                    .table(BlogPreviewTokens::Table)
                    .col(BlogPreviewTokens::TenantId)
                    .col(BlogPreviewTokens::PostId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_preview_tokens_expires")
                    .table(BlogPreviewTokens::Table)
                    .col(BlogPreviewTokens::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(BlogPreviewTokens::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum BlogPosts {
    #[iden = "blog_posts"]
    Table,
    #[iden = "id"]
    Id,
}

#[derive(Iden)]
enum BlogPreviewTokens {
    #[iden = "blog_preview_tokens"]
    Table,
    #[iden = "id"]
    Id,
    #[iden = "tenant_id"]
    TenantId,
    #[iden = "post_id"]
    PostId,
    #[iden = "token"]
    Token,
    #[iden = "created_by"]
    CreatedBy,
    #[iden = "expires_at"]
    ExpiresAt,
    #[iden = "created_at"]
    CreatedAt,
}
