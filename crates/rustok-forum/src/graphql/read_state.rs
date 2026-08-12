use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

use async_graphql::{Context, Enum, FieldError, InputObject, Object, Result, SimpleObject};
use prometheus::{IntCounterVec, Opts};
use rustok_api::{
    AuthContext, Permission, RequestContext, TenantContext,
    graphql::{GraphQLError, require_module_enabled, resolve_graphql_locale},
    has_any_effective_permission,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    ForumReadModelService, ForumTopicReadOperation, ForumTopicReadState,
    ForumTopicReadStateService, ForumTopicReadTransport, MarkForumTopicReadInput,
    MarkForumTopicsReadBatchInput, MarkForumTopicsReadBatchResult, TopicReadModel, TopicStatus,
    TopicUnreadCursorQuery, TopicUnreadReadModel, topic_read_audience_port_context,
};

use super::ForumGraphqlRuntimeData;

const MODULE_SLUG: &str = "forum";
const LOCALE_RESOURCE_UNREAD_TOPIC: &str = "unread_topic";
const LOCALE_OUTCOME_EXACT: &str = "exact";
const LOCALE_OUTCOME_FALLBACK: &str = "fallback";
const LOCALE_OUTCOME_MISSING: &str = "missing";
const UNREAD_TOPIC_STATE_IMPLICIT: &str = "implicit";
const UNREAD_TOPIC_STATE_REPLY: &str = "reply";
const UNREAD_TOPIC_STATE_REVISION: &str = "revision";
const UNREAD_TOPIC_STATE_REPLY_AND_REVISION: &str = "reply_and_revision";
const UNREAD_TOPIC_STATE_READ: &str = "read";

static FORUM_GRAPHQL_LOCALE_RESOLUTION_TOTAL: OnceLock<IntCounterVec> = OnceLock::new();
static FORUM_GRAPHQL_LOCALE_RESOLUTION_REGISTERED: AtomicBool = AtomicBool::new(false);
static FORUM_GRAPHQL_UNREAD_TOPIC_STATE_TOTAL: OnceLock<IntCounterVec> = OnceLock::new();
static FORUM_GRAPHQL_UNREAD_TOPIC_STATE_REGISTERED: AtomicBool = AtomicBool::new(false);

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
        observe_unread_topic_locale_resolution(&page.items);
        observe_unread_topic_activity(&page.items);

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
        input: Option<MarkForumTopicsReadBatchGraphqlInput>,
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
        let locale = resolve_graphql_locale(ctx, None);
        let audience_context = topic_read_audience_port_context(
            ForumTopicReadTransport::Graphql,
            ForumTopicReadOperation::MarkCategoryRead,
            tenant_id,
            &auth,
            ctx.data_opt::<RequestContext>(),
            locale.as_str(),
        )?;
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();

        let result = runtime
            .visibility_scoped_read_state_service(db.clone())
            .mark_category_read_with_audience_context(
                tenant_id,
                category_id,
                forum_security(&auth),
                audience_context,
                batch_input(input.unwrap_or_default())?,
            )
            .await?;
        Ok(map_batch_result(result))
    }

    async fn mark_all_forum_topics_read(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        input: Option<MarkForumTopicsReadBatchGraphqlInput>,
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
        let locale = resolve_graphql_locale(ctx, None);
        let audience_context = topic_read_audience_port_context(
            ForumTopicReadTransport::Graphql,
            ForumTopicReadOperation::MarkAllRead,
            tenant_id,
            &auth,
            ctx.data_opt::<RequestContext>(),
            locale.as_str(),
        )?;
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();

        let result = runtime
            .visibility_scoped_read_state_service(db.clone())
            .mark_all_read_with_audience_context(
                tenant_id,
                forum_security(&auth),
                audience_context,
                batch_input(input.unwrap_or_default())?,
            )
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

fn batch_input(
    input: MarkForumTopicsReadBatchGraphqlInput,
) -> Result<MarkForumTopicsReadBatchInput> {
    Ok(MarkForumTopicsReadBatchInput {
        cursor: input.cursor,
        limit: graphql_limit(input.limit)?,
    })
}

fn locale_resolution_outcome(
    requested_locale: &str,
    effective_locale: &str,
    available_locale_count: usize,
) -> &'static str {
    if available_locale_count == 0 {
        LOCALE_OUTCOME_MISSING
    } else if requested_locale == effective_locale {
        LOCALE_OUTCOME_EXACT
    } else {
        LOCALE_OUTCOME_FALLBACK
    }
}

fn forum_graphql_locale_resolution_counter() -> Option<&'static IntCounterVec> {
    let counter = FORUM_GRAPHQL_LOCALE_RESOLUTION_TOTAL.get_or_init(|| {
        IntCounterVec::new(
            Opts::new(
                "rustok_forum_graphql_locale_resolution_total",
                "Forum GraphQL localized items by fixed resource kind and locale resolution outcome",
            ),
            &["resource", "outcome"],
        )
        .expect("Forum GraphQL locale resolution metric descriptor must be valid")
    });

    if FORUM_GRAPHQL_LOCALE_RESOLUTION_REGISTERED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
        && rustok_telemetry::register_runtime_collector(Box::new(counter.clone())).is_err()
    {
        FORUM_GRAPHQL_LOCALE_RESOLUTION_REGISTERED.store(false, Ordering::Release);
        return None;
    }

    Some(counter)
}

fn observe_unread_topic_locale_resolution(items: &[TopicUnreadReadModel]) {
    let Some(counter) = forum_graphql_locale_resolution_counter() else {
        return;
    };

    for item in items {
        let outcome = locale_resolution_outcome(
            &item.topic.requested_locale,
            &item.topic.effective_locale,
            item.topic.available_locales.len(),
        );
        counter
            .with_label_values(&[LOCALE_RESOURCE_UNREAD_TOPIC, outcome])
            .inc();
    }
}

fn unread_topic_activity_state(
    read_state_explicit: bool,
    unread_count: i64,
    has_unread_topic_revision: bool,
) -> &'static str {
    if !read_state_explicit {
        UNREAD_TOPIC_STATE_IMPLICIT
    } else if unread_count > 0 && has_unread_topic_revision {
        UNREAD_TOPIC_STATE_REPLY_AND_REVISION
    } else if unread_count > 0 {
        UNREAD_TOPIC_STATE_REPLY
    } else if has_unread_topic_revision {
        UNREAD_TOPIC_STATE_REVISION
    } else {
        UNREAD_TOPIC_STATE_READ
    }
}

fn forum_graphql_unread_topic_state_counter() -> Option<&'static IntCounterVec> {
    let counter = FORUM_GRAPHQL_UNREAD_TOPIC_STATE_TOTAL.get_or_init(|| {
        IntCounterVec::new(
            Opts::new(
                "rustok_forum_graphql_unread_topic_state_total",
                "Forum GraphQL unread-topic item observations by fixed current unread activity state",
            ),
            &["state"],
        )
        .expect("Forum GraphQL unread-topic state metric descriptor must be valid")
    });

    if FORUM_GRAPHQL_UNREAD_TOPIC_STATE_REGISTERED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
        && rustok_telemetry::register_runtime_collector(Box::new(counter.clone())).is_err()
    {
        FORUM_GRAPHQL_UNREAD_TOPIC_STATE_REGISTERED.store(false, Ordering::Release);
        return None;
    }

    Some(counter)
}

fn observe_unread_topic_activity(items: &[TopicUnreadReadModel]) {
    let Some(counter) = forum_graphql_unread_topic_state_counter() else {
        return;
    };

    for item in items {
        let state = unread_topic_activity_state(
            item.read_state_explicit,
            item.unread_count,
            item.has_unread_topic_revision,
        );
        counter.with_label_values(&[state]).inc();
    }
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

#[cfg(test)]
mod tests {
    use super::{
        LOCALE_OUTCOME_EXACT, LOCALE_OUTCOME_FALLBACK, LOCALE_OUTCOME_MISSING,
        UNREAD_TOPIC_STATE_IMPLICIT, UNREAD_TOPIC_STATE_READ, UNREAD_TOPIC_STATE_REPLY,
        UNREAD_TOPIC_STATE_REPLY_AND_REVISION, UNREAD_TOPIC_STATE_REVISION,
        locale_resolution_outcome, unread_topic_activity_state,
    };

    #[test]
    fn locale_resolution_outcomes_are_fixed_and_do_not_expose_locale_values() {
        assert_eq!(
            locale_resolution_outcome("ru", "ru", 1),
            LOCALE_OUTCOME_EXACT
        );
        assert_eq!(
            locale_resolution_outcome("tenant-secret", "different-secret", 2),
            LOCALE_OUTCOME_FALLBACK
        );
        assert_eq!(
            locale_resolution_outcome("tenant-secret", "tenant-secret", 0),
            LOCALE_OUTCOME_MISSING
        );
    }

    #[test]
    fn unread_topic_activity_states_are_fixed() {
        assert_eq!(
            unread_topic_activity_state(false, 0, false),
            UNREAD_TOPIC_STATE_IMPLICIT
        );
        assert_eq!(
            unread_topic_activity_state(true, 2, true),
            UNREAD_TOPIC_STATE_REPLY_AND_REVISION
        );
        assert_eq!(
            unread_topic_activity_state(true, 2, false),
            UNREAD_TOPIC_STATE_REPLY
        );
        assert_eq!(
            unread_topic_activity_state(true, 0, true),
            UNREAD_TOPIC_STATE_REVISION
        );
        assert_eq!(
            unread_topic_activity_state(true, 0, false),
            UNREAD_TOPIC_STATE_READ
        );
    }
}
