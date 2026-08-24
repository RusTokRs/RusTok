use async_graphql::{Context, Object, Result};
use rustok_api::{
    AuthContext, Permission, RequestContext, TenantContext,
    graphql::{PaginationInput, require_module_enabled, resolve_graphql_locale},
};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::time::Instant;
use uuid::Uuid;

use crate::{
    ForumReplyReadOperation, ForumReplyReadTransport, ReplyResponse, ReplyStatus, RevisionService,
    reply_read_audience_port_context,
};

use super::{ForumGraphqlRuntimeData, ForumReplyConnection, GqlForumReply};

const MODULE_SLUG: &str = "forum";
const PUBLIC_REPLY_STATUSES: [ReplyStatus; 1] = [ReplyStatus::Approved];

#[derive(Default)]
pub struct ForumReplyAudienceQuery;

#[Object]
impl ForumReplyAudienceQuery {
    /// Exact authenticated owner reply list. Parent-topic category and richer
    /// audience layers are resolved before reply content is returned.
    async fn forum_audience_replies(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        topic_id: Uuid,
        locale: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<ForumReplyConnection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = super::require_forum_permission(
            ctx,
            &[Permission::FORUM_REPLIES_LIST],
            "Permission denied: forum_replies:list required",
        )?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = super::resolve_tenant_scope(tenant, tenant_id)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let context = reply_read_audience_port_context(
            ForumReplyReadTransport::Graphql,
            ForumReplyReadOperation::ReplyList,
            tenant_id,
            auth,
            ctx.data_opt::<RequestContext>(),
            locale.as_str(),
        )?;
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();
        let service = runtime.reply_audience_read_service(db.clone(), event_bus.clone());

        let started_at = Instant::now();
        let (replies, total) = service
            .list_response_authenticated_owner_visible_with_audience_context(
                tenant_id,
                forum_security(&auth),
                context,
                topic_id,
                crate::ListRepliesFilter {
                    locale: Some(locale),
                    page: (offset / limit + 1) as u64,
                    per_page: limit as u64,
                },
                Some(tenant.default_locale.as_str()),
                None,
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "forum.audience_replies",
            "exact_reply_audience_owner",
            started_at.elapsed().as_secs_f64(),
            total,
        );

        let items = replies
            .into_iter()
            .map(map_reply_response)
            .collect::<Vec<_>>();
        metrics::record_read_path_budget(
            "graphql",
            "forum.audience_replies",
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

    /// Exact storefront reply list through parent-topic storefront and richer
    /// audience visibility. Missing and denied topics return an empty connection.
    async fn forum_storefront_audience_replies(
        &self,
        ctx: &Context<'_>,
        topic_id: Uuid,
        tenant_id: Option<Uuid>,
        locale: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<ForumReplyConnection> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_public_forum_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = super::resolve_tenant_scope(tenant, tenant_id)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let requested_limit = pagination.requested_limit();
        let (offset, limit) = pagination.normalize()?;
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();
        let service = runtime.reply_audience_read_service(db.clone(), event_bus.clone());
        let filter = crate::ListRepliesFilter {
            locale: Some(locale.clone()),
            page: (offset / limit + 1) as u64,
            per_page: limit as u64,
        };

        let started_at = Instant::now();
        let (replies, total) = if let Some(auth) = ctx.data_opt::<AuthContext>() {
            let context = reply_read_audience_port_context(
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
                    forum_security(auth),
                    context,
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
            "forum.storefront_audience_replies",
            "exact_reply_audience_owner",
            started_at.elapsed().as_secs_f64(),
            total,
        );

        let items = replies
            .into_iter()
            .map(map_reply_response)
            .collect::<Vec<_>>();
        metrics::record_read_path_budget(
            "graphql",
            "forum.storefront_audience_replies",
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

    /// Exact current Forum-owned reply revision for an already-visible,
    /// approved storefront target. This remains a generic Forum owner fact and
    /// does not construct any optional-consumer subject or command.
    async fn forum_storefront_reply_current_revision(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
        locale: Option<String>,
    ) -> Result<Option<String>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        super::require_public_forum_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = super::resolve_tenant_scope(tenant, tenant_id)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();
        let service = runtime.reply_audience_read_service(db.clone(), event_bus.clone());

        let reply = if let Some(auth) = ctx.data_opt::<AuthContext>() {
            let context = reply_read_audience_port_context(
                ForumReplyReadTransport::Graphql,
                ForumReplyReadOperation::SelectedReply,
                tenant_id,
                auth,
                ctx.data_opt::<RequestContext>(),
                locale.as_str(),
            )?;
            service
                .get_authenticated_storefront_visible_with_audience_context(
                    tenant_id,
                    forum_security(auth),
                    context,
                    id,
                    Some(tenant.default_locale.as_str()),
                    Some(&PUBLIC_REPLY_STATUSES),
                )
                .await?
        } else {
            service
                .get_public_storefront_visible_with_locale_fallback(
                    tenant_id,
                    id,
                    locale.as_str(),
                    Some(tenant.default_locale.as_str()),
                    public_channel_slug(ctx).as_deref(),
                    Some(&PUBLIC_REPLY_STATUSES),
                )
                .await?
        };
        let Some(reply) = reply else {
            return Ok(None);
        };

        let revision = RevisionService::new(db.clone())
            .current_reply_revision(tenant_id, reply.id)
            .await?;
        Ok(Some(revision.to_string()))
    }
}

fn forum_security(auth: &AuthContext) -> SecurityContext {
    SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)
}

fn public_channel_slug(ctx: &Context<'_>) -> Option<String> {
    ctx.data_opt::<RequestContext>()
        .and_then(|request| request.channel_slug.clone())
}

fn map_reply_response(reply: ReplyResponse) -> GqlForumReply {
    GqlForumReply {
        id: reply.id,
        requested_locale: reply.requested_locale,
        locale: reply.locale,
        effective_locale: reply.effective_locale,
        topic_id: reply.topic_id,
        author_id: reply.author_id,
        author_profile: None,
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
