use std::collections::HashMap;

use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, RequestContext, TenantContext,
    graphql::{GraphQLError, PaginationInput, require_module_enabled, resolve_graphql_locale},
    has_any_effective_permission,
};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ForumStorefrontReadStateService, ForumTopicReadState, ForumTopicUnreadSummary,
    ListTopicsFilter, TopicListItem, TopicService,
};

const MODULE_SLUG: &str = "forum";

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumStorefrontUnreadTopic {
    pub id: Uuid,
    pub effective_locale: String,
    pub category_id: Uuid,
    pub title: String,
    pub slug: String,
    pub status: String,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub created_at: String,
    pub read_state_explicit: bool,
    pub last_read_position: i64,
    pub last_read_revision: i64,
    pub unread_count: i64,
    pub has_unread_topic_revision: bool,
    pub is_unread: bool,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumStorefrontUnreadTopicPage {
    pub items: Vec<GqlForumStorefrontUnreadTopic>,
    pub total: i64,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumStorefrontTopicReadState {
    pub topic_id: Uuid,
    pub last_read_position: i64,
    pub last_read_revision: i64,
    pub explicit: bool,
    pub updated_at: Option<String>,
}

#[derive(Default)]
pub struct ForumStorefrontReadStateQuery;

#[Object]
impl ForumStorefrontReadStateQuery {
    async fn forum_storefront_unread_topics(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        category_id: Option<Uuid>,
        locale: Option<String>,
        #[graphql(default)] pagination: PaginationInput,
    ) -> Result<GqlForumStorefrontUnreadTopicPage> {
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
        let request = ctx.data::<RequestContext>()?;
        let (offset, limit) = pagination.normalize()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let security = forum_security(&auth);

        let (topics, total) = TopicService::new(db.clone(), event_bus.clone())
            .list_storefront_visible_with_locale_fallback(
                tenant_id,
                security.clone(),
                ListTopicsFilter {
                    category_id,
                    status: None,
                    locale: Some(locale),
                    page: (offset / limit + 1) as u64,
                    per_page: limit as u64,
                },
                Some(tenant.default_locale.as_str()),
                request.channel_slug.as_deref(),
            )
            .await?;
        let summaries = ForumStorefrontReadStateService::new(db.clone())
            .summarize_topics(
                tenant_id,
                security,
                topics.iter().map(|topic| topic.id).collect(),
            )
            .await?
            .into_iter()
            .map(|summary| (summary.topic_id, summary))
            .collect::<HashMap<_, _>>();
        let items = topics
            .into_iter()
            .map(|topic| {
                let summary = summaries.get(&topic.id).ok_or_else(|| {
                    <FieldError as GraphQLError>::internal_error(
                        "Forum storefront unread summary is unavailable",
                    )
                })?;
                Ok(map_topic(topic, summary))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(GqlForumStorefrontUnreadTopicPage {
            items,
            total: total as i64,
        })
    }
}

#[derive(Default)]
pub struct ForumStorefrontReadStateMutation;

#[Object]
impl ForumStorefrontReadStateMutation {
    async fn mark_forum_storefront_topic_read(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        topic_id: Uuid,
        locale: Option<String>,
    ) -> Result<GqlForumStorefrontTopicReadState> {
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
        let request = ctx.data::<RequestContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let security = forum_security(&auth);

        let visible = TopicService::new(db.clone(), event_bus.clone())
            .get_storefront_visible_with_locale_fallback(
                tenant_id,
                security.clone(),
                topic_id,
                locale.as_str(),
                Some(tenant.default_locale.as_str()),
                request.channel_slug.as_deref(),
            )
            .await?;
        if visible.is_none() {
            return Err(<FieldError as GraphQLError>::not_found(
                "Forum topic is unavailable",
            ));
        }

        let state = ForumStorefrontReadStateService::new(db.clone())
            .mark_topic_read_current(tenant_id, topic_id, security)
            .await?;
        Ok(map_read_state(state))
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

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)
}

fn map_topic(
    topic: TopicListItem,
    summary: &ForumTopicUnreadSummary,
) -> GqlForumStorefrontUnreadTopic {
    GqlForumStorefrontUnreadTopic {
        id: topic.id,
        effective_locale: topic.effective_locale,
        category_id: topic.category_id,
        title: topic.title,
        slug: topic.slug,
        status: topic.status,
        is_pinned: topic.is_pinned,
        is_locked: topic.is_locked,
        reply_count: topic.reply_count,
        created_at: topic.created_at,
        read_state_explicit: summary.read_state_explicit,
        last_read_position: summary.last_read_position,
        last_read_revision: summary.last_read_revision,
        unread_count: summary.unread_count,
        has_unread_topic_revision: summary.has_unread_topic_revision,
        is_unread: summary.is_unread,
    }
}

fn map_read_state(state: ForumTopicReadState) -> GqlForumStorefrontTopicReadState {
    GqlForumStorefrontTopicReadState {
        topic_id: state.topic_id,
        last_read_position: state.last_read_position,
        last_read_revision: state.last_read_revision,
        explicit: state.explicit,
        updated_at: state.updated_at,
    }
}
