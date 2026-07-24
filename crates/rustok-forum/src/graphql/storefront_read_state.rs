use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, RequestContext, TenantContext,
    graphql::{GraphQLError, require_module_enabled, resolve_graphql_locale},
    has_any_effective_permission,
};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ForumError, ForumStorefrontReadStateService, ForumStorefrontUnreadTopic,
    ForumTopicReadState, ListTopicsFilter,
};

const MODULE_SLUG: &str = "forum";
const DEFAULT_STOREFRONT_UNREAD_LIMIT: u64 = 20;
const MAX_STOREFRONT_UNREAD_LIMIT: u64 = 100;

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
        limit: Option<i32>,
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
        let limit = storefront_limit(limit)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let page = ForumStorefrontReadStateService::new(db.clone(), event_bus.clone())
            .list_topics_with_unread(
                tenant_id,
                forum_security(&auth),
                ListTopicsFilter {
                    category_id,
                    status: None,
                    locale: Some(locale),
                    page: 1,
                    per_page: limit,
                },
                Some(tenant.default_locale.as_str()),
                request.channel_slug.as_deref(),
            )
            .await?;

        Ok(GqlForumStorefrontUnreadTopicPage {
            items: page.items.into_iter().map(map_topic).collect(),
            total: page.total as i64,
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

        let state = match ForumStorefrontReadStateService::new(db.clone(), event_bus.clone())
            .mark_topic_read_current_visible(
                tenant_id,
                topic_id,
                forum_security(&auth),
                locale.as_str(),
                Some(tenant.default_locale.as_str()),
                request.channel_slug.as_deref(),
            )
            .await
        {
            Ok(state) => state,
            Err(ForumError::TopicNotFound(_)) => {
                return Err(<FieldError as GraphQLError>::not_found(
                    "Forum topic is unavailable",
                ));
            }
            Err(error) => return Err(error.into()),
        };
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

fn storefront_limit(limit: Option<i32>) -> Result<u64> {
    let limit = limit.unwrap_or(DEFAULT_STOREFRONT_UNREAD_LIMIT as i32);
    let limit = u64::try_from(limit).map_err(|_| {
        <FieldError as GraphQLError>::bad_user_input(
            "Forum storefront unread limit must be positive",
        )
    })?;
    if !(1..=MAX_STOREFRONT_UNREAD_LIMIT).contains(&limit) {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "Forum storefront unread limit must be between 1 and 100",
        ));
    }
    Ok(limit)
}

fn map_topic(item: ForumStorefrontUnreadTopic) -> GqlForumStorefrontUnreadTopic {
    GqlForumStorefrontUnreadTopic {
        id: item.topic.id,
        effective_locale: item.topic.effective_locale,
        category_id: item.topic.category_id,
        title: item.topic.title,
        slug: item.topic.slug,
        status: item.topic.status,
        is_pinned: item.topic.is_pinned,
        is_locked: item.topic.is_locked,
        reply_count: item.topic.reply_count,
        created_at: item.topic.created_at,
        read_state_explicit: item.read_state_explicit,
        last_read_position: item.last_read_position,
        last_read_revision: item.last_read_revision,
        unread_count: item.unread_count,
        has_unread_topic_revision: item.has_unread_topic_revision,
        is_unread: item.is_unread,
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
