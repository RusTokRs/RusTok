use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result};
use rustok_api::{
    AuthContext, RequestContext, TenantContext,
    graphql::{GraphQLError, PaginationInput, require_module_enabled, resolve_graphql_locale},
};
use rustok_channel::ChannelService;
use rustok_outbox::TransactionalEventBus;
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::time::Instant;
use uuid::Uuid;

use crate::{ForumTopicAudienceListService, TopicListItem};

use super::types::*;

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub struct ForumStorefrontAudienceTopicsQuery;

#[Object]
impl ForumStorefrontAudienceTopicsQuery {
    /// Public storefront topic pagination through the exact richer-audience owner.
    ///
    /// The response page and total are derived from the same allowed sequence.
    /// Authenticated callers may use this field as the public fallback surface;
    /// user-specific unread composition remains on `forumStorefrontUnreadTopics`.
    async fn forum_storefront_audience_topics(
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
        let request = ctx.data_opt::<RequestContext>();
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let list_started_at = Instant::now();
        let page = ForumTopicAudienceListService::new(db.clone(), event_bus.clone())
            .list_public_storefront_visible_with_locale_fallback(
                tenant_id,
                crate::ListTopicsFilter {
                    category_id,
                    status: None,
                    locale: Some(locale),
                    page: (offset / limit + 1) as u64,
                    per_page: limit as u64,
                },
                Some(tenant.default_locale.as_str()),
                request.and_then(|request| request.channel_slug.as_deref()),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.storefront_audience_topics",
            "exact_audience_owner",
            list_started_at.elapsed().as_secs_f64(),
            page.total,
        );

        let items = page
            .items
            .into_iter()
            .map(map_topic_list_item)
            .collect::<Vec<_>>();
        metrics::record_read_path_budget(
            "graphql",
            "forum.storefront_audience_topics",
            Some(requested_limit),
            limit as u64,
            items.len(),
        );

        Ok(ForumTopicConnection::new(
            items,
            page.total as i64,
            offset,
            limit,
        ))
    }
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

async fn require_public_forum_channel_enabled(ctx: &Context<'_>) -> Result<()> {
    if ctx.data_opt::<AuthContext>().is_some() {
        return Ok(());
    }

    let Some(request) = ctx.data_opt::<RequestContext>() else {
        return Ok(());
    };
    let Some(channel_id) = request.channel_id else {
        return Ok(());
    };

    let db = ctx.data::<DatabaseConnection>()?;
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

fn map_topic_list_item(topic: TopicListItem) -> GqlForumTopicListItem {
    GqlForumTopicListItem {
        id: topic.id,
        requested_locale: topic.requested_locale,
        locale: topic.locale,
        effective_locale: topic.effective_locale,
        available_locales: topic.available_locales,
        category_id: topic.category_id,
        author_id: topic.author_id,
        author_profile: None,
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
