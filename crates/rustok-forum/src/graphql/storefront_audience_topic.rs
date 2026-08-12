use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result};
use rustok_api::{
    AuthContext, RequestContext, TenantContext,
    graphql::{GraphQLError, require_module_enabled, resolve_graphql_locale},
};
use rustok_channel::ChannelService;
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ForumTopicReadOperation, ForumTopicReadTransport, RevisionService, TopicResponse,
    topic_read_audience_port_context,
};

use super::{ForumGraphqlRuntimeData, GqlForumTopic};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub struct ForumStorefrontAudienceTopicQuery;

#[Object]
impl ForumStorefrontAudienceTopicQuery {
    /// Exact module-owned storefront topic read through the canonical richer
    /// category/topic audience owner. Missing and denied targets are both null.
    async fn forum_storefront_audience_topic(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
        locale: Option<String>,
    ) -> Result<Option<GqlForumTopic>> {
        let (_, topic) =
            load_storefront_audience_topic(ctx, id, tenant_id, locale.as_deref()).await?;
        Ok(topic.map(map_topic_response))
    }

    /// Exact current Forum-owned topic revision for an already-visible
    /// storefront target. This is a generic Forum owner fact: it does not
    /// construct reaction subjects or depend on any optional consumer module.
    async fn forum_storefront_topic_current_revision(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
        locale: Option<String>,
    ) -> Result<Option<String>> {
        let (tenant_id, topic) =
            load_storefront_audience_topic(ctx, id, tenant_id, locale.as_deref()).await?;
        let Some(topic) = topic else {
            return Ok(None);
        };

        let db = ctx.data::<DatabaseConnection>()?;
        let revision = RevisionService::new(db.clone())
            .current_topic_revision(tenant_id, topic.id)
            .await?;
        Ok(Some(revision.to_string()))
    }
}

async fn load_storefront_audience_topic(
    ctx: &Context<'_>,
    id: Uuid,
    requested_tenant_id: Option<Uuid>,
    requested_locale: Option<&str>,
) -> Result<(Uuid, Option<TopicResponse>)> {
    require_module_enabled(ctx, MODULE_SLUG).await?;
    require_public_forum_channel_enabled(ctx).await?;

    let db = ctx.data::<DatabaseConnection>()?;
    let event_bus = ctx.data::<TransactionalEventBus>()?;
    let tenant = ctx.data::<TenantContext>()?;
    let tenant_id = resolve_tenant_scope(tenant, requested_tenant_id)?;
    let locale = resolve_graphql_locale(ctx, requested_locale);
    let runtime = ctx
        .data_opt::<ForumGraphqlRuntimeData>()
        .cloned()
        .unwrap_or_default();
    let service = runtime.topic_audience_read_service(db.clone(), event_bus.clone());

    let topic = if let Some(auth) = ctx.data_opt::<AuthContext>() {
        let security =
            SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
        let audience_context = topic_read_audience_port_context(
            ForumTopicReadTransport::Graphql,
            ForumTopicReadOperation::SelectedTopic,
            tenant_id,
            auth,
            ctx.data_opt::<RequestContext>(),
            locale.as_str(),
        )?;
        service
            .get_authenticated_storefront_visible_with_audience_context(
                tenant_id,
                security,
                audience_context,
                id,
                Some(tenant.default_locale.as_str()),
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
            )
            .await?
    };

    Ok((tenant_id, topic))
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

    let Some(request_context) = ctx.data_opt::<RequestContext>() else {
        return Ok(());
    };
    let Some(channel_id) = request_context.channel_id else {
        return Ok(());
    };

    let db = ctx.data::<DatabaseConnection>()?;
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

fn public_channel_slug(ctx: &Context<'_>) -> Option<String> {
    ctx.data_opt::<RequestContext>()
        .and_then(|request| request.channel_slug.clone())
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
        author_profile: None,
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
