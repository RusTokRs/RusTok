use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

use crate::entities::{blog_post, blog_post_channel_visibility};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                            .string_len(128)
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
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_post_channel_visibility_post_slug")
                    .table(BlogPostChannelVisibility::Table)
                    .col(BlogPostChannelVisibility::PostId)
                    .col(BlogPostChannelVisibility::ChannelSlug)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_post_channel_visibility_tenant_slug")
                    .table(BlogPostChannelVisibility::Table)
                    .col(BlogPostChannelVisibility::TenantId)
                    .col(BlogPostChannelVisibility::ChannelSlug)
                    .to_owned(),
            )
            .await?;

        let posts = blog_post::Entity::find()
            .filter(blog_post::Column::Metadata.is_not_null())
            .all(manager.get_connection())
            .await?;

        for post in posts {
            for channel_slug in extract_channel_slugs(&post.metadata) {
                blog_post_channel_visibility::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    post_id: Set(post.id),
                    tenant_id: Set(post.tenant_id),
                    channel_slug: Set(channel_slug),
                    created_at: Set(chrono::Utc::now().into()),
                }
                .insert(manager.get_connection())
                .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(BlogPostChannelVisibility::Table)
                    .to_owned(),
            )
            .await
    }
}

fn extract_channel_slugs(metadata: &serde_json::Value) -> Vec<String> {
    metadata
        .get("channel_visibility")
        .and_then(|value| value.get("allowed_channel_slugs"))
        .and_then(|value| value.as_array())
        .map(|items| {
            let mut normalized = items
                .iter()
                .filter_map(|item| item.as_str())
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| item.to_ascii_lowercase())
                .collect::<Vec<_>>();
            normalized.sort();
            normalized.dedup();
            normalized
        })
        .unwrap_or_default()
}

#[derive(DeriveIden)]
enum BlogPostChannelVisibility {
    Table,
    Id,
    PostId,
    TenantId,
    ChannelSlug,
    CreatedAt,
}

#[derive(DeriveIden)]
enum BlogPosts {
    Table,
    Id,
}
