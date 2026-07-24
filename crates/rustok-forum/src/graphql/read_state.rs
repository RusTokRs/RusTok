use async_graphql::{Context, Enum, FieldError, InputObject, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled, resolve_graphql_locale},
    has_any_effective_permission,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    ForumReadModelService, ForumTopicReadState, ForumTopicReadStateService,
    MarkForumTopicReadInput, MarkForumTopicsReadBatchInput, MarkForumTopicsReadBatchResult,
    TopicReadModel, TopicStatus, TopicUnreadCursorQuery, TopicUnreadReadModel,
};

const MODULE_SLUG: &str = "forum";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GqlForumTopicStatus {
    Open,
    Closed,
    Archived,
}

impl From<GqlForumTopicStatus> for TopicStatus {
    fn from(value: GqlForumTopicStatus) -> Self {
        match value {
            GqlForumTopicStatus::Open => Self::Open,
            GqlForumTopicStatus::Closed => Self::Closed,
            GqlForumTopicStatus::Archived => Self::Archived,
        }
    }
}

#[derive(Clone, Debug, InputObject)]
pub struct MarkForumTopicReadGraphqlInput {
    pub last_read_position: i64,
    pub last_read_revision: i64,
}

#[derive(Clone, Debug, Default, InputObject)]
pub struct MarkForumTopicsReadBatchGraphqlInput {
    pub cursor: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumTopicReadModel {
    pub id: Uuid,
    pub category_id: Uuid,
    pub author_id: Option<Uuid>,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub title: String,
    pub slug: String,
    pub metadata: Value,
    pub status: String,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub vote_score: i32,
    pub current_user_vote: Option<i32>,
    pub is_subscribed: bool,
    pub solution_reply_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumTopicUnreadItem {
    pub topic: GqlForumTopicReadModel,
    pub read_state_explicit: bool,
    pub last_read_position: i64,
    pub last_read_revision: i64,
    pub unread_count: i64,
    pub has_unread_topic_revision: bool,
    pub is_unread: bool,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumTopicUnreadPage {
    pub items: Vec<GqlForumTopicUnreadItem>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumTopicReadState {
    pub tenant_id: Uuid,
    pub topic_id: Uuid,
    pub user_id: Option<Uuid>,
    pub last_read_position: i64,
    pub last_read_revision: i64,
    pub explicit: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumTopicsReadBatchResult {
    pub processed: i64,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub snapshot_at: String,
}

#[derive(Default)]
pub struct ForumReadStateQuery;

#[Object]
impl ForumReadStateQuery {
    async fn forum_unread_topics(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        cursor: Option<String>,
        limit: Option<i32>,
        category_id: Option<Uuid>,
        status: Option<GqlForumTopicStatus>,
        locale: Option<String>,
        fallback_locale: Option<String>,
        #[graphql(default)] unread_only: bool,
    ) -> Result<GqlForumTopicUnreadPage> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_LIST],
            "Permission denied: forum_topics:list required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let limit = graphql_limit(limit)?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let fallback_locale = fallback_locale.or_else(|| Some(tenant.default_locale.clone()));

        let page = ForumReadModelService::new(db.clone())
            .list_topics_with_unread(
                tenant_id,
                forum_security(&auth),
                TopicUnreadCursorQuery {
                    cursor,
                    limit,
                    category_id,
                    status: status.map(Into::into),
                    locale: Some(locale),
                    fallback_locale,
                    unread_only,
                },
            )
            .await?;

        Ok(GqlForumTopicUnreadPage {
            items: page.items.into_iter().map(map_unread_item).collect(),
            next_cursor: page.next_cursor,
            has_more: page.has_more,
        })
    }

    async fn forum_topic_read_state(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        topic_id: Uuid,
    ) -> Result<GqlForumTopicReadState> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_READ],
            "Permission denied: forum_topics:read required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let state = ForumTopicReadStateService::new(db.clone())
            .get_topic_read_state(tenant_id, topic_id, forum_security(&auth))
            .await?;
        Ok(map_read_state(state))
    }
}

#[derive(Default)]
pub struct ForumReadStateMutation;

#[Object]
impl ForumReadStateMutation {
    async fn mark_forum_topic_read(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        topic_id: Uuid,
        input: MarkForumTopicReadGraphqlInput,
    ) -> Result<GqlForumTopicReadState> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_READ],
            "Permission denied: forum_topics:read required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let state = ForumTopicReadStateService::new(db.clone())
            .mark_topic_read(
                tenant_id,
                topic_id,
                forum_security(&auth),
                MarkForumTopicReadInput {
                    last_read_position: input.last_read_position,
                    last_read_revision: input.last_read_revision,
                },
            )
            .await?;
        Ok(map_read_state(state))
    }

    async fn mark_forum_category_read(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        category_id: Uuid,
        #[graphql(default)] input: MarkForumTopicsReadBatchGraphqlInput,
    ) -> Result<GqlForumTopicsReadBatchResult> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_READ],
            "Permission denied: forum_topics:read required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let result = ForumTopicReadStateService::new(db.clone())
            .mark_category_read(
                tenant_id,
                category_id,
                forum_security(&auth),
                batch_input(input)?,
            )
            .await?;
        Ok(map_batch_result(result))
    }

    async fn mark_all_forum_topics_read(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        #[graphql(default)] input: MarkForumTopicsReadBatchGraphqlInput,
    ) -> Result<GqlForumTopicsReadBatchResult> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_READ],
            "Permission denied: forum_topics:read required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;

        let result = ForumTopicReadStateService::new(db.clone())
            .mark_all_read(tenant_id, forum_security(&auth), batch_input(input)?)
            .await?;
        Ok(map_batch_result(result))
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

fn graphql_limit(limit: Option<i32>) -> Result<Option<u64>> {
    limit
        .map(|value| {
            u64::try_from(value).map_err(|_| {
                <FieldError as GraphQLError>::bad_user_input(
                    "Forum cursor limit must be nonnegative",
                )
            })
        })
        .transpose()
}

fn batch_input(input: MarkForumTopicsReadBatchGraphqlInput) -> Result<MarkForumTopicsReadBatchInput> {
    Ok(MarkForumTopicsReadBatchInput {
        cursor: input.cursor,
        limit: graphql_limit(input.limit)?,
    })
}

fn map_topic(topic: TopicReadModel) -> GqlForumTopicReadModel {
    GqlForumTopicReadModel {
        id: topic.id,
        category_id: topic.category_id,
        author_id: topic.author_id,
        requested_locale: topic.requested_locale,
        effective_locale: topic.effective_locale,
        available_locales: topic.available_locales,
        title: topic.title,
        slug: topic.slug,
        metadata: topic.metadata,
        status: topic.status,
        is_pinned: topic.is_pinned,
        is_locked: topic.is_locked,
        reply_count: topic.reply_count,
        vote_score: topic.vote_score,
        current_user_vote: topic.current_user_vote,
        is_subscribed: topic.is_subscribed,
        solution_reply_id: topic.solution_reply_id,
        created_at: topic.created_at,
        updated_at: topic.updated_at,
    }
}

fn map_unread_item(item: TopicUnreadReadModel) -> GqlForumTopicUnreadItem {
    GqlForumTopicUnreadItem {
        topic: map_topic(item.topic),
        read_state_explicit: item.read_state_explicit,
        last_read_position: item.last_read_position,
        last_read_revision: item.last_read_revision,
        unread_count: item.unread_count,
        has_unread_topic_revision: item.has_unread_topic_revision,
        is_unread: item.is_unread,
    }
}

fn map_read_state(state: ForumTopicReadState) -> GqlForumTopicReadState {
    GqlForumTopicReadState {
        tenant_id: state.tenant_id,
        topic_id: state.topic_id,
        user_id: state.user_id,
        last_read_position: state.last_read_position,
        last_read_revision: state.last_read_revision,
        explicit: state.explicit,
        created_at: state.created_at,
        updated_at: state.updated_at,
    }
}

fn map_batch_result(result: MarkForumTopicsReadBatchResult) -> GqlForumTopicsReadBatchResult {
    GqlForumTopicsReadBatchResult {
        processed: result.processed as i64,
        next_cursor: result.next_cursor,
        has_more: result.has_more,
        snapshot_at: result.snapshot_at,
    }
}
