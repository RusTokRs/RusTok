use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result, dataloader::DataLoader};
use rustok_api::Permission;
use rustok_api::{
    AuthContext, RequestContext, TenantContext,
    graphql::{GraphQLError, PaginationInput, require_module_enabled, resolve_graphql_locale},
    has_any_effective_permission,
};
use rustok_channel::ChannelService;
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use rustok_profiles::{
    ProfilePresentationService, ProfileSummaryLoader, ProfileSummaryLoaderKey,
    graphql::GqlProfileSummary,
};
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use uuid::Uuid;

use crate::{
    CategoryListItem, CategoryResponse, ForumCategoryReadOperation, ForumCategoryReadTransport,
    ForumError, ForumReplyReadOperation, ForumReplyReadTransport, ForumResult,
    ForumWidgetCatalogResponse, ForumWidgetContractService, ReplyResponse, ReplyStatus,
    TopicListItem, TopicResponse, TopicService, UserStatsService,
    category_read_audience_port_context, reply_read_audience_port_context,
};

use super::{ForumGraphqlRuntimeData, types::*};

const MODULE_SLUG: &str = "forum";
const PUBLIC_REPLY_STATUSES: [ReplyStatus; 1] = [ReplyStatus::Approved];

#[derive(Default)]
pub struct ForumContentQuery;

#[Object]
impl ForumContentQuery {
    async fn forum_categories(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
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
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let page_number = (offset / limit + 1) as u64;
        let per_page = limit as u64;
        let audience_context = category_read_audience_port_context(
            ForumCategoryReadTransport::Graphql,
            ForumCategoryReadOperation::CategoryList,
            tenant_id,
            &auth,
            ctx.data_opt::<RequestContext>(),
            locale.as_str(),
        )?;
        let runtime = forum_runtime(ctx);

        let started_at = Instant::now();
        let page = runtime
            .category_audience_read_service(db.clone())
            .list_authenticated_owner_visible_with_audience_context(
                tenant_id,
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions),
                audience_context,
                page_number,
                per_page,
                Some(tenant.default_locale.as_str()),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.categories",
            "exact_category_audience_owner",
            started_at.elapsed().as_secs_f64(),
            page.total,
        );

        let items = page
            .items
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
            page.total as i64,
            offset,
            limit,
        ))
    }

    async fn forum_topics(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
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
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
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

        let started_at = Instant::now();
        let (topics, total) = service
            .list_with_locale_fallback(
                tenant_id,
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions),
                filter,
                Some(tenant.default_locale.as_str()),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.topics",
            "service_list",
            started_at.elapsed().as_secs_f64(),
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

    async fn forum_category(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
        locale: Option<String>,
    ) -> Result<Option<GqlForumCategory>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_READ],
            "Permission denied: forum_categories:read required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let audience_context = category_read_audience_port_context(
            ForumCategoryReadTransport::Graphql,
            ForumCategoryReadOperation::SelectedCategory,
            tenant_id,
            &auth,
            ctx.data_opt::<RequestContext>(),
            locale.as_str(),
        )?;
        let service = forum_runtime(ctx).category_audience_read_service(db.clone());
        let category = match service
            .get_authenticated_owner_visible_with_audience_context(
                tenant_id,
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions),
                audience_context,
                id,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(category) => category,
            Err(ForumError::CategoryNotFound(_)) => return Ok(None),
            Err(error) => return Err(async_graphql::Error::new(error.to_string())),
        };

        Ok(Some(map_category_response(category)))
    }

    async fn forum_topic(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
        locale: Option<String>,
    ) -> Result<Option<GqlForumTopic>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_READ],
            "Permission denied: forum_topics:read required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let service = TopicService::new(db.clone(), event_bus.clone());
        let topic = match service
            .get_with_locale_fallback(
                tenant_id,
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions),
                id,
                &locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(topic) => topic,
            Err(ForumError::TopicNotFound(_)) => return Ok(None),
            Err(error) => return Err(async_graphql::Error::new(error.to_string())),
        };
        let author_profiles = load_author_profiles_map(
            ctx,
            db,
            tenant_id,
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

    async fn forum_replies(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
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
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let filter = crate::ListRepliesFilter {
            locale: Some(locale.clone()),
            page: (offset / limit + 1) as u64,
            per_page: limit as u64,
        };
        let audience_context = reply_read_audience_port_context(
            ForumReplyReadTransport::Graphql,
            ForumReplyReadOperation::ReplyList,
            tenant_id,
            &auth,
            ctx.data_opt::<RequestContext>(),
            locale.as_str(),
        )?;
        let service = forum_runtime(ctx).reply_audience_read_service(db.clone(), event_bus.clone());

        let started_at = Instant::now();
        let (replies, total) = service
            .list_response_authenticated_owner_visible_with_audience_context(
                tenant_id,
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions),
                audience_context,
                topic_id,
                filter,
                Some(tenant.default_locale.as_str()),
                None,
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.replies",
            "exact_reply_audience_owner",
            started_at.elapsed().as_secs_f64(),
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
            .get(
                tenant_id,
                SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions),
                user_id,
            )
            .await?;
        Ok(GqlForumUserStats {
            user_id: stats.user_id,
            topic_count: stats.topic_count,
            reply_count: stats.reply_count,
            solution_count: stats.solution_count,
            updated_at: stats.updated_at,
        })
    }

    async fn forum_widget_catalog(&self, ctx: &Context<'_>) -> Result<GqlForumWidgetCatalog> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_READ],
            "Permission denied: forum_topics:read required",
        )?;
        Ok(map_widget_catalog(ForumWidgetContractService::catalog()))
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
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let page_number = (offset / limit + 1) as u64;
        let per_page = limit as u64;
        let service = forum_runtime(ctx).category_audience_read_service(db.clone());

        let started_at = Instant::now();
        let page = if let Some(auth) = ctx.data_opt::<AuthContext>() {
            let audience_context = category_read_audience_port_context(
                ForumCategoryReadTransport::Graphql,
                ForumCategoryReadOperation::CategoryList,
                tenant_id,
                auth,
                ctx.data_opt::<RequestContext>(),
                locale.as_str(),
            )?;
            service
                .list_authenticated_storefront_visible_with_audience_context(
                    tenant_id,
                    SecurityContext::from_permission_snapshot(
                        Some(auth.user_id),
                        &auth.permissions,
                    ),
                    audience_context,
                    page_number,
                    per_page,
                    Some(tenant.default_locale.as_str()),
                )
                .await?
        } else {
            service
                .list_public_storefront_visible_with_locale_fallback(
                    tenant_id,
                    &locale,
                    page_number,
                    per_page,
                    Some(tenant.default_locale.as_str()),
                )
                .await?
        };
        metrics::record_read_path_query(
            "graphql",
            "forum.storefront_categories",
            "exact_category_audience_owner",
            started_at.elapsed().as_secs_f64(),
            page.total,
        );

        let items = page
            .items
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
            page.total as i64,
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
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
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

        let started_at = Instant::now();
        let (topics, total) = list_public_storefront_topics(
            &service,
            tenant_id,
            forum_request_security(ctx),
            filter,
            Some(tenant.default_locale.as_str()),
            public_channel_slug(ctx),
        )
        .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.storefront_topics",
            "service_list",
            started_at.elapsed().as_secs_f64(),
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
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let service = TopicService::new(db.clone(), event_bus.clone());
        let topic = match service
            .get_with_locale_fallback(
                tenant_id,
                forum_request_security(ctx),
                id,
                &locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(topic) => topic,
            Err(ForumError::TopicNotFound(_)) => return Ok(None),
            Err(error) => return Err(async_graphql::Error::new(error.to_string())),
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
            tenant_id,
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
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let filter = crate::ListRepliesFilter {
            locale: Some(locale.clone()),
            page: (offset / limit + 1) as u64,
            per_page: limit as u64,
        };
        let service = forum_runtime(ctx).reply_audience_read_service(db.clone(), event_bus.clone());

        let started_at = Instant::now();
        let (replies, total) = if let Some(auth) = ctx.data_opt::<AuthContext>() {
            let audience_context = reply_read_audience_port_context(
                ForumReplyReadTransport::Graphql,
                ForumReplyReadOperation::ReplyList,
                tenant_id,
                auth,
                ctx.data_opt::<RequestContext>(),
                locale.as_str(),
            )?;
            service
                .list_authenticated_storefront_visible_with_audience_context(
                    tenant_id,
                    SecurityContext::from_permission_snapshot(
                        Some(auth.user_id),
                        &auth.permissions,
                    ),
                    audience_context,
                    topic_id,
                    filter,
                    Some(tenant.default_locale.as_str()),
                    Some(&PUBLIC_REPLY_STATUSES),
                )
                .await?
        } else {
            service
                .list_public_storefront_visible_with_locale_fallback(
                    tenant_id,
                    topic_id,
                    filter,
                    Some(tenant.default_locale.as_str()),
                    public_channel_slug(ctx).as_deref(),
                    Some(&PUBLIC_REPLY_STATUSES),
                )
                .await?
        };
        metrics::record_read_path_query(
            "graphql",
            "forum.storefront_replies",
            "exact_reply_audience_owner",
            started_at.elapsed().as_secs_f64(),
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

fn forum_runtime(ctx: &Context<'_>) -> ForumGraphqlRuntimeData {
    ctx.data_opt::<ForumGraphqlRuntimeData>()
        .cloned()
        .unwrap_or_default()
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

fn forum_request_security(ctx: &Context<'_>) -> SecurityContext {
    ctx.data_opt::<AuthContext>()
        .map(|auth| {
            SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)
        })
        .unwrap_or_else(SecurityContext::public_read)
}

fn resolve_tenant_scope(tenant: &TenantContext, requested_tenant_id: Option<Uuid>) -> Result<Uuid> {
    match requested_tenant_id {
        Some(requested_tenant_id) if requested_tenant_id != tenant.id => {
            Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: tenant scope mismatch",
            ))
        }
        Some(requested_tenant_id) => Ok(requested_tenant_id),
        None => Ok(tenant.id),
    }
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
        parent_id: None,
        position: 0,
        topic_count: category.topic_count,
        reply_count: category.reply_count,
        moderated: false,
        is_subscribed: category.is_subscribed,
    }
}

fn map_category_response(category: CategoryResponse) -> GqlForumCategory {
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

fn map_topic_list_item(
    topic: TopicListItem,
    author_profile: Option<GqlProfileSummary>,
) -> GqlForumTopicListItem {
    GqlForumTopicListItem {
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
        metadata: topic.metadata,
        status: topic.status,
        channel_slugs: topic.channel_slugs,
        vote_score: topic.vote_score,
        current_user_vote: topic.current_user_vote,
        is_subscribed: topic.is_subscribed,
        solution_reply_id: topic.solution_reply_id,
        is_pinned: topic.is_pinned,
        is_locked: topic.is_locked,
        reply_count: topic.reply_count,
        created_at: topic.created_at,
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
        content_plain_text: reply.content_plain_text,
        status: reply.status,
        vote_score: reply.vote_score,
        current_user_vote: reply.current_user_vote,
        is_solution: reply.is_solution,
        parent_reply_id: reply.parent_reply_id,
        created_at: reply.created_at,
        updated_at: reply.updated_at,
    }
}

fn map_widget_catalog(catalog: ForumWidgetCatalogResponse) -> GqlForumWidgetCatalog {
    GqlForumWidgetCatalog {
        catalog_version: catalog.catalog_version,
        builder_contract_version: catalog.builder_contract_version,
        consumer_min_version: catalog.consumer_min_version,
        compatibility_matrix: catalog
            .compatibility_matrix
            .into_iter()
            .map(|entry| GqlForumWidgetCompatibilityEntry {
                provider_contract_version: entry.provider_contract_version,
                consumer_min_version: entry.consumer_min_version,
            })
            .collect(),
        items: catalog
            .items
            .into_iter()
            .map(|item| GqlForumWidgetCatalogItem {
                widget_type: item.widget_type,
                data_contract_version: item.data_contract_version,
                props_schema: item.props_schema,
                capability_requirements: GqlForumWidgetCapabilityRequirements {
                    preview: item.capability_requirements.preview,
                    publish: item.capability_requirements.publish,
                    moderation_view: item.capability_requirements.moderation_view,
                },
                fallback_mode: item.fallback_mode,
                error_mapping: GqlForumWidgetErrorMapping {
                    validation: item.error_mapping.validation,
                    sanitize: item.error_mapping.sanitize,
                    rbac: item.error_mapping.rbac,
                    runtime: item.error_mapping.runtime,
                },
            })
            .collect(),
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

    let profiles = ProfilePresentationService::new(db.clone())
        .find_profile_summaries(
            tenant_id,
            &user_ids,
            Some(requested_locale),
            Some(tenant_default_locale),
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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
                .extend_with(|_, extension| extension.set("code", "INTERNAL_SERVER_ERROR"))
        })?;
    if enabled {
        Ok(())
    } else {
        Err(
            async_graphql::Error::new("Forum module is not enabled for this channel")
                .extend_with(|_, extension| extension.set("code", "FORBIDDEN")),
        )
    }
}

fn is_public_request(ctx: &Context<'_>) -> bool {
    ctx.data_opt::<AuthContext>().is_none()
}

fn public_channel_slug(ctx: &Context<'_>) -> Option<String> {
    ctx.data_opt::<RequestContext>()
        .and_then(|request| request.channel_slug.clone())
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
