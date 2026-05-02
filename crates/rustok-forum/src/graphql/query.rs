use async_graphql::{dataloader::DataLoader, Context, ErrorExtensions, FieldError, Object, Result};
use rustok_api::{
    graphql::{require_module_enabled, resolve_graphql_locale, GraphQLError, PaginationInput},
    has_any_effective_permission, AuthContext, RequestContext, TenantContext,
};
use rustok_channel::ChannelService;
use rustok_core::{Permission, SecurityContext};
use rustok_outbox::TransactionalEventBus;
use rustok_profiles::{
    graphql::GqlProfileSummary, ProfileService, ProfileSummaryLoader, ProfileSummaryLoaderKey,
    ProfilesReader,
};
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use uuid::Uuid;

use crate::{
    CategoryListItem, CategoryService, ForumError, ForumResult, ReplyResponse, ReplyService,
    TopicListItem, TopicResponse, TopicService, UserStatsService,
};

use super::types::*;

const MODULE_SLUG: &str = "forum";
const PUBLIC_REPLY_STATUSES: [&str; 1] = [crate::constants::reply_status::APPROVED];

#[derive(Default)]
pub struct ForumQuery;

#[Object]
impl ForumQuery {
    async fn forum_categories(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        locale: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<ForumCategoryConnection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_LIST],
            "Permission denied: forum_categories:list required",
        )?;

        let tenant = ctx.data::<TenantContext>()?;
        let service = CategoryService::new(db.clone());
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let page = (offset / limit + 1) as u64;
        let per_page = limit as u64;

        let list_started_at = Instant::now();
        let (categories, total) = service
            .list_paginated_with_locale_fallback(
                tenant_id,
                auth.security_context(),
                &locale,
                page,
                per_page,
                Some(tenant.default_locale.as_str()),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.categories",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );

        let items = categories
            .into_iter()
            .map(map_category_list_item)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "forum.categories",
            Some(requested_limit),
            per_page,
            items.len(),
        );

        Ok(ForumCategoryConnection::new(
            items,
            total as i64,
            offset,
            limit,
        ))
    }

    async fn forum_topics(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        category_id: Option<Uuid>,
        locale: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<ForumTopicConnection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_LIST],
            "Permission denied: forum_topics:list required",
        )?;

        let tenant = ctx.data::<TenantContext>()?;
        let service = TopicService::new(db.clone(), event_bus.clone());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let filter = crate::ListTopicsFilter {
            category_id,
            status: None,
            locale: Some(locale.clone()),
            page: (offset / limit + 1) as u64,
            per_page: limit as u64,
        };

        let list_started_at = Instant::now();
        let (topics, total) = service
            .list_with_locale_fallback(
                tenant_id,
                auth.security_context(),
                filter,
                Some(tenant.default_locale.as_str()),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.topics",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );

        let author_profiles = load_author_profiles_map(
            ctx,
            db,
            tenant_id,
            topics.iter().map(|topic| topic.author_id),
            locale.as_str(),
            tenant.default_locale.as_str(),
        )
        .await?;
        let items = topics
            .into_iter()
            .map(|topic| {
                let author_profile = topic
                    .author_id
                    .and_then(|author_id| author_profiles.get(&author_id).cloned());
                map_topic_list_item(topic, author_profile)
            })
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "forum.topics",
            Some(requested_limit),
            limit as u64,
            items.len(),
        );

        Ok(ForumTopicConnection::new(
            items,
            total as i64,
            offset,
            limit,
        ))
    }

    async fn forum_replies(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<ForumReplyConnection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_REPLIES_LIST],
            "Permission denied: forum_replies:list required",
        )?;

        let tenant = ctx.data::<TenantContext>()?;
        let service = ReplyService::new(db.clone(), event_bus.clone());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let filter = crate::ListRepliesFilter {
            locale: Some(locale.clone()),
            page: (offset / limit + 1) as u64,
            per_page: limit as u64,
        };

        let list_started_at = Instant::now();
        let (replies, total) = service
            .list_response_for_topic_with_locale_fallback(
                tenant_id,
                auth.security_context(),
                topic_id,
                filter,
                Some(tenant.default_locale.as_str()),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.replies",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );

        let author_profiles = load_author_profiles_map(
            ctx,
            db,
            tenant_id,
            replies.iter().map(|reply| reply.author_id),
            locale.as_str(),
            tenant.default_locale.as_str(),
        )
        .await?;
        let items = replies
            .into_iter()
            .map(|reply| {
                let author_profile = reply
                    .author_id
                    .and_then(|author_id| author_profiles.get(&author_id).cloned());
                map_reply_response(reply, author_profile)
            })
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "forum.replies",
            Some(requested_limit),
            limit as u64,
            items.len(),
        );

        Ok(ForumReplyConnection::new(
            items,
            total as i64,
            offset,
            limit,
        ))
    }

    async fn forum_user_stats(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<GqlForumUserStats> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_READ],
            "Permission denied: forum_topics:read required",
        )?;

        let stats = UserStatsService::new(db.clone())
            .get(tenant_id, auth.security_context(), user_id)
            .await?;

        Ok(GqlForumUserStats {
            user_id: stats.user_id,
            topic_count: stats.topic_count,
            reply_count: stats.reply_count,
            solution_count: stats.solution_count,
            updated_at: stats.updated_at,
        })
    }

    async fn forum_storefront_categories(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        locale: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<ForumCategoryConnection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_forum_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let resolved_tenant_id = tenant_id.unwrap_or(tenant.id);
        let service = CategoryService::new(db.clone());
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let page = (offset / limit + 1) as u64;
        let per_page = limit as u64;

        let list_started_at = Instant::now();
        let (categories, total) = service
            .list_paginated_with_locale_fallback(
                resolved_tenant_id,
                forum_security_or_system(ctx),
                &locale,
                page,
                per_page,
                Some(tenant.default_locale.as_str()),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.storefront_categories",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );

        let items = categories
            .into_iter()
            .map(map_category_list_item)
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "forum.storefront_categories",
            Some(requested_limit),
            per_page,
            items.len(),
        );

        Ok(ForumCategoryConnection::new(
            items,
            total as i64,
            offset,
            limit,
        ))
    }

    async fn forum_storefront_topics(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        category_id: Option<Uuid>,
        locale: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<ForumTopicConnection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_forum_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let resolved_tenant_id = tenant_id.unwrap_or(tenant.id);
        let service = TopicService::new(db.clone(), event_bus.clone());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let filter = crate::ListTopicsFilter {
            category_id,
            status: None,
            locale: Some(locale.clone()),
            page: (offset / limit + 1) as u64,
            per_page: limit as u64,
        };

        let list_started_at = Instant::now();
        let (topics, total) = list_public_storefront_topics(
            &service,
            resolved_tenant_id,
            forum_security_or_system(ctx),
            filter,
            Some(tenant.default_locale.as_str()),
            public_channel_slug(ctx),
        )
        .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.storefront_topics",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );

        let author_profiles = load_author_profiles_map(
            ctx,
            db,
            resolved_tenant_id,
            topics.iter().map(|topic| topic.author_id),
            locale.as_str(),
            tenant.default_locale.as_str(),
        )
        .await?;
        let items = topics
            .into_iter()
            .map(|topic| {
                let author_profile = topic
                    .author_id
                    .and_then(|author_id| author_profiles.get(&author_id).cloned());
                map_topic_list_item(topic, author_profile)
            })
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "forum.storefront_topics",
            Some(requested_limit),
            limit as u64,
            items.len(),
        );

        Ok(ForumTopicConnection::new(
            items,
            total as i64,
            offset,
            limit,
        ))
    }

    async fn forum_storefront_topic(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
        locale: Option<String>,
    ) -> Result<Option<GqlForumTopic>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_forum_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let resolved_tenant_id = tenant_id.unwrap_or(tenant.id);
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let service = TopicService::new(db.clone(), event_bus.clone());

        let topic = match service
            .get_with_locale_fallback(
                resolved_tenant_id,
                forum_security_or_system(ctx),
                id,
                &locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(topic) => topic,
            Err(ForumError::TopicNotFound(_)) => return Ok(None),
            Err(err) => return Err(async_graphql::Error::new(err.to_string())),
        };

        if is_public_request(ctx)
            && (topic.status != crate::constants::topic_status::OPEN
                || !is_topic_visible_for_channel(
                    &topic.channel_slugs,
                    public_channel_slug(ctx).as_deref(),
                ))
        {
            return Ok(None);
        }

        let author_profiles = load_author_profiles_map(
            ctx,
            db,
            resolved_tenant_id,
            [topic.author_id],
            locale.as_str(),
            tenant.default_locale.as_str(),
        )
        .await?;

        let author_profile = topic
            .author_id
            .and_then(|author_id| author_profiles.get(&author_id).cloned());

        Ok(Some(map_topic_response(topic, author_profile)))
    }

    async fn forum_storefront_replies(
        &self,
        ctx: &Context<'_>,
        topic_id: Uuid,
        tenant_id: Option<Uuid>,
        locale: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<ForumReplyConnection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_forum_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let resolved_tenant_id = tenant_id.unwrap_or(tenant.id);
        let service = ReplyService::new(db.clone(), event_bus.clone());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let topic_service = TopicService::new(db.clone(), event_bus.clone());
        let topic = match topic_service
            .get_with_locale_fallback(
                resolved_tenant_id,
                forum_security_or_system(ctx),
                topic_id,
                &locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(topic) => Some(topic),
            Err(ForumError::TopicNotFound(_)) => None,
            Err(err) => return Err(async_graphql::Error::new(err.to_string())),
        };

        let topic_is_visible = topic.as_ref().is_some_and(|topic| {
            is_storefront_topic_visible(
                &topic.status,
                &topic.channel_slugs,
                public_channel_slug(ctx).as_deref(),
            )
        });

        if !topic_is_visible {
            return Ok(ForumReplyConnection::new(Vec::new(), 0, offset, limit));
        }

        let filter = crate::ListRepliesFilter {
            locale: Some(locale.clone()),
            page: (offset / limit + 1) as u64,
            per_page: limit as u64,
        };

        let list_started_at = Instant::now();
        let (replies, total) = service
            .list_response_for_topic_by_statuses_with_locale_fallback(
                resolved_tenant_id,
                forum_security_or_system(ctx),
                topic_id,
                filter,
                Some(tenant.default_locale.as_str()),
                Some(&PUBLIC_REPLY_STATUSES),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.storefront_replies",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );

        let author_profiles = load_author_profiles_map(
            ctx,
            db,
            resolved_tenant_id,
            replies.iter().map(|reply| reply.author_id),
            locale.as_str(),
            tenant.default_locale.as_str(),
        )
        .await?;
        let items = replies
            .into_iter()
            .map(|reply| {
                let author_profile = reply
                    .author_id
                    .and_then(|author_id| author_profiles.get(&author_id).cloned());
                map_reply_response(reply, author_profile)
            })
            .collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "forum.storefront_replies",
            Some(requested_limit),
            limit as u64,
            items.len(),
        );

        Ok(ForumReplyConnection::new(
            items,
            total as i64,
            offset,
            limit,
        ))
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

fn forum_security_or_system(ctx: &Context<'_>) -> SecurityContext {
    ctx.data::<AuthContext>()
        .map(|auth| auth.security_context())
        .unwrap_or_else(|_| SecurityContext::system())
}

fn map_category_list_item(category: CategoryListItem) -> GqlForumCategory {
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
        topic_count: category.topic_count,
        reply_count: category.reply_count,
        is_subscribed: category.is_subscribed,
    }
}

fn map_topic_list_item(
    topic: TopicListItem,
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
        body: String::new(),
        body_format: "markdown".to_string(),
        metadata: topic.metadata,
        status: topic.status,
        tags: Vec::new(),
        channel_slugs: topic.channel_slugs,
        vote_score: topic.vote_score,
        current_user_vote: topic.current_user_vote,
        is_subscribed: topic.is_subscribed,
        solution_reply_id: topic.solution_reply_id,
        is_pinned: topic.is_pinned,
        is_locked: topic.is_locked,
        reply_count: topic.reply_count,
        created_at: topic.created_at,
        updated_at: String::new(),
    }
}

fn map_topic_response(
    topic: TopicResponse,
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
        body_format: topic.body_format,
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

fn map_reply_response(
    reply: ReplyResponse,
    author_profile: Option<GqlProfileSummary>,
) -> GqlForumReply {
    GqlForumReply {
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
        vote_score: reply.vote_score,
        current_user_vote: reply.current_user_vote,
        is_solution: reply.is_solution,
        parent_reply_id: reply.parent_reply_id,
        created_at: reply.created_at,
        updated_at: reply.updated_at,
    }
}

async fn load_author_profiles_map<I>(
    ctx: &Context<'_>,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    author_ids: I,
    requested_locale: &str,
    tenant_default_locale: &str,
) -> Result<HashMap<Uuid, GqlProfileSummary>>
where
    I: IntoIterator<Item = Option<Uuid>>,
{
    let user_ids = author_ids
        .into_iter()
        .flatten()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if user_ids.is_empty() {
        return Ok(HashMap::new());
    }

    if let Some(loader) = ctx.data_opt::<DataLoader<ProfileSummaryLoader>>() {
        let keys = user_ids
            .iter()
            .map(|user_id| ProfileSummaryLoaderKey {
                tenant_id,
                user_id: *user_id,
                requested_locale: Some(requested_locale.to_string()),
                tenant_default_locale: Some(tenant_default_locale.to_string()),
            })
            .collect::<Vec<_>>();
        let profiles = loader.load_many(keys).await?;
        return Ok(profiles
            .into_iter()
            .map(|(key, summary)| (key.user_id, summary.into()))
            .collect());
    }

    let profiles = ProfileService::new(db.clone())
        .find_profile_summaries(
            tenant_id,
            &user_ids,
            Some(requested_locale),
            Some(tenant_default_locale),
        )
        .await
        .map_err(|err| async_graphql::Error::new(err.to_string()))?;

    Ok(profiles
        .into_iter()
        .map(|(user_id, summary)| (user_id, summary.into()))
        .collect())
}

async fn require_public_forum_channel_enabled(ctx: &Context<'_>) -> Result<()> {
    let db = ctx.data::<DatabaseConnection>()?;
    ensure_public_forum_channel_enabled(
        db,
        ctx.data_opt::<RequestContext>(),
        ctx.data_opt::<AuthContext>().is_some(),
    )
    .await
}

async fn ensure_public_forum_channel_enabled(
    db: &DatabaseConnection,
    request_context: Option<&RequestContext>,
    is_authenticated: bool,
) -> Result<()> {
    if is_authenticated {
        return Ok(());
    }

    let Some(request_context) = request_context else {
        return Ok(());
    };
    let Some(channel_id) = request_context.channel_id else {
        return Ok(());
    };

    let enabled = ChannelService::new(db.clone())
        .is_module_enabled(channel_id, MODULE_SLUG)
        .await
        .map_err(|error| {
            async_graphql::Error::new(format!("Channel module check failed: {error}"))
                .extend_with(|_, ext| ext.set("code", "INTERNAL_SERVER_ERROR"))
        })?;

    if enabled {
        return Ok(());
    }

    Err(
        async_graphql::Error::new("Forum module is not enabled for this channel")
            .extend_with(|_, ext| ext.set("code", "FORBIDDEN")),
    )
}

fn is_public_request(ctx: &Context<'_>) -> bool {
    ctx.data_opt::<AuthContext>().is_none()
}

fn public_channel_slug(ctx: &Context<'_>) -> Option<String> {
    ctx.data_opt::<RequestContext>()
        .and_then(|rc| rc.channel_slug.clone())
}

pub(crate) fn is_topic_visible_for_channel(
    channel_slugs: &[String],
    channel_slug: Option<&str>,
) -> bool {
    if channel_slugs.is_empty() {
        return true;
    }

    let Some(channel_slug) = channel_slug else {
        return false;
    };

    let normalized = channel_slug.trim().to_ascii_lowercase();
    !normalized.is_empty() && channel_slugs.iter().any(|item| item == &normalized)
}

fn is_storefront_topic_visible(
    status: &str,
    channel_slugs: &[String],
    channel_slug: Option<&str>,
) -> bool {
    status == crate::constants::topic_status::OPEN
        && is_topic_visible_for_channel(channel_slugs, channel_slug)
}

async fn list_public_storefront_topics(
    service: &TopicService,
    tenant_id: Uuid,
    security: SecurityContext,
    base_filter: crate::ListTopicsFilter,
    fallback_locale: Option<&str>,
    channel_slug: Option<String>,
) -> ForumResult<(Vec<TopicListItem>, u64)> {
    service
        .list_storefront_visible_with_locale_fallback(
            tenant_id,
            security,
            base_filter,
            fallback_locale,
            channel_slug.as_deref(),
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::{
        is_storefront_topic_visible, is_topic_visible_for_channel, list_public_storefront_topics,
        ForumQuery,
    };
    use crate::{
        migrations, CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
        ReplyService, TopicService,
    };
    use async_graphql::{EmptyMutation, EmptySubscription, Schema};
    use rustok_api::{RequestContext, TenantContext};
    use rustok_core::{MemoryTransport, SecurityContext};
    use rustok_outbox::TransactionalEventBus;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
    use sea_orm_migration::SchemaManager;
    use std::sync::Arc;
    use uuid::Uuid;

    async fn setup_forum_query_db() -> DatabaseConnection {
        let db_url = format!(
            "sqlite:file:forum_query_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut opts = ConnectOptions::new(db_url);
        opts.max_connections(5)
            .min_connections(1)
            .sqlx_logging(false);

        Database::connect(opts)
            .await
            .expect("failed to connect forum query test sqlite database")
    }

    async fn ensure_forum_query_schema(db: &DatabaseConnection) {
        let manager = SchemaManager::new(db);
        for migration in migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("forum migration should apply");
        }

        db.execute(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE IF NOT EXISTS tenant_modules (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                module_slug TEXT NOT NULL,
                enabled BOOLEAN NOT NULL,
                settings TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#
            .to_string(),
        ))
        .await
        .expect("tenant_modules table should exist");
    }

    async fn enable_forum_module(db: &DatabaseConnection, tenant_id: Uuid) {
        db.execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at) VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [
                Uuid::new_v4().into(),
                tenant_id.into(),
                "forum".to_string().into(),
                true.into(),
                "{}".to_string().into(),
            ],
        ))
        .await
        .expect("forum module should be enabled");
    }

    fn tenant_context(tenant_id: Uuid) -> TenantContext {
        TenantContext {
            id: tenant_id,
            name: "Forum Tenant".to_string(),
            slug: "forum-tenant".to_string(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn request_context(tenant_id: Uuid, channel_slug: Option<&str>) -> RequestContext {
        RequestContext {
            tenant_id,
            user_id: None,
            channel_id: None,
            channel_slug: channel_slug.map(|slug| slug.to_string()),
            channel_resolution_source: None,
            locale: "en".to_string(),
        }
    }

    #[test]
    fn topic_visibility_without_channel_restriction_is_public() {
        assert!(is_topic_visible_for_channel(&[], None));
    }

    #[test]
    fn topic_visibility_requires_matching_channel_when_restricted() {
        let channels = vec!["news".to_string()];
        assert!(is_topic_visible_for_channel(&channels, Some("news")));
        assert!(is_topic_visible_for_channel(&channels, Some(" NEWS ")));
        assert!(!is_topic_visible_for_channel(&channels, Some("blog")));
        assert!(!is_topic_visible_for_channel(&channels, None));
    }

    #[test]
    fn storefront_topic_requires_open_status() {
        let channels = vec!["news".to_string()];
        assert!(is_storefront_topic_visible("open", &channels, Some("news")));
        assert!(!is_storefront_topic_visible(
            "closed",
            &channels,
            Some("news")
        ));
    }

    #[tokio::test]
    async fn storefront_topics_refill_page_after_filtered_items() {
        let db = setup_forum_query_db().await;
        ensure_forum_query_schema(&db).await;

        let transport = MemoryTransport::new();
        let _receiver = transport.subscribe();
        let event_bus = TransactionalEventBus::new(Arc::new(transport));
        let tenant_id = Uuid::new_v4();
        let security = SecurityContext::system();

        let category = CategoryService::new(db.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "General".to_string(),
                    slug: "general".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await
            .expect("category should be created");

        let service = TopicService::new(db.clone(), event_bus.clone());
        let first_visible = service
            .create(
                tenant_id,
                security.clone(),
                CreateTopicInput {
                    locale: "en".to_string(),
                    category_id: category.id,
                    title: "Visible One".to_string(),
                    slug: Some("visible-one".to_string()),
                    body: "Body".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    channel_slugs: Some(vec!["web".to_string()]),
                },
            )
            .await
            .expect("first visible topic should be created");
        let closed_topic = service
            .create(
                tenant_id,
                security.clone(),
                CreateTopicInput {
                    locale: "en".to_string(),
                    category_id: category.id,
                    title: "Closed".to_string(),
                    slug: Some("closed".to_string()),
                    body: "Body".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    channel_slugs: None,
                },
            )
            .await
            .expect("closed topic should be created");
        use crate::entities::forum_topic;
        use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
        let closed_model = forum_topic::Entity::find_by_id(closed_topic.id)
            .one(&db)
            .await
            .expect("closed topic lookup should succeed")
            .expect("closed topic should exist");
        let mut closed_active: forum_topic::ActiveModel = closed_model.into();
        closed_active.status = Set(crate::constants::topic_status::CLOSED.to_string());
        closed_active
            .update(&db)
            .await
            .expect("status update should succeed");

        let _channel_filtered = service
            .create(
                tenant_id,
                security.clone(),
                CreateTopicInput {
                    locale: "en".to_string(),
                    category_id: category.id,
                    title: "Mobile Only".to_string(),
                    slug: Some("mobile-only".to_string()),
                    body: "Body".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    channel_slugs: Some(vec!["mobile".to_string()]),
                },
            )
            .await
            .expect("channel filtered topic should be created");
        let second_visible = service
            .create(
                tenant_id,
                security,
                CreateTopicInput {
                    locale: "en".to_string(),
                    category_id: category.id,
                    title: "Visible Two".to_string(),
                    slug: Some("visible-two".to_string()),
                    body: "Body".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    channel_slugs: Some(vec!["web".to_string()]),
                },
            )
            .await
            .expect("second visible topic should be created");

        let (topics, total) = list_public_storefront_topics(
            &service,
            tenant_id,
            SecurityContext::system(),
            crate::ListTopicsFilter {
                category_id: Some(category.id),
                status: None,
                locale: Some("en".to_string()),
                page: 1,
                per_page: 2,
            },
            Some("en"),
            Some("web".to_string()),
        )
        .await
        .expect("storefront topic list should succeed");

        assert_eq!(total, 2);
        assert_eq!(topics.len(), 2);
        let ids = topics.into_iter().map(|topic| topic.id).collect::<Vec<_>>();
        assert!(ids.contains(&first_visible.id));
        assert!(ids.contains(&second_visible.id));
    }

    #[tokio::test]
    async fn storefront_replies_return_empty_for_channel_ineligible_topic() {
        let db = setup_forum_query_db().await;
        ensure_forum_query_schema(&db).await;

        let transport = MemoryTransport::new();
        let _receiver = transport.subscribe();
        let event_bus = TransactionalEventBus::new(Arc::new(transport));
        let tenant_id = Uuid::new_v4();
        enable_forum_module(&db, tenant_id).await;

        let security = SecurityContext::system();
        let category = CategoryService::new(db.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "General".to_string(),
                    slug: "general".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await
            .expect("category should be created");

        let topic = TopicService::new(db.clone(), event_bus.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateTopicInput {
                    locale: "en".to_string(),
                    category_id: category.id,
                    title: "Restricted".to_string(),
                    slug: Some("restricted".to_string()),
                    body: "Body".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    channel_slugs: Some(vec!["mobile".to_string()]),
                },
            )
            .await
            .expect("restricted topic should be created");

        ReplyService::new(db.clone(), event_bus.clone())
            .create(
                tenant_id,
                security,
                topic.id,
                CreateReplyInput {
                    locale: "en".to_string(),
                    content: "Reply".to_string(),
                    content_format: "markdown".to_string(),
                    content_json: None,
                    parent_reply_id: None,
                },
            )
            .await
            .expect("reply should be created");

        let schema = Schema::build(ForumQuery, EmptyMutation, EmptySubscription)
            .data(db.clone())
            .data(event_bus)
            .data(tenant_context(tenant_id))
            .data(request_context(tenant_id, Some("web")))
            .finish();

        let response = schema
            .execute(format!(
                "{{ forumStorefrontReplies(topicId: \"{}\") {{ pageInfo {{ totalCount }} items {{ id }} }} }}",
                topic.id
            ))
            .await;
        assert!(
            response.errors.is_empty(),
            "storefront replies query should not error: {:?}",
            response.errors
        );

        let data = response
            .data
            .into_json()
            .expect("graphql data should be json");
        assert_eq!(data["forumStorefrontReplies"]["pageInfo"]["totalCount"], 0);
        assert_eq!(
            data["forumStorefrontReplies"]["items"]
                .as_array()
                .expect("items should be array")
                .len(),
            0
        );
    }
}
