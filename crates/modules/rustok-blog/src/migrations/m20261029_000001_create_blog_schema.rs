//! Consolidated Blog schema migration baseline.
//!
//! Replaces the 32 unreleased development migrations with a single canonical schema
//! containing all current Blog tables, indexes, constraints, and triggers under
//! AGENTS.md §7.1 and the Zero-Legacy Policy.

use sea_orm::{ConnectionTrait, DatabaseBackend};
use sea_orm_migration::prelude::*;

use crate::domain::BLOG_CATEGORY_SETTINGS_MAX_BYTES;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Create blog_posts table
        manager
            .create_table(
                Table::create()
                    .table(BlogPosts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogPosts::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(BlogPosts::TenantId).uuid().not_null())
                    .col(ColumnDef::new(BlogPosts::AuthorId).uuid().not_null())
                    .col(ColumnDef::new(BlogPosts::CategoryId).uuid())
                    .col(
                        ColumnDef::new(BlogPosts::Status)
                            .string_len(32)
                            .not_null()
                            .default("draft"),
                    )
                    .col(ColumnDef::new(BlogPosts::Slug).string_len(255).not_null())
                    .col(
                        ColumnDef::new(BlogPosts::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(ColumnDef::new(BlogPosts::FeaturedImageUrl).text())
                    .col(ColumnDef::new(BlogPosts::PublishedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(BlogPosts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(BlogPosts::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(BlogPosts::ArchivedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(BlogPosts::ScheduledAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(BlogPosts::IsPinned)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(BlogPosts::PinnedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(BlogPosts::CommentCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(BlogPosts::Version)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;

        // blog_posts indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_blog_posts_tenant_slug")
                    .table(BlogPosts::Table)
                    .col(BlogPosts::TenantId)
                    .col(BlogPosts::Slug)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_posts_tenant_status_published")
                    .table(BlogPosts::Table)
                    .col(BlogPosts::TenantId)
                    .col(BlogPosts::Status)
                    .col(BlogPosts::PublishedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_posts_tenant_pinned")
                    .table(BlogPosts::Table)
                    .col(BlogPosts::TenantId)
                    .col(BlogPosts::IsPinned)
                    .col(BlogPosts::PinnedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_posts_tenant_scheduled")
                    .table(BlogPosts::Table)
                    .col(BlogPosts::TenantId)
                    .col(BlogPosts::ScheduledAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_blog_posts_tenant_id")
                    .table(BlogPosts::Table)
                    .col(BlogPosts::TenantId)
                    .col(BlogPosts::Id)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 2. Create blog_post_translations table
        manager
            .create_table(
                Table::create()
                    .table(BlogPostTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogPostTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(BlogPostTranslations::PostId).uuid().not_null())
                    .col(
                        ColumnDef::new(BlogPostTranslations::Locale)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogPostTranslations::Title)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(BlogPostTranslations::Excerpt).text())
                    .col(ColumnDef::new(BlogPostTranslations::SeoTitle).string_len(255))
                    .col(ColumnDef::new(BlogPostTranslations::SeoDescription).string_len(500))
                    .col(ColumnDef::new(BlogPostTranslations::Body).text().not_null())
                    .col(
                        ColumnDef::new(BlogPostTranslations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(BlogPostTranslations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blog_post_translations_post")
                            .from(BlogPostTranslations::Table, BlogPostTranslations::PostId)
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
                    .name("idx_blog_post_translations_unique")
                    .table(BlogPostTranslations::Table)
                    .col(BlogPostTranslations::PostId)
                    .col(BlogPostTranslations::Locale)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

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

        // 4. Create blog_categories table
        manager
            .create_table(
                Table::create()
                    .table(BlogCategories::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogCategories::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(BlogCategories::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(BlogCategories::Settings)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(BlogCategories::Revision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(BlogCategories::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(BlogCategories::UpdatedAt)
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
                    .name("uq_blog_categories_tenant_id")
                    .table(BlogCategories::Table)
                    .col(BlogCategories::TenantId)
                    .col(BlogCategories::Id)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

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

        // 6. Create blog_tag_usage table
        manager
            .create_table(
                Table::create()
                    .table(BlogTagUsage::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(BlogTagUsage::TenantId).uuid().not_null())
                    .col(ColumnDef::new(BlogTagUsage::TagId).uuid().not_null())
                    .col(
                        ColumnDef::new(BlogTagUsage::CanonicalKey)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogTagUsage::UseCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .primary_key(
                        Index::create()
                            .col(BlogTagUsage::TenantId)
                            .col(BlogTagUsage::TagId),
                    )
                    .to_owned(),
            )
            .await?;

        // 7. Create blog_post_routes table
        manager
            .create_table(
                Table::create()
                    .table(BlogPostRoutes::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(BlogPostRoutes::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(BlogPostRoutes::SourceRoute)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogPostRoutes::TargetKind)
                            .string_len(32)
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
                            .string_len(64)
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

        // 9. Create blog_comment_projection_deliveries table
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
                        ColumnDef::new(BlogCommentProjectionDeliveries::ProjectionRevision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(BlogCommentProjectionDeliveries::ProcessedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blog_comment_projection_deliveries_post")
                            .from(
                                BlogCommentProjectionDeliveries::Table,
                                BlogCommentProjectionDeliveries::PostId,
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
                    .name("idx_blog_comment_projection_deliveries_tenant_post")
                    .table(BlogCommentProjectionDeliveries::Table)
                    .col(BlogCommentProjectionDeliveries::TenantId)
                    .col(BlogCommentProjectionDeliveries::PostId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 10. Create blog_comments_tcp_delegation_schedule_state table
        manager
            .create_table(
                Table::create()
                    .table(BlogCommentsTcpDelegationScheduleState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleState::StateKey)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleState::Generation)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleState::ActorId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleState::RequestId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleState::OccurredAtUnixMs)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleState::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // 11. Create blog_comments_tcp_delegation_schedule_audit_outbox table
        manager
            .create_table(
                Table::create()
                    .table(BlogCommentsTcpDelegationScheduleAuditOutbox::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditOutbox::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditOutbox::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditOutbox::RequestId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditOutbox::ActorId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditOutbox::Operation)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditOutbox::Source)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsTcpDelegationScheduleAuditOutbox::PreviousGeneration,
                        )
                        .big_integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsTcpDelegationScheduleAuditOutbox::CandidateGeneration,
                        )
                        .big_integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsTcpDelegationScheduleAuditOutbox::OccurredAtUnixMs,
                        )
                        .big_integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditOutbox::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditOutbox::RelayState)
                            .string_len(32)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditOutbox::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditOutbox::LastError).text())
                    .col(ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditOutbox::NextAttemptAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditOutbox::RelayedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_comments_tcp_audit_outbox_relay")
                    .table(BlogCommentsTcpDelegationScheduleAuditOutbox::Table)
                    .col(BlogCommentsTcpDelegationScheduleAuditOutbox::RelayState)
                    .col(BlogCommentsTcpDelegationScheduleAuditOutbox::NextAttemptAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 12. Create blog_comments_tcp_delegation_schedule_audit_recovery_audits table
        manager
            .create_table(
                Table::create()
                    .table(BlogCommentsTcpDelegationScheduleAuditRecoveryAudits::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleAuditRecoveryAudits::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsTcpDelegationScheduleAuditRecoveryAudits::OutboxId,
                        )
                        .uuid()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsTcpDelegationScheduleAuditRecoveryAudits::ActorId,
                        )
                        .uuid()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsTcpDelegationScheduleAuditRecoveryAudits::Action,
                        )
                        .string_len(64)
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsTcpDelegationScheduleAuditRecoveryAudits::CreatedAt,
                        )
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blog_comments_audit_recovery_outbox")
                            .from(
                                BlogCommentsTcpDelegationScheduleAuditRecoveryAudits::Table,
                                BlogCommentsTcpDelegationScheduleAuditRecoveryAudits::OutboxId,
                            )
                            .to(
                                BlogCommentsTcpDelegationScheduleAuditOutbox::Table,
                                BlogCommentsTcpDelegationScheduleAuditOutbox::Id,
                            )
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Backend-specific constraints and triggers
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await?,
            DatabaseBackend::Sqlite => up_sqlite(manager).await?,
            backend => {
                return Err(DbErr::Custom(format!(
                    "Blog schema migration does not support {backend:?}"
                )));
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            BlogCommentsTcpDelegationScheduleAuditRecoveryAudits::Table.into_table_ref(),
            BlogCommentsTcpDelegationScheduleAuditOutbox::Table.into_table_ref(),
            BlogCommentsTcpDelegationScheduleState::Table.into_table_ref(),
            BlogCommentProjectionDeliveries::Table.into_table_ref(),
            BlogPreviewTokens::Table.into_table_ref(),
            BlogPostRoutes::Table.into_table_ref(),
            BlogTagUsage::Table.into_table_ref(),
            BlogPostTags::Table.into_table_ref(),
            BlogPostChannelVisibility::Table.into_table_ref(),
            BlogPostTranslations::Table.into_table_ref(),
            BlogPosts::Table.into_table_ref(),
            BlogCategories::Table.into_table_ref(),
        ] {
            manager
                .drop_table(Table::drop().table(table).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    // 1. Column integrity constraints on blog_posts
    for (constraint, predicate) in [
        (
            "ck_blog_posts_status_vocabulary",
            "status IN ('draft', 'published', 'archived')",
        ),
        ("ck_blog_posts_version_positive", "version > 0"),
        ("ck_blog_posts_comment_count_non_negative", "comment_count >= 0"),
        (
            "ck_blog_posts_pin_state_coherent",
            "(is_pinned AND pinned_at IS NOT NULL) OR (NOT is_pinned AND pinned_at IS NULL)",
        ),
        (
            "ck_blog_posts_published_requires_published_at",
            "status <> 'published' OR published_at IS NOT NULL",
        ),
        (
            "ck_blog_posts_schedule_is_draft_only",
            "status = 'draft' OR scheduled_at IS NULL",
        ),
    ] {
        connection
            .execute_unprepared(&format!(
                "ALTER TABLE blog_posts ADD CONSTRAINT {constraint} CHECK ({predicate});"
            ))
            .await?;
    }

    // 2. Settings JSON contract constraints on blog_categories
    connection
        .execute_unprepared(&format!(
            r#"
ALTER TABLE blog_categories
    ADD CONSTRAINT ck_blog_categories_settings_object
    CHECK (jsonb_typeof(settings) = 'object');

ALTER TABLE blog_categories
    ADD CONSTRAINT ck_blog_categories_settings_size
    CHECK (octet_length(settings::text) <= {BLOG_CATEGORY_SETTINGS_MAX_BYTES});
"#
        ))
        .await?;

    // 3. Category tenant-integrity FK with ON DELETE SET NULL (category_id)
    connection
        .execute_unprepared(
            r#"
ALTER TABLE blog_posts
    ADD CONSTRAINT fk_blog_posts_tenant_category
    FOREIGN KEY (tenant_id, category_id)
    REFERENCES blog_categories (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE SET NULL (category_id);
"#,
        )
        .await?;

    // 4. Channel visibility composite tenant foreign key
    connection
        .execute_unprepared(
            r#"
ALTER TABLE blog_post_channel_visibility
    ADD CONSTRAINT fk_blog_post_channel_visibility_tenant_post
    FOREIGN KEY (tenant_id, post_id)
    REFERENCES blog_posts (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;
"#,
        )
        .await?;

    // 5. Post tags composite tenant foreign key
    connection
        .execute_unprepared(
            r#"
ALTER TABLE blog_post_tags
    ADD CONSTRAINT fk_blog_post_tags_tenant_post
    FOREIGN KEY (tenant_id, post_id)
    REFERENCES blog_posts (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;
"#,
        )
        .await?;

    // 6. Preview tokens composite tenant foreign key
    connection
        .execute_unprepared(
            r#"
ALTER TABLE blog_preview_tokens
    ADD CONSTRAINT fk_blog_preview_tokens_tenant_post
    FOREIGN KEY (tenant_id, post_id)
    REFERENCES blog_posts (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;
"#,
        )
        .await?;

    // 7. Comment projection deliveries composite tenant foreign key
    connection
        .execute_unprepared(
            r#"
ALTER TABLE blog_comment_projection_deliveries
    ADD CONSTRAINT fk_blog_comment_projection_deliveries_tenant_post
    FOREIGN KEY (tenant_id, post_id)
    REFERENCES blog_posts (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;
"#,
        )
        .await?;

    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    // 1. Column integrity triggers on blog_posts
    const SQLITE_INVALID_ROW_PREDICATE: &str = "
        NEW.status NOT IN ('draft', 'published', 'archived')
        OR NEW.version <= 0
        OR NEW.comment_count < 0
        OR (NEW.is_pinned AND NEW.pinned_at IS NULL)
        OR (NOT NEW.is_pinned AND NEW.pinned_at IS NOT NULL)
        OR (NEW.status = 'published' AND NEW.published_at IS NULL)
        OR (NEW.status <> 'draft' AND NEW.scheduled_at IS NOT NULL)
    ";

    for (trigger, timing) in [
        ("blog_posts_column_integrity_insert", "BEFORE INSERT"),
        ("blog_posts_column_integrity_update", "BEFORE UPDATE"),
    ] {
        connection
            .execute_unprepared(&format!(
                r#"
CREATE TRIGGER {trigger}
{timing} ON blog_posts
FOR EACH ROW
WHEN {SQLITE_INVALID_ROW_PREDICATE}
BEGIN
    SELECT RAISE(ABORT, 'blog_posts column integrity violated');
END;
"#
            ))
            .await?;
    }

    // 2. Settings contract triggers on blog_categories
    for (trigger, timing) in [
        ("blog_categories_settings_contract_insert", "BEFORE INSERT"),
        ("blog_categories_settings_contract_update", "BEFORE UPDATE OF settings"),
    ] {
        connection
            .execute_unprepared(&format!(
                r#"
CREATE TRIGGER {trigger}
{timing} ON blog_categories
FOR EACH ROW
WHEN json_valid(NEW.settings) = 0
  OR json_type(NEW.settings) <> 'object'
  OR length(CAST(json(NEW.settings) AS BLOB)) > {BLOG_CATEGORY_SETTINGS_MAX_BYTES}
BEGIN
    SELECT RAISE(ABORT, 'blog_categories settings contract violated');
END;
"#
            ))
            .await?;
    }

    // 3. Category tenant integrity triggers on blog_posts & blog_categories
    connection
        .execute_unprepared(
            r#"
CREATE TRIGGER blog_posts_category_tenant_insert
BEFORE INSERT ON blog_posts
FOR EACH ROW
WHEN NEW.category_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1
      FROM blog_categories category
      WHERE category.id = NEW.category_id
        AND category.tenant_id = NEW.tenant_id
  )
BEGIN
    SELECT RAISE(ABORT, 'blog post category tenant mismatch');
END;

CREATE TRIGGER blog_posts_category_tenant_update
BEFORE UPDATE OF tenant_id, category_id ON blog_posts
FOR EACH ROW
WHEN NEW.category_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1
      FROM blog_categories category
      WHERE category.id = NEW.category_id
        AND category.tenant_id = NEW.tenant_id
  )
BEGIN
    SELECT RAISE(ABORT, 'blog post category tenant mismatch');
END;

CREATE TRIGGER blog_categories_delete_null_post_category
AFTER DELETE ON blog_categories
FOR EACH ROW
BEGIN
    UPDATE blog_posts
    SET category_id = NULL
    WHERE tenant_id = OLD.tenant_id
      AND category_id = OLD.id;
END;
"#,
        )
        .await?;

    // 4. Channel visibility tenant integrity triggers
    connection
        .execute_unprepared(
            r#"
CREATE TRIGGER blog_post_channel_visibility_tenant_insert
BEFORE INSERT ON blog_post_channel_visibility
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM blog_posts post
    WHERE post.id = NEW.post_id
      AND post.tenant_id = NEW.tenant_id
)
BEGIN
    SELECT RAISE(ABORT, 'blog channel visibility tenant mismatch');
END;

CREATE TRIGGER blog_post_channel_visibility_tenant_update
BEFORE UPDATE OF tenant_id, post_id ON blog_post_channel_visibility
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM blog_posts post
    WHERE post.id = NEW.post_id
      AND post.tenant_id = NEW.tenant_id
)
BEGIN
    SELECT RAISE(ABORT, 'blog channel visibility tenant mismatch');
END;
"#,
        )
        .await?;

    // 5. Post tags tenant integrity triggers
    connection
        .execute_unprepared(
            r#"
CREATE TRIGGER blog_post_tags_tenant_insert
BEFORE INSERT ON blog_post_tags
FOR EACH ROW
WHEN NEW.tenant_id IS NULL
  OR NOT EXISTS (
      SELECT 1 FROM blog_posts post
      WHERE post.id = NEW.post_id
        AND post.tenant_id = NEW.tenant_id
  )
BEGIN
    SELECT RAISE(ABORT, 'blog post tag tenant mismatch');
END;

CREATE TRIGGER blog_post_tags_tenant_update
BEFORE UPDATE OF tenant_id, post_id, tag_id ON blog_post_tags
FOR EACH ROW
WHEN NEW.tenant_id IS NULL
  OR NOT EXISTS (
      SELECT 1 FROM blog_posts post
      WHERE post.id = NEW.post_id
        AND post.tenant_id = NEW.tenant_id
  )
BEGIN
    SELECT RAISE(ABORT, 'blog post tag tenant mismatch');
END;
"#,
        )
        .await?;

    // 6. Preview tokens tenant integrity triggers
    connection
        .execute_unprepared(
            r#"
CREATE TRIGGER blog_preview_tokens_tenant_insert
BEFORE INSERT ON blog_preview_tokens
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM blog_posts post
    WHERE post.id = NEW.post_id
      AND post.tenant_id = NEW.tenant_id
)
BEGIN
    SELECT RAISE(ABORT, 'blog preview token references post of another tenant');
END;

CREATE TRIGGER blog_preview_tokens_tenant_update
BEFORE UPDATE OF tenant_id, post_id ON blog_preview_tokens
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM blog_posts post
    WHERE post.id = NEW.post_id
      AND post.tenant_id = NEW.tenant_id
)
BEGIN
    SELECT RAISE(ABORT, 'blog preview token references post of another tenant');
END;
"#,
        )
        .await?;

    // 7. Comment projection deliveries tenant integrity triggers
    connection
        .execute_unprepared(
            r#"
CREATE TRIGGER blog_comment_projection_deliveries_tenant_insert
BEFORE INSERT ON blog_comment_projection_deliveries
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM blog_posts post
    WHERE post.id = NEW.post_id
      AND post.tenant_id = NEW.tenant_id
)
BEGIN
    SELECT RAISE(ABORT, 'blog comment projection delivery references post of another tenant');
END;

CREATE TRIGGER blog_comment_projection_deliveries_tenant_update
BEFORE UPDATE OF tenant_id, post_id ON blog_comment_projection_deliveries
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM blog_posts post
    WHERE post.id = NEW.post_id
      AND post.tenant_id = NEW.tenant_id
)
BEGIN
    SELECT RAISE(ABORT, 'blog comment projection delivery references post of another tenant');
END;
"#,
        )
        .await?;

    Ok(())
}

#[derive(Iden)]
enum BlogPosts {
    #[iden = "blog_posts"]
    Table,
    Id,
    TenantId,
    AuthorId,
    CategoryId,
    Status,
    Slug,
    Metadata,
    FeaturedImageUrl,
    PublishedAt,
    CreatedAt,
    UpdatedAt,
    ArchivedAt,
    ScheduledAt,
    IsPinned,
    PinnedAt,
    CommentCount,
    Version,
}

#[derive(Iden)]
enum BlogPostTranslations {
    #[iden = "blog_post_translations"]
    Table,
    Id,
    PostId,
    Locale,
    Title,
    Excerpt,
    SeoTitle,
    SeoDescription,
    Body,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum BlogPostChannelVisibility {
    #[iden = "blog_post_channel_visibility"]
    Table,
    Id,
    PostId,
    TenantId,
    ChannelSlug,
    CreatedAt,
}

#[derive(Iden)]
enum BlogCategories {
    #[iden = "blog_categories"]
    Table,
    Id,
    TenantId,
    Settings,
    Revision,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum BlogPostTags {
    #[iden = "blog_post_tags"]
    Table,
    PostId,
    TagId,
    TenantId,
    CreatedAt,
}

#[derive(Iden)]
enum BlogTagUsage {
    #[iden = "blog_tag_usage"]
    Table,
    TenantId,
    TagId,
    CanonicalKey,
    UseCount,
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

#[derive(Iden)]
enum BlogPreviewTokens {
    #[iden = "blog_preview_tokens"]
    Table,
    Id,
    TenantId,
    PostId,
    Token,
    CreatedBy,
    ExpiresAt,
    CreatedAt,
}

#[derive(Iden)]
enum BlogCommentProjectionDeliveries {
    #[iden = "blog_comment_projection_deliveries"]
    Table,
    EventId,
    TenantId,
    CommentId,
    PostId,
    Delta,
    ProjectionRevision,
    ProcessedAt,
}

#[derive(Iden)]
enum BlogCommentsTcpDelegationScheduleState {
    #[iden = "blog_comments_tcp_delegation_schedule_state"]
    Table,
    StateKey,
    Generation,
    ActorId,
    RequestId,
    OccurredAtUnixMs,
    UpdatedAt,
}

#[derive(Iden)]
enum BlogCommentsTcpDelegationScheduleAuditOutbox {
    #[iden = "blog_comments_tcp_delegation_schedule_audit_outbox"]
    Table,
    Id,
    TenantId,
    RequestId,
    ActorId,
    Operation,
    Source,
    PreviousGeneration,
    CandidateGeneration,
    OccurredAtUnixMs,
    CreatedAt,
    RelayState,
    AttemptCount,
    LastError,
    NextAttemptAt,
    RelayedAt,
}

#[derive(Iden)]
enum BlogCommentsTcpDelegationScheduleAuditRecoveryAudits {
    #[iden = "blog_comments_tcp_delegation_schedule_audit_recovery_audits"]
    Table,
    Id,
    OutboxId,
    ActorId,
    Action,
    CreatedAt,
}
