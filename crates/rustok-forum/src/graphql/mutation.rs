use async_graphql::{Context, FieldError, Object, Result, dataloader::DataLoader};
use rustok_api::Permission;
use rustok_api::graphql::{GraphQLError, require_module_enabled};
use rustok_api::{AuthContext, TenantContext};
use rustok_outbox::TransactionalEventBus;
use rustok_profiles::{
    ProfileService, ProfileSummaryLoader, ProfileSummaryLoaderKey, ProfilesReader,
    graphql::GqlProfileSummary,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::moderation_transport::{ForumModerationTransport, moderation_audience_port_context};
use crate::reply_create_transport::{
    ForumReplyCreateTransport, reply_create_audience_port_context,
};
use crate::topic_create_transport::{
    ForumTopicCreateTransport, topic_create_audience_port_context,
};
use crate::{
    CategoryResponse, CategoryService, ReplyService, SubscriptionService, TopicService, VoteService,
};

use super::{ForumGraphqlRuntimeData, require_forum_permission, resolve_tenant_scope, types::*};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub struct ForumContentMutation;

#[Object]
impl ForumContentMutation {
    async fn create_forum_topic(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
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

        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let audience_context = topic_create_audience_port_context(
            ForumTopicCreateTransport::Graphql,
            tenant_id,
            &auth,
            ctx.data_opt::<rustok_api::RequestContext>(),
            tenant.default_locale.as_str(),
        )?;
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();
        let topic = runtime
            .topic_service(db.clone(), event_bus.clone())
            .create_with_audience_context(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                audience_context,
                crate::CreateTopicInput {
                    locale: input.locale,
                    category_id: input.category_id,
                    title: input.title,
                    slug: input.slug,
                    body: input.body,
                    metadata: input.metadata.unwrap_or_else(|| serde_json::json!({})),
                    tags: input.tags,
                    channel_slugs: input.channel_slugs,
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

        Ok(map_topic(topic, author_profile))
    }

    async fn update_forum_topic(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
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

        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let service = TopicService::new(db.clone(), event_bus.clone());
        let topic = service
            .update(
                tenant_id,
                id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                crate::UpdateTopicInput {
                    locale: input.locale,
                    title: input.title,
                    body: input.body,
                    metadata: input.metadata,
                    tags: input.tags,
                    channel_slugs: input.channel_slugs,
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

        Ok(map_topic(topic, author_profile))
    }

    async fn delete_forum_topic(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
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

        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let service = TopicService::new(db.clone(), event_bus.clone());
        service
            .delete(
                tenant_id,
                id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
            )
            .await?;

        Ok(true)
    }

    async fn set_forum_category_subscription(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> Result<GqlForumCategory> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_READ],
            "Permission denied: forum_categories:read required",
        )?;

        SubscriptionService::new(db.clone())
            .set_category_subscription(
                tenant_id,
                category_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
            )
            .await?;

        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let category = CategoryService::new(db.clone())
            .get_with_locale_fallback(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                category_id,
                tenant.default_locale.as_str(),
                Some(tenant.default_locale.as_str()),
            )
            .await?;

        Ok(map_category(category))
    }

    async fn clear_forum_category_subscription(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> Result<GqlForumCategory> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_READ],
            "Permission denied: forum_categories:read required",
        )?;

        SubscriptionService::new(db.clone())
            .clear_category_subscription(
                tenant_id,
                category_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
            )
            .await?;

        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let category = CategoryService::new(db.clone())
            .get_with_locale_fallback(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                category_id,
                tenant.default_locale.as_str(),
                Some(tenant.default_locale.as_str()),
            )
            .await?;

        Ok(map_category(category))
    }

    async fn set_forum_topic_subscription(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: Option<String>,
    ) -> Result<GqlForumTopic> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_READ],
            "Permission denied: forum_topics:read required",
        )?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let resolved_locale = locale.unwrap_or_else(|| tenant.default_locale.clone());

        SubscriptionService::new(db.clone())
            .set_topic_subscription(
                tenant_id,
                topic_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
            )
            .await?;

        let topic = TopicService::new(db.clone(), event_bus.clone())
            .get_with_locale_fallback(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                topic_id,
                resolved_locale.as_str(),
                Some(tenant.default_locale.as_str()),
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

        Ok(map_topic(topic, author_profile))
    }

    async fn clear_forum_topic_subscription(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: Option<String>,
    ) -> Result<GqlForumTopic> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_READ],
            "Permission denied: forum_topics:read required",
        )?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let resolved_locale = locale.unwrap_or_else(|| tenant.default_locale.clone());

        SubscriptionService::new(db.clone())
            .clear_topic_subscription(
                tenant_id,
                topic_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
            )
            .await?;

        let topic = TopicService::new(db.clone(), event_bus.clone())
            .get_with_locale_fallback(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                topic_id,
                resolved_locale.as_str(),
                Some(tenant.default_locale.as_str()),
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

        Ok(map_topic(topic, author_profile))
    }

    async fn create_forum_reply(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
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

        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let audience_context = reply_create_audience_port_context(
            ForumReplyCreateTransport::Graphql,
            tenant_id,
            &auth,
            ctx.data_opt::<rustok_api::RequestContext>(),
            tenant.default_locale.as_str(),
        )?;
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();
        let reply = runtime
            .reply_service(db.clone(), event_bus.clone())
            .create_with_audience_context(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                topic_id,
                audience_context,
                crate::CreateReplyInput {
                    locale: input.locale,
                    content: input.content,
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
            content_plain_text: reply.content_plain_text,
            status: reply.status,
            vote_score: reply.vote_score,
            current_user_vote: reply.current_user_vote,
            is_solution: reply.is_solution,
            parent_reply_id: reply.parent_reply_id,
            created_at: reply.created_at,
            updated_at: reply.updated_at,
        })
    }

    async fn set_forum_topic_vote(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        topic_id: Uuid,
        value: i32,
        locale: Option<String>,
    ) -> Result<GqlForumTopic> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_READ],
            "Permission denied: forum_topics:read required",
        )?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let resolved_locale = locale.unwrap_or_else(|| tenant.default_locale.clone());

        VoteService::new(db.clone())
            .set_topic_vote(
                tenant_id,
                topic_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                value,
            )
            .await?;

        let topic = TopicService::new(db.clone(), event_bus.clone())
            .get_with_locale_fallback(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                topic_id,
                resolved_locale.as_str(),
                Some(tenant.default_locale.as_str()),
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

        Ok(map_topic(topic, author_profile))
    }

    async fn clear_forum_topic_vote(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: Option<String>,
    ) -> Result<GqlForumTopic> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_READ],
            "Permission denied: forum_topics:read required",
        )?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let resolved_locale = locale.unwrap_or_else(|| tenant.default_locale.clone());

        VoteService::new(db.clone())
            .clear_topic_vote(
                tenant_id,
                topic_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
            )
            .await?;

        let topic = TopicService::new(db.clone(), event_bus.clone())
            .get_with_locale_fallback(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                topic_id,
                resolved_locale.as_str(),
                Some(tenant.default_locale.as_str()),
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

        Ok(map_topic(topic, author_profile))
    }

    async fn set_forum_reply_vote(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        reply_id: Uuid,
        value: i32,
        locale: Option<String>,
    ) -> Result<GqlForumReply> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_REPLIES_READ],
            "Permission denied: forum_replies:read required",
        )?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let resolved_locale = locale.unwrap_or_else(|| tenant.default_locale.clone());

        VoteService::new(db.clone())
            .set_reply_vote(
                tenant_id,
                reply_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                value,
            )
            .await?;

        let reply = ReplyService::new(db.clone(), event_bus.clone())
            .get_with_locale_fallback(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                reply_id,
                resolved_locale.as_str(),
                Some(tenant.default_locale.as_str()),
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
            content_plain_text: reply.content_plain_text,
            status: reply.status,
            vote_score: reply.vote_score,
            current_user_vote: reply.current_user_vote,
            is_solution: reply.is_solution,
            parent_reply_id: reply.parent_reply_id,
            created_at: reply.created_at,
            updated_at: reply.updated_at,
        })
    }

    async fn clear_forum_reply_vote(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        reply_id: Uuid,
        locale: Option<String>,
    ) -> Result<GqlForumReply> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_REPLIES_READ],
            "Permission denied: forum_replies:read required",
        )?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let resolved_locale = locale.unwrap_or_else(|| tenant.default_locale.clone());

        VoteService::new(db.clone())
            .clear_reply_vote(
                tenant_id,
                reply_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
            )
            .await?;

        let reply = ReplyService::new(db.clone(), event_bus.clone())
            .get_with_locale_fallback(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                reply_id,
                resolved_locale.as_str(),
                Some(tenant.default_locale.as_str()),
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
            content_plain_text: reply.content_plain_text,
            status: reply.status,
            vote_score: reply.vote_score,
            current_user_vote: reply.current_user_vote,
            is_solution: reply.is_solution,
            parent_reply_id: reply.parent_reply_id,
            created_at: reply.created_at,
            updated_at: reply.updated_at,
        })
    }

    async fn mark_forum_topic_solution(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        topic_id: Uuid,
        reply_id: Uuid,
        locale: Option<String>,
    ) -> Result<GqlForumTopic> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
            .clone();
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, Some(tenant_id))?;
        let audience_context = moderation_audience_port_context(
            ForumModerationTransport::Graphql,
            tenant_id,
            &auth,
            ctx.data_opt::<rustok_api::RequestContext>(),
            tenant.default_locale.as_str(),
        )?;
        let resolved_locale = locale.unwrap_or_else(|| tenant.default_locale.clone());
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();

        runtime
            .moderation_service(db.clone(), event_bus.clone())
            .mark_solution_with_audience_context(
                tenant_id,
                topic_id,
                reply_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                audience_context,
            )
            .await?;

        let topic = TopicService::new(db.clone(), event_bus.clone())
            .get_with_locale_fallback(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                topic_id,
                resolved_locale.as_str(),
                Some(tenant.default_locale.as_str()),
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

        Ok(map_topic(topic, author_profile))
    }

    async fn clear_forum_topic_solution(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: Option<String>,
    ) -> Result<GqlForumTopic> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
            .clone();
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, Some(tenant_id))?;
        let audience_context = moderation_audience_port_context(
            ForumModerationTransport::Graphql,
            tenant_id,
            &auth,
            ctx.data_opt::<rustok_api::RequestContext>(),
            tenant.default_locale.as_str(),
        )?;
        let resolved_locale = locale.unwrap_or_else(|| tenant.default_locale.clone());
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();

        runtime
            .moderation_service(db.clone(), event_bus.clone())
            .clear_solution_with_audience_context(
                tenant_id,
                topic_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                audience_context,
            )
            .await?;

        let topic = TopicService::new(db.clone(), event_bus.clone())
            .get_with_locale_fallback(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                topic_id,
                resolved_locale.as_str(),
                Some(tenant.default_locale.as_str()),
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

        Ok(map_topic(topic, author_profile))
    }

    async fn create_forum_category(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        input: CreateForumCategoryInput,
    ) -> Result<GqlForumCategory> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_CREATE],
            "Permission denied: forum_categories:create required",
        )?;

        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let service = CategoryService::new(db.clone());
        let category = service
            .create(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
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

        Ok(map_category(category))
    }

    async fn update_forum_category(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        id: Uuid,
        input: UpdateForumCategoryInput,
    ) -> Result<GqlForumCategory> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_UPDATE],
            "Permission denied: forum_categories:update required",
        )?;

        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let category = CategoryService::new(db.clone())
            .update(
                tenant_id,
                id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                crate::UpdateCategoryInput {
                    locale: input.locale,
                    name: input.name,
                    slug: input.slug,
                    description: input.description,
                    icon: input.icon,
                    color: input.color,
                    position: input.position,
                    moderated: input.moderated,
                },
            )
            .await?;

        Ok(map_category(category))
    }

    async fn delete_forum_category(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        id: Uuid,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_DELETE],
            "Permission denied: forum_categories:delete required",
        )?;

        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        CategoryService::new(db.clone())
            .delete(
                tenant_id,
                id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
            )
            .await?;

        Ok(true)
    }
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

fn map_category(category: CategoryResponse) -> GqlForumCategory {
    GqlForumCategory {
        id: category.id,
        requested_locale: category.requested_locale,
        locale: category.locale,
        effective_locale: category.effective_locale,
        available_locales: category.available_locales,
        name: category.name,
        slug: category.slug,
        description: category.description,
        icon: category.icon,
        color: category.color,
        parent_id: category.parent_id,
        position: category.position,
        topic_count: category.topic_count,
        reply_count: category.reply_count,
        moderated: category.moderated,
        is_subscribed: category.is_subscribed,
    }
}

fn map_topic(
    topic: crate::TopicResponse,
    author_profile: Option<GqlProfileSummary>,
) -> GqlForumTopic {
    GqlForumTopic {
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
        body_plain_text: topic.body_plain_text,
        metadata: topic.metadata,
        status: topic.status,
        tags: topic.tags,
        channel_slugs: topic.channel_slugs,
        vote_score: topic.vote_score,
        current_user_vote: topic.current_user_vote,
        is_subscribed: topic.is_subscribed,
        solution_reply_id: topic.solution_reply_id,
        is_pinned: topic.is_pinned,
        is_locked: topic.is_locked,
        reply_count: topic.reply_count,
        created_at: topic.created_at,
        updated_at: topic.updated_at,
    }
}
