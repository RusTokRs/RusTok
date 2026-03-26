use async_graphql::{dataloader::DataLoader, Context, FieldError, Object, Result};
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError},
    has_any_effective_permission, AuthContext,
};
use rustok_core::{Permission, CONTENT_FORMAT_MARKDOWN};
use rustok_outbox::TransactionalEventBus;
use rustok_profiles::{
    graphql::GqlProfileSummary, ProfileService, ProfileSummaryLoader, ProfileSummaryLoaderKey,
    ProfilesReader,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{CategoryService, ReplyService, TopicService};

use super::types::*;

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub struct ForumMutation;

#[Object]
impl ForumMutation {
    async fn create_forum_topic(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        input: CreateForumTopicInput,
    ) -> Result<GqlForumTopic> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_CREATE],
            "Permission denied: forum_topics:create required",
        )?;

        let service = TopicService::new(db.clone(), event_bus.clone());
        let topic = service
            .create(
                tenant_id,
                auth.security_context(),
                crate::CreateTopicInput {
                    locale: input.locale,
                    category_id: input.category_id,
                    title: input.title,
                    slug: input.slug,
                    body: input.body,
                    body_format: input
                        .body_format
                        .unwrap_or_else(|| CONTENT_FORMAT_MARKDOWN.to_string()),
                    content_json: input.content_json,
                    tags: input.tags,
                },
            )
            .await?;
        let author_profile = load_author_profile(
            ctx,
            db,
            tenant_id,
            topic.author_id,
            topic.effective_locale.as_str(),
        )
        .await?;

        Ok(GqlForumTopic {
            id: topic.id,
            requested_locale: topic.requested_locale,
            locale: topic.locale,
            effective_locale: topic.effective_locale,
            available_locales: topic.available_locales,
            category_id: topic.category_id,
            author_id: topic.author_id,
            author_profile,
            title: topic.title,
            slug: topic.slug,
            body: topic.body,
            body_format: topic.body_format,
            status: topic.status,
            tags: topic.tags,
            is_pinned: topic.is_pinned,
            is_locked: topic.is_locked,
            reply_count: topic.reply_count,
            created_at: topic.created_at,
            updated_at: topic.updated_at,
        })
    }

    async fn update_forum_topic(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateForumTopicInput,
    ) -> Result<GqlForumTopic> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_UPDATE],
            "Permission denied: forum_topics:update required",
        )?;

        let service = TopicService::new(db.clone(), event_bus.clone());
        let topic = service
            .update(
                tenant_id,
                id,
                auth.security_context(),
                crate::UpdateTopicInput {
                    locale: input.locale,
                    title: input.title,
                    body: input.body,
                    body_format: input.body_format,
                    content_json: input.content_json,
                    tags: input.tags,
                },
            )
            .await?;
        let author_profile = load_author_profile(
            ctx,
            db,
            tenant_id,
            topic.author_id,
            topic.effective_locale.as_str(),
        )
        .await?;

        Ok(GqlForumTopic {
            id: topic.id,
            requested_locale: topic.requested_locale,
            locale: topic.locale,
            effective_locale: topic.effective_locale,
            available_locales: topic.available_locales,
            category_id: topic.category_id,
            author_id: topic.author_id,
            author_profile,
            title: topic.title,
            slug: topic.slug,
            body: topic.body,
            body_format: topic.body_format,
            status: topic.status,
            tags: topic.tags,
            is_pinned: topic.is_pinned,
            is_locked: topic.is_locked,
            reply_count: topic.reply_count,
            created_at: topic.created_at,
            updated_at: topic.updated_at,
        })
    }

    async fn delete_forum_topic(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_DELETE],
            "Permission denied: forum_topics:delete required",
        )?;

        let service = TopicService::new(db.clone(), event_bus.clone());
        service
            .delete(tenant_id, id, auth.security_context())
            .await?;

        Ok(true)
    }

    async fn create_forum_reply(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        topic_id: Uuid,
        input: CreateForumReplyInput,
    ) -> Result<GqlForumReply> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_REPLIES_CREATE],
            "Permission denied: forum_replies:create required",
        )?;

        let service = ReplyService::new(db.clone(), event_bus.clone());
        let reply = service
            .create(
                tenant_id,
                auth.security_context(),
                topic_id,
                crate::CreateReplyInput {
                    locale: input.locale,
                    content: input.content,
                    content_format: input
                        .content_format
                        .unwrap_or_else(|| CONTENT_FORMAT_MARKDOWN.to_string()),
                    content_json: input.content_json,
                    parent_reply_id: input.parent_reply_id,
                },
            )
            .await?;
        let author_profile = load_author_profile(
            ctx,
            db,
            tenant_id,
            reply.author_id,
            reply.effective_locale.as_str(),
        )
        .await?;

        Ok(GqlForumReply {
            id: reply.id,
            requested_locale: reply.requested_locale,
            locale: reply.locale,
            effective_locale: reply.effective_locale,
            topic_id: reply.topic_id,
            author_id: reply.author_id,
            author_profile,
            content: reply.content,
            content_format: reply.content_format,
            status: reply.status,
            parent_reply_id: reply.parent_reply_id,
            created_at: reply.created_at,
            updated_at: reply.updated_at,
        })
    }

    async fn create_forum_category(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        input: CreateForumCategoryInput,
    ) -> Result<GqlForumCategory> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_CREATE],
            "Permission denied: forum_categories:create required",
        )?;

        let service = CategoryService::new(db.clone());
        let category = service
            .create(
                tenant_id,
                auth.security_context(),
                crate::CreateCategoryInput {
                    locale: input.locale,
                    name: input.name,
                    slug: input.slug,
                    description: input.description,
                    icon: input.icon,
                    color: input.color,
                    parent_id: input.parent_id,
                    position: input.position,
                    moderated: input.moderated,
                },
            )
            .await?;

        Ok(GqlForumCategory {
            id: category.id,
            requested_locale: category.requested_locale,
            locale: category.locale,
            effective_locale: category.effective_locale,
            name: category.name,
            slug: category.slug,
            description: category.description,
            icon: category.icon,
            color: category.color,
            topic_count: category.topic_count,
            reply_count: category.reply_count,
        })
    }
}

fn require_forum_permission(
    ctx: &Context<'_>,
    permissions: &[Permission],
    message: &str,
) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();

    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(<FieldError as GraphQLError>::permission_denied(message));
    }

    Ok(auth)
}

async fn load_author_profile(
    ctx: &Context<'_>,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    author_id: Option<Uuid>,
    requested_locale: &str,
) -> Result<Option<GqlProfileSummary>> {
    let Some(author_id) = author_id else {
        return Ok(None);
    };

    if let Some(loader) = ctx.data_opt::<DataLoader<ProfileSummaryLoader>>() {
        let profile = loader
            .load_one(ProfileSummaryLoaderKey {
                tenant_id,
                user_id: author_id,
                requested_locale: Some(requested_locale.to_string()),
                tenant_default_locale: None,
            })
            .await?;
        return Ok(profile.map(Into::into));
    }

    let profile = ProfileService::new(db.clone())
        .find_profile_summary(tenant_id, author_id, Some(requested_locale), None)
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

    Ok(profile.map(Into::into))
}
