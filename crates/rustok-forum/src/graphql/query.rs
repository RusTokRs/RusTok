use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    graphql::{require_module_enabled, resolve_graphql_locale, GraphQLError, PaginationInput},
    has_any_effective_permission, AuthContext, TenantContext,
};
use rustok_core::{Permission, SecurityContext};
use rustok_outbox::TransactionalEventBus;
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::time::Instant;
use uuid::Uuid;

use crate::{
    CategoryListItem, CategoryService, ForumError, ReplyResponse, ReplyService, TopicListItem,
    TopicResponse, TopicService,
};

use super::types::*;

const MODULE_SLUG: &str = "forum";

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
            locale: Some(locale),
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

        let items = topics
            .into_iter()
            .map(map_topic_list_item)
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
            &[Permission::FORUM_REPLIES_READ],
            "Permission denied: forum_replies:read required",
        )?;

        let tenant = ctx.data::<TenantContext>()?;
        let service = ReplyService::new(db.clone(), event_bus.clone());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let filter = crate::ListRepliesFilter {
            locale: Some(locale),
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

        let items = replies
            .into_iter()
            .map(map_reply_response)
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

    async fn forum_storefront_categories(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        locale: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<ForumCategoryConnection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
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
            locale: Some(locale),
            page: (offset / limit + 1) as u64,
            per_page: limit as u64,
        };

        let list_started_at = Instant::now();
        let (topics, total) = service
            .list_with_locale_fallback(
                resolved_tenant_id,
                forum_security_or_system(ctx),
                filter,
                Some(tenant.default_locale.as_str()),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.storefront_topics",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );

        let items = topics
            .into_iter()
            .map(map_topic_list_item)
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
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let resolved_tenant_id = tenant_id.unwrap_or(tenant.id);
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let service = TopicService::new(db.clone(), event_bus.clone());

        let topic = match service
            .get_with_locale_fallback(
                resolved_tenant_id,
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

        Ok(Some(map_topic_response(topic)))
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
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let resolved_tenant_id = tenant_id.unwrap_or(tenant.id);
        let service = ReplyService::new(db.clone(), event_bus.clone());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let filter = crate::ListRepliesFilter {
            locale: Some(locale),
            page: (offset / limit + 1) as u64,
            per_page: limit as u64,
        };

        let list_started_at = Instant::now();
        let (replies, total) = service
            .list_response_for_topic_with_locale_fallback(
                resolved_tenant_id,
                forum_security_or_system(ctx),
                topic_id,
                filter,
                Some(tenant.default_locale.as_str()),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.storefront_replies",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );

        let items = replies
            .into_iter()
            .map(map_reply_response)
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
        requested_locale: category.locale.clone(),
        locale: category.locale,
        effective_locale: category.effective_locale,
        name: category.name,
        slug: category.slug,
        description: category.description,
        icon: category.icon,
        color: category.color,
        topic_count: category.topic_count,
        reply_count: category.reply_count,
    }
}

fn map_topic_list_item(topic: TopicListItem) -> GqlForumTopic {
    GqlForumTopic {
        id: topic.id,
        requested_locale: topic.locale.clone(),
        locale: topic.locale,
        effective_locale: topic.effective_locale.clone(),
        available_locales: vec![topic.effective_locale],
        category_id: topic.category_id,
        author_id: topic.author_id,
        title: topic.title,
        slug: topic.slug,
        body: String::new(),
        body_format: "markdown".to_string(),
        status: topic.status,
        tags: Vec::new(),
        is_pinned: topic.is_pinned,
        is_locked: topic.is_locked,
        reply_count: topic.reply_count,
        created_at: topic.created_at,
        updated_at: String::new(),
    }
}

fn map_topic_response(topic: TopicResponse) -> GqlForumTopic {
    GqlForumTopic {
        id: topic.id,
        requested_locale: topic.requested_locale,
        locale: topic.locale,
        effective_locale: topic.effective_locale,
        available_locales: topic.available_locales,
        category_id: topic.category_id,
        author_id: topic.author_id,
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
    }
}

fn map_reply_response(reply: ReplyResponse) -> GqlForumReply {
    GqlForumReply {
        id: reply.id,
        requested_locale: reply.requested_locale,
        locale: reply.locale,
        effective_locale: reply.effective_locale,
        topic_id: reply.topic_id,
        author_id: reply.author_id,
        content: reply.content,
        content_format: reply.content_format,
        status: reply.status,
        parent_reply_id: reply.parent_reply_id,
        created_at: reply.created_at,
        updated_at: reply.updated_at,
    }
}
