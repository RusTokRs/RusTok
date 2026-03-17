use async_graphql::{Context, FieldError, Object, Result};
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::time::Instant;
use uuid::Uuid;

use crate::context::AuthContext;
use crate::context::TenantContext;
use crate::graphql::common::require_module_enabled;
use crate::graphql::common::resolve_graphql_locale;
use crate::graphql::errors::GraphQLError;
use crate::graphql::schema::module_slug;
use crate::services::rbac_service::RbacService;
use rustok_core::Permission;
use rustok_forum::{CategoryService, ReplyService, TopicService};
use rustok_outbox::TransactionalEventBus;

use super::types::*;
use crate::graphql::common::PaginationInput;

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
        require_module_enabled(ctx, module_slug::FORUM).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[
                Permission::FORUM_CATEGORIES_LIST,
                Permission::FORUM_CATEGORIES_MANAGE,
            ],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: forum_categories:list required",
            ));
        }

        let security = auth.security_context();
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
                security,
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
            .map(|c| GqlForumCategory {
                id: c.id,
                requested_locale: c.locale.clone(),
                locale: c.locale,
                effective_locale: c.effective_locale,
                name: c.name,
                slug: c.slug,
                description: c.description,
                icon: c.icon,
                color: c.color,
                topic_count: c.topic_count,
                reply_count: c.reply_count,
            })
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
        require_module_enabled(ctx, module_slug::FORUM).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[
                Permission::FORUM_TOPICS_LIST,
                Permission::FORUM_TOPICS_MANAGE,
            ],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: forum_topics:list required",
            ));
        }

        let security = auth.security_context();
        let tenant = ctx.data::<TenantContext>()?;
        let service = TopicService::new(db.clone(), event_bus.clone());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let filter = rustok_forum::ListTopicsFilter {
            category_id,
            status: None,
            locale: Some(resolve_graphql_locale(ctx, locale.as_deref())),
            page: (offset / limit + 1) as u64,
            per_page: limit as u64,
        };

        let list_started_at = Instant::now();
        let (topics, total) = service
            .list_with_locale_fallback(
                tenant_id,
                security,
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
            .map(|t| GqlForumTopic {
                id: t.id,
                requested_locale: t.locale.clone(),
                locale: t.locale,
                effective_locale: t.effective_locale,
                category_id: t.category_id,
                author_id: t.author_id,
                title: t.title,
                slug: t.slug,
                body: String::new(),
                status: t.status,
                tags: Vec::new(),
                is_pinned: t.is_pinned,
                is_locked: t.is_locked,
                reply_count: t.reply_count,
                created_at: t.created_at,
                updated_at: String::new(),
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
        require_module_enabled(ctx, module_slug::FORUM).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[
                Permission::FORUM_REPLIES_READ,
                Permission::FORUM_REPLIES_MANAGE,
            ],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: forum_replies:read required",
            ));
        }

        let security = auth.security_context();
        let tenant = ctx.data::<TenantContext>()?;
        let service = ReplyService::new(db.clone(), event_bus.clone());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let filter = rustok_forum::ListRepliesFilter {
            locale: Some(resolve_graphql_locale(ctx, locale.as_deref())),
            page: (offset / limit + 1) as u64,
            per_page: limit as u64,
        };

        let list_started_at = Instant::now();
        let (replies, total) = service
            .list_for_topic_with_locale_fallback(
                tenant_id,
                security,
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
            .map(|r| GqlForumReply {
                id: r.id,
                requested_locale: r.locale.clone(),
                locale: r.locale,
                effective_locale: r.effective_locale,
                topic_id: r.topic_id,
                author_id: r.author_id,
                content: r.content_preview,
                status: r.status,
                parent_reply_id: r.parent_reply_id,
                created_at: r.created_at,
                updated_at: String::new(),
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
}
