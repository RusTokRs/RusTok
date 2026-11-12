use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, SecondsFormat, Utc};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, TransactionTrait, sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};

use crate::dto::MAX_FORUM_CATEGORY_TREE_NODES;
use crate::entities::{
    forum_category, forum_reply, forum_topic, forum_topic_read_state, forum_topic_revision,
};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::state_machine::{ReplyStatus, TopicStatus};

use super::topic_read_state_lock::{
    lock_active_topic_read_state_write_in_tx, lock_active_topic_read_state_writes_in_tx,
    lock_topic_read_state_scopes_in_tx,
};

const BULK_READ_CURSOR_VERSION: &str = "br1";
const DEFAULT_BULK_READ_LIMIT: u64 = 50;
const MAX_BULK_READ_LIMIT: u64 = 100;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MarkForumTopicReadInput {
    pub last_read_position: i64,
    pub last_read_revision: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MarkForumTopicsReadBatchInput {
    pub cursor: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MarkForumTopicsReadBatchResult {
    pub processed: u64,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub snapshot_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumTopicReadState {
    pub tenant_id: Uuid,
    pub topic_id: Uuid,
    pub user_id: Option<Uuid>,
    pub last_read_position: i64,
    pub last_read_revision: i64,
    pub explicit: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BulkReadScope {
    Tenant,
    Category(Uuid),
}

impl BulkReadScope {
    fn cursor_token(self) -> String {
        match self {
            Self::Tenant => "all".to_string(),
            Self::Category(category_id) => category_id.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
struct BulkReadCursor {
    snapshot_at: sea_orm::prelude::DateTimeWithTimeZone,
    created_at: sea_orm::prelude::DateTimeWithTimeZone,
    topic_id: Uuid,
}

#[derive(Clone, Copy, Debug)]
struct TopicReadHighWater {
    topic_id: Uuid,
    last_read_position: i64,
    last_read_revision: i64,
}

pub struct ForumTopicReadStateService {
    db: DatabaseConnection,
}

impl ForumTopicReadStateService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn get_topic_read_state(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<ForumTopicReadState> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        ensure_topic_exists(&self.db, tenant_id, topic_id).await?;

        let Some(user_id) = security.user_id else {
            return Ok(implicit_state(tenant_id, topic_id, None));
        };

        let state = forum_topic_read_state::Entity::find()
            .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_read_state::Column::TopicId.eq(topic_id))
            .filter(forum_topic_read_state::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?;

        Ok(state
            .map(explicit_state)
            .unwrap_or_else(|| implicit_state(tenant_id, topic_id, Some(user_id))))
    }

    pub async fn mark_topic_read(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: MarkForumTopicReadInput,
    ) -> ForumResult<ForumTopicReadState> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let user_id = authenticated_user_id(
            &security,
            "Authenticated user context is required to mark a topic read",
        )?;
        validate_nonnegative(&input)?;

        let txn = self.db.begin().await?;
        let topic = lock_active_topic_read_state_write_in_tx(&txn, tenant_id, topic_id).await?;
        lock_topic_read_state_scopes_in_tx(&txn, tenant_id, &[topic_id]).await?;

        let latest_public_position = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(topic.id))
            .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
            .order_by_desc(forum_reply::Column::Position)
            .one(&txn)
            .await?
            .map(|reply| reply.position)
            .unwrap_or(0);
        if input.last_read_position > latest_public_position {
            return Err(ForumError::Validation(format!(
                "Forum topic read position {} exceeds latest public position {latest_public_position}",
                input.last_read_position
            )));
        }

        let latest_topic_revision = forum_topic_revision::Entity::find()
            .filter(forum_topic_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_revision::Column::TopicId.eq(topic.id))
            .order_by_desc(forum_topic_revision::Column::Id)
            .one(&txn)
            .await?
            .map(|revision| revision.id)
            .unwrap_or(0);
        if input.last_read_revision > latest_topic_revision {
            return Err(ForumError::Validation(format!(
                "Forum topic read revision {} exceeds latest topic revision {latest_topic_revision}",
                input.last_read_revision
            )));
        }

        let now: sea_orm::prelude::DateTimeWithTimeZone = Utc::now().into();
        upsert_topic_read_high_water_in_tx(
            &txn,
            tenant_id,
            user_id,
            TopicReadHighWater {
                topic_id: topic.id,
                last_read_position: input.last_read_position,
                last_read_revision: input.last_read_revision,
            },
            &now,
        )
        .await?;

        let state = load_explicit_state_in_tx(&txn, tenant_id, topic.id, user_id).await?;
        txn.commit().await?;

        Ok(explicit_state(state))
    }

    /// Marks the current category subtree read in one bounded, resumable page.
    ///
    /// The first page fixes a topic-creation snapshot. Topics created after that
    /// point are intentionally excluded and are picked up by a later operation.
    pub async fn mark_category_read(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: MarkForumTopicsReadBatchInput,
    ) -> ForumResult<MarkForumTopicsReadBatchResult> {
        self.mark_scope_read(
            tenant_id,
            BulkReadScope::Category(category_id),
            security,
            input,
        )
        .await
    }

    /// Marks all current tenant topics read in one bounded, resumable page.
    pub async fn mark_all_read(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: MarkForumTopicsReadBatchInput,
    ) -> ForumResult<MarkForumTopicsReadBatchResult> {
        self.mark_scope_read(tenant_id, BulkReadScope::Tenant, security, input)
            .await
    }

    async fn mark_scope_read(
        &self,
        tenant_id: Uuid,
        scope: BulkReadScope,
        security: SecurityContext,
        input: MarkForumTopicsReadBatchInput,
    ) -> ForumResult<MarkForumTopicsReadBatchResult> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let user_id = authenticated_user_id(
            &security,
            "Authenticated user context is required to mark forum topics read",
        )?;
        let limit = validated_bulk_read_limit(input.limit)?;
        let cursor = input
            .cursor
            .as_deref()
            .map(|value| decode_bulk_read_cursor(value, tenant_id, user_id, scope))
            .transpose()?;
        let snapshot_at = cursor
            .as_ref()
            .map(|cursor| cursor.snapshot_at)
            .unwrap_or_else(|| Utc::now().into());

        let txn = self.db.begin().await?;
        let category_ids = match scope {
            BulkReadScope::Tenant => None,
            BulkReadScope::Category(category_id) => {
                Some(category_subtree_ids_in_tx(&txn, tenant_id, category_id).await?)
            }
        };

        let mut select = forum_topic::Entity::find()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Status.ne(TopicStatus::Archived))
            .filter(forum_topic::Column::CreatedAt.lte(snapshot_at));
        // Held topics are visible only to their author and moderators; a
        // bulk mark-read must not create read state for topics the caller
        // cannot see (the audience-scoped service enforces the same rule
        // per topic).
        if security.get_scope(Resource::ForumTopics, Action::Moderate) != PermissionScope::All {
            select = select.filter(
                Condition::any()
                    .add(forum_topic::Column::Status.ne(TopicStatus::Pending))
                    .add(forum_topic::Column::AuthorId.eq(user_id)),
            );
        }
        if let Some(category_ids) = category_ids {
            select = select.filter(forum_topic::Column::CategoryId.is_in(category_ids));
        }
        if let Some(cursor) = cursor.as_ref() {
            select = select.filter(
                Condition::any()
                    .add(forum_topic::Column::CreatedAt.gt(cursor.created_at))
                    .add(
                        Condition::all()
                            .add(forum_topic::Column::CreatedAt.eq(cursor.created_at))
                            .add(forum_topic::Column::Id.gt(cursor.topic_id)),
                    ),
            );
        }

        let mut topics = select
            .order_by_asc(forum_topic::Column::CreatedAt)
            .order_by_asc(forum_topic::Column::Id)
            .limit(limit + 1)
            .all(&txn)
            .await?;
        let has_more = topics.len() > limit as usize;
        topics.truncate(limit as usize);

        let topic_ids = topics.iter().map(|topic| topic.id).collect::<Vec<_>>();
        lock_active_topic_read_state_writes_in_tx(&txn, tenant_id, &topic_ids).await?;
        lock_topic_read_state_scopes_in_tx(&txn, tenant_id, &topic_ids).await?;
        let public_positions = latest_public_positions_in_tx(&txn, tenant_id, &topic_ids).await?;
        let topic_revisions = latest_topic_revisions_in_tx(&txn, tenant_id, &topic_ids).await?;
        let now: sea_orm::prelude::DateTimeWithTimeZone = Utc::now().into();
        for topic in &topics {
            upsert_topic_read_high_water_in_tx(
                &txn,
                tenant_id,
                user_id,
                TopicReadHighWater {
                    topic_id: topic.id,
                    last_read_position: public_positions.get(&topic.id).copied().unwrap_or(0),
                    last_read_revision: topic_revisions.get(&topic.id).copied().unwrap_or(0),
                },
                &now,
            )
            .await?;
        }

        let next_cursor = has_more
            .then(|| {
                topics.last().map(|topic| {
                    encode_bulk_read_cursor(tenant_id, user_id, scope, &snapshot_at, topic)
                })
            })
            .flatten();
        let processed = topics.len() as u64;
        txn.commit().await?;

        Ok(MarkForumTopicsReadBatchResult {
            processed,
            next_cursor,
            has_more,
            snapshot_at: snapshot_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
        })
    }
}

fn authenticated_user_id(security: &SecurityContext, message: &str) -> ForumResult<Uuid> {
    security
        .user_id
        .ok_or_else(|| ForumError::forbidden(message))
}

fn validated_bulk_read_limit(limit: Option<u64>) -> ForumResult<u64> {
    let limit = limit.unwrap_or(DEFAULT_BULK_READ_LIMIT);
    if !(1..=MAX_BULK_READ_LIMIT).contains(&limit) {
        return Err(ForumError::Validation(format!(
            "Forum bulk read limit must be between 1 and {MAX_BULK_READ_LIMIT}"
        )));
    }
    Ok(limit)
}

fn validate_nonnegative(input: &MarkForumTopicReadInput) -> ForumResult<()> {
    if input.last_read_position < 0 {
        return Err(ForumError::Validation(
            "Forum topic read position must be nonnegative".to_string(),
        ));
    }
    if input.last_read_revision < 0 {
        return Err(ForumError::Validation(
            "Forum topic read revision must be nonnegative".to_string(),
        ));
    }
    Ok(())
}

async fn category_subtree_ids_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    root_category_id: Uuid,
) -> ForumResult<Vec<Uuid>> {
    category_subtree_ids_with_port_in_tx(
        &crate::ports::taxonomy::NoopForumTaxonomyPort,
        txn,
        tenant_id,
        root_category_id,
    )
    .await
}

async fn category_subtree_ids_with_port_in_tx(
    port: &dyn crate::ports::taxonomy::ForumTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    root_category_id: Uuid,
) -> ForumResult<Vec<Uuid>> {
    let categories = forum_category::Entity::find()
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .order_by_asc(forum_category::Column::Id)
        .limit(MAX_FORUM_CATEGORY_TREE_NODES + 1)
        .all(txn)
        .await?;
    if categories.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
        return Err(ForumError::Validation(format!(
            "Forum category subtree exceeds the {MAX_FORUM_CATEGORY_TREE_NODES}-node owner bound"
        )));
    }
    if !categories
        .iter()
        .any(|category| category.id == root_category_id)
    {
        return Err(ForumError::CategoryNotFound(root_category_id));
    }

    let category_ids = categories.iter().map(|c| c.id).collect::<Vec<_>>();
    let parents = port
        .load_category_parents(
            txn.into(),
            tenant_id,
            &category_ids,
        )
        .await?;
    let mut children = HashMap::<Uuid, Vec<Uuid>>::new();
    for (term_id, parent_id) in parents {
        if let Some(parent_id) = parent_id {
            children.entry(parent_id).or_default().push(term_id);
        }
    }

    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([root_category_id]);
    while let Some(category_id) = queue.pop_front() {
        if !seen.insert(category_id) {
            return Err(ForumError::Validation(
                "Forum category subtree contains a cycle".to_string(),
            ));
        }
        if let Some(child_ids) = children.get(&category_id) {
            queue.extend(child_ids.iter().copied());
        }
    }

    let mut ids = seen.into_iter().collect::<Vec<_>>();
    ids.sort_unstable();
    Ok(ids)
}

async fn latest_public_positions_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, i64>> {
    if topic_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = forum_reply::Entity::find()
        .select_only()
        .column(forum_reply::Column::TopicId)
        .column_as(forum_reply::Column::Position.max(), "last_read_position")
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.is_in(topic_ids.to_vec()))
        .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
        .group_by(forum_reply::Column::TopicId)
        .into_tuple::<(Uuid, Option<i64>)>()
        .all(txn)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(topic_id, position)| (topic_id, position.unwrap_or(0)))
        .collect())
}

async fn latest_topic_revisions_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_ids: &[Uuid],
) -> ForumResult<HashMap<Uuid, i64>> {
    if topic_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = forum_topic_revision::Entity::find()
        .select_only()
        .column(forum_topic_revision::Column::TopicId)
        .column_as(forum_topic_revision::Column::Id.max(), "last_read_revision")
        .filter(forum_topic_revision::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_revision::Column::TopicId.is_in(topic_ids.to_vec()))
        .group_by(forum_topic_revision::Column::TopicId)
        .into_tuple::<(Uuid, Option<i64>)>()
        .all(txn)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(topic_id, revision)| (topic_id, revision.unwrap_or(0)))
        .collect())
}

async fn upsert_topic_read_high_water_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    user_id: Uuid,
    high_water: TopicReadHighWater,
    observed_at: &sea_orm::prelude::DateTimeWithTimeZone,
) -> ForumResult<()> {
    forum_topic_read_state::Entity::insert(forum_topic_read_state::ActiveModel {
        tenant_id: Set(tenant_id),
        topic_id: Set(high_water.topic_id),
        user_id: Set(user_id),
        last_read_position: Set(high_water.last_read_position),
        last_read_revision: Set(high_water.last_read_revision),
        created_at: Set(*observed_at),
        updated_at: Set(*observed_at),
    })
    .on_conflict(
        OnConflict::columns([
            forum_topic_read_state::Column::TenantId,
            forum_topic_read_state::Column::TopicId,
            forum_topic_read_state::Column::UserId,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec_without_returning(txn)
    .await?;

    forum_topic_read_state::Entity::update_many()
        .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_read_state::Column::TopicId.eq(high_water.topic_id))
        .filter(forum_topic_read_state::Column::UserId.eq(user_id))
        .filter(forum_topic_read_state::Column::LastReadPosition.lt(high_water.last_read_position))
        .set(forum_topic_read_state::ActiveModel {
            last_read_position: Set(high_water.last_read_position),
            ..Default::default()
        })
        .exec(txn)
        .await?;

    forum_topic_read_state::Entity::update_many()
        .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_read_state::Column::TopicId.eq(high_water.topic_id))
        .filter(forum_topic_read_state::Column::UserId.eq(user_id))
        .filter(forum_topic_read_state::Column::LastReadRevision.lt(high_water.last_read_revision))
        .set(forum_topic_read_state::ActiveModel {
            last_read_revision: Set(high_water.last_read_revision),
            ..Default::default()
        })
        .exec(txn)
        .await?;

    forum_topic_read_state::Entity::update_many()
        .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_read_state::Column::TopicId.eq(high_water.topic_id))
        .filter(forum_topic_read_state::Column::UserId.eq(user_id))
        .filter(forum_topic_read_state::Column::LastReadPosition.lte(high_water.last_read_position))
        .filter(forum_topic_read_state::Column::LastReadRevision.lte(high_water.last_read_revision))
        .set(forum_topic_read_state::ActiveModel {
            updated_at: Set(*observed_at),
            ..Default::default()
        })
        .exec(txn)
        .await?;

    Ok(())
}

async fn load_explicit_state_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    user_id: Uuid,
) -> ForumResult<forum_topic_read_state::Model> {
    forum_topic_read_state::Entity::find()
        .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_read_state::Column::TopicId.eq(topic_id))
        .filter(forum_topic_read_state::Column::UserId.eq(user_id))
        .one(txn)
        .await?
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum topic read state disappeared during monotonic update".to_string(),
            )
        })
}

fn encode_bulk_read_cursor(
    tenant_id: Uuid,
    user_id: Uuid,
    scope: BulkReadScope,
    snapshot_at: &sea_orm::prelude::DateTimeWithTimeZone,
    topic: &forum_topic::Model,
) -> String {
    format!(
        "{BULK_READ_CURSOR_VERSION}|{tenant_id}|{user_id}|{}|{}|{}|{}",
        scope.cursor_token(),
        snapshot_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
        topic.created_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
        topic.id
    )
}

fn decode_bulk_read_cursor(
    value: &str,
    expected_tenant_id: Uuid,
    expected_user_id: Uuid,
    expected_scope: BulkReadScope,
) -> ForumResult<BulkReadCursor> {
    let expected_scope_token = expected_scope.cursor_token();
    let mut parts = value.splitn(7, '|');
    if parts.next() != Some(BULK_READ_CURSOR_VERSION)
        || parts.next() != Some(expected_tenant_id.to_string().as_str())
        || parts.next() != Some(expected_user_id.to_string().as_str())
        || parts.next() != Some(expected_scope_token.as_str())
    {
        return Err(invalid_bulk_read_cursor());
    }
    let snapshot_at = parts
        .next()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .ok_or_else(invalid_bulk_read_cursor)?;
    let created_at = parts
        .next()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .ok_or_else(invalid_bulk_read_cursor)?;
    let topic_id = parts
        .next()
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(invalid_bulk_read_cursor)?;
    if created_at > snapshot_at {
        return Err(invalid_bulk_read_cursor());
    }
    Ok(BulkReadCursor {
        snapshot_at,
        created_at,
        topic_id,
    })
}

fn invalid_bulk_read_cursor() -> ForumError {
    ForumError::Validation("Invalid forum bulk read cursor".to_string())
}

async fn ensure_topic_exists(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    forum_topic::Entity::find_by_id(topic_id)
        .filter(forum_topic::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .map(|_| ())
        .ok_or(ForumError::TopicNotFound(topic_id))
}

fn explicit_state(model: forum_topic_read_state::Model) -> ForumTopicReadState {
    ForumTopicReadState {
        tenant_id: model.tenant_id,
        topic_id: model.topic_id,
        user_id: Some(model.user_id),
        last_read_position: model.last_read_position,
        last_read_revision: model.last_read_revision,
        explicit: true,
        created_at: Some(model.created_at.to_rfc3339()),
        updated_at: Some(model.updated_at.to_rfc3339()),
    }
}

fn implicit_state(tenant_id: Uuid, topic_id: Uuid, user_id: Option<Uuid>) -> ForumTopicReadState {
    ForumTopicReadState {
        tenant_id,
        topic_id,
        user_id,
        last_read_position: 0,
        last_read_revision: 0,
        explicit: false,
        created_at: None,
        updated_at: None,
    }
}

const VISIBILITY_BULK_READ_CURSOR_VERSION: &str = "brv1";

#[derive(Clone, Debug)]
struct VisibilityBulkReadCursor {
    snapshot_at: sea_orm::prelude::DateTimeWithTimeZone,
    created_at: sea_orm::prelude::DateTimeWithTimeZone,
    topic_id: Uuid,
}

/// Exact visibility-scoped storefront bulk read owner.
///
/// Raw topic candidates remain bounded and cursor ordered, while only topics
/// admitted by the current route-channel and richer audience owners receive a
/// read-state update. A raw page may therefore advance with `processed == 0`.
pub struct ForumVisibilityScopedReadStateService {
    db: DatabaseConnection,
    audience_facts: Option<crate::audience::SharedForumAudienceFactsPort>,
}

impl ForumVisibilityScopedReadStateService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_optional_audience_facts(db, None)
    }

    pub fn with_audience_facts(
        db: DatabaseConnection,
        audience_facts: crate::audience::SharedForumAudienceFactsPort,
    ) -> Self {
        Self::with_optional_audience_facts(db, Some(audience_facts))
    }

    fn with_optional_audience_facts(
        db: DatabaseConnection,
        audience_facts: Option<crate::audience::SharedForumAudienceFactsPort>,
    ) -> Self {
        Self { db, audience_facts }
    }

    /// Marks the exact currently visible topics in one category subtree read.
    pub async fn mark_category_read_with_audience_context(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        context: rustok_api::PortContext,
        input: MarkForumTopicsReadBatchInput,
    ) -> ForumResult<MarkForumTopicsReadBatchResult> {
        self.mark_scope_read_with_audience_context(
            tenant_id,
            BulkReadScope::Category(category_id),
            security,
            context,
            input,
        )
        .await
    }

    /// Marks the exact currently visible topics in the tenant storefront read.
    pub async fn mark_all_read_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: rustok_api::PortContext,
        input: MarkForumTopicsReadBatchInput,
    ) -> ForumResult<MarkForumTopicsReadBatchResult> {
        self.mark_scope_read_with_audience_context(
            tenant_id,
            BulkReadScope::Tenant,
            security,
            context,
            input,
        )
        .await
    }

    /// Owner read state for one topic, gated by the exact topic audience.
    ///
    /// Missing and audience-denied topics are indistinguishable
    /// (`TopicNotFound`). This is the single-topic counterpart of the bulk
    /// operations below; transports must not call the unaudited
    /// `ForumTopicReadStateService` directly, or hidden topics leak their
    /// existence through read-state responses.
    pub async fn get_topic_read_state_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: rustok_api::PortContext,
    ) -> ForumResult<ForumTopicReadState> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        self.require_topic_owner_visible(tenant_id, topic_id, &security, context)
            .await?;
        ForumTopicReadStateService::new(self.db.clone())
            .get_topic_read_state(tenant_id, topic_id, security)
            .await
    }

    /// Marks one topic read after the same audience gate as
    /// [`Self::get_topic_read_state_with_audience_context`]. The delegated
    /// owner write still validates the high-water marks against the latest
    /// approved reply position and topic revision.
    pub async fn mark_topic_read_with_audience_context(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: rustok_api::PortContext,
        input: MarkForumTopicReadInput,
    ) -> ForumResult<ForumTopicReadState> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        authenticated_user_id(
            &security,
            "Authenticated user context is required to mark a topic read",
        )?;
        self.require_topic_owner_visible(tenant_id, topic_id, &security, context)
            .await?;
        ForumTopicReadStateService::new(self.db.clone())
            .mark_topic_read(tenant_id, topic_id, security, input)
            .await
    }

    async fn require_topic_owner_visible(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: &SecurityContext,
        context: rustok_api::PortContext,
    ) -> ForumResult<()> {
        validate_visibility_bulk_context(tenant_id, &context)?;
        let viewer = if security.user_id.is_some() {
            super::topic_audience_visibility::ForumTopicAudienceViewer::authenticated(
                security.clone(),
                context,
            )?
        } else {
            super::topic_audience_visibility::ForumTopicAudienceViewer::public()
        };
        let visible = super::topic_audience_visibility::ForumTopicAudienceVisibilityService::new(
            self.db.clone(),
            self.audience_facts.clone(),
        )
        .is_topic_owner_visible(tenant_id, topic_id, &viewer)
        .await?;
        if !visible {
            return Err(ForumError::TopicNotFound(topic_id));
        }
        Ok(())
    }

    async fn mark_scope_read_with_audience_context(
        &self,
        tenant_id: Uuid,
        scope: BulkReadScope,
        security: SecurityContext,
        context: rustok_api::PortContext,
        input: MarkForumTopicsReadBatchInput,
    ) -> ForumResult<MarkForumTopicsReadBatchResult> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let user_id = authenticated_user_id(
            &security,
            "Authenticated user context is required to mark visible forum topics read",
        )?;
        validate_visibility_bulk_context(tenant_id, &context)?;
        let limit = validated_bulk_read_limit(input.limit)?;

        let route_scope =
            super::topic_visibility::ForumTopicVisibilityScope::storefront_for_viewer(
                context.channel.as_deref(),
                true,
            )?;
        let channel_slug = route_scope.channel_slug().map(str::to_string);
        let channel_scope_token = visibility_channel_scope_token(channel_slug.as_deref());
        let cursor = input
            .cursor
            .as_deref()
            .map(|value| {
                decode_visibility_bulk_read_cursor(
                    value,
                    tenant_id,
                    user_id,
                    scope,
                    &channel_scope_token,
                )
            })
            .transpose()?;
        let snapshot_at = cursor
            .as_ref()
            .map(|cursor| cursor.snapshot_at)
            .unwrap_or_else(|| Utc::now().into());

        let topic_viewer =
            super::topic_audience_visibility::ForumTopicAudienceViewer::authenticated(
                security.clone(),
                context.clone(),
            )?;

        if let BulkReadScope::Category(category_id) = scope {
            let category_viewer =
                super::category_audience_visibility::ForumCategoryAudienceViewer::authenticated(
                    security.clone(),
                    context.clone(),
                )?;
            if !super::category_audience_visibility::ForumCategoryAudienceVisibilityService::new(
                self.db.clone(),
                self.audience_facts.clone(),
            )
            .is_category_visible(tenant_id, category_id, &category_viewer)
            .await?
            {
                return Err(ForumError::CategoryNotFound(category_id));
            }
        }

        let candidate_txn = self.db.begin().await?;
        let category_ids = match scope {
            BulkReadScope::Tenant => None,
            BulkReadScope::Category(category_id) => {
                Some(category_subtree_ids_in_tx(&candidate_txn, tenant_id, category_id).await?)
            }
        };

        let mut select = forum_topic::Entity::find()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Status.ne(TopicStatus::Archived))
            .filter(forum_topic::Column::CreatedAt.lte(snapshot_at));
        if let Some(category_ids) = category_ids {
            select = select.filter(forum_topic::Column::CategoryId.is_in(category_ids));
        }
        if let Some(cursor) = cursor.as_ref() {
            select = select.filter(
                Condition::any()
                    .add(forum_topic::Column::CreatedAt.gt(cursor.created_at))
                    .add(
                        Condition::all()
                            .add(forum_topic::Column::CreatedAt.eq(cursor.created_at))
                            .add(forum_topic::Column::Id.gt(cursor.topic_id)),
                    ),
            );
        }

        let mut candidates = select
            .order_by_asc(forum_topic::Column::CreatedAt)
            .order_by_asc(forum_topic::Column::Id)
            .limit(limit + 1)
            .all(&candidate_txn)
            .await?;
        candidate_txn.commit().await?;

        let has_more = candidates.len() > limit as usize;
        candidates.truncate(limit as usize);

        let visibility = super::topic_audience_visibility::ForumTopicAudienceVisibilityService::new(
            self.db.clone(),
            self.audience_facts.clone(),
        );
        let mut visible_topic_ids = Vec::with_capacity(candidates.len());
        for topic in &candidates {
            if visibility
                .is_topic_visible(tenant_id, topic.id, channel_slug.as_deref(), &topic_viewer)
                .await?
            {
                visible_topic_ids.push(topic.id);
            }
        }

        if !visible_topic_ids.is_empty() {
            let write_txn = self.db.begin().await?;
            lock_active_topic_read_state_writes_in_tx(&write_txn, tenant_id, &visible_topic_ids)
                .await?;
            lock_topic_read_state_scopes_in_tx(&write_txn, tenant_id, &visible_topic_ids).await?;
            let public_positions =
                latest_public_positions_in_tx(&write_txn, tenant_id, &visible_topic_ids).await?;
            let topic_revisions =
                latest_topic_revisions_in_tx(&write_txn, tenant_id, &visible_topic_ids).await?;
            let observed_at: sea_orm::prelude::DateTimeWithTimeZone = Utc::now().into();
            for topic_id in &visible_topic_ids {
                upsert_topic_read_high_water_in_tx(
                    &write_txn,
                    tenant_id,
                    user_id,
                    TopicReadHighWater {
                        topic_id: *topic_id,
                        last_read_position: public_positions.get(topic_id).copied().unwrap_or(0),
                        last_read_revision: topic_revisions.get(topic_id).copied().unwrap_or(0),
                    },
                    &observed_at,
                )
                .await?;
            }
            write_txn.commit().await?;
        }

        let next_cursor = if has_more {
            candidates.last().map(|topic| {
                encode_visibility_bulk_read_cursor(
                    tenant_id,
                    user_id,
                    scope,
                    &channel_scope_token,
                    &snapshot_at,
                    topic,
                )
            })
        } else {
            None
        };

        Ok(MarkForumTopicsReadBatchResult {
            processed: visible_topic_ids.len() as u64,
            next_cursor,
            has_more,
            snapshot_at: snapshot_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
        })
    }
}

fn validate_visibility_bulk_context(
    tenant_id: Uuid,
    context: &rustok_api::PortContext,
) -> ForumResult<()> {
    let context_tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        ForumError::Validation(
            "Forum visibility-scoped bulk read context tenant is invalid".to_string(),
        )
    })?;
    if context_tenant_id != tenant_id {
        return Err(ForumError::Validation(
            "Forum visibility-scoped bulk read context tenant does not match the request"
                .to_string(),
        ));
    }
    Ok(())
}

fn visibility_channel_scope_token(channel_slug: Option<&str>) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(channel_slug.unwrap_or("<no-route-channel>").as_bytes());
    hex::encode(hasher.finalize())
}

fn encode_visibility_bulk_read_cursor(
    tenant_id: Uuid,
    user_id: Uuid,
    scope: BulkReadScope,
    channel_scope_token: &str,
    snapshot_at: &sea_orm::prelude::DateTimeWithTimeZone,
    topic: &forum_topic::Model,
) -> String {
    format!(
        "{VISIBILITY_BULK_READ_CURSOR_VERSION}|{tenant_id}|{user_id}|{}|{channel_scope_token}|{}|{}|{}",
        scope.cursor_token(),
        snapshot_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
        topic.created_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
        topic.id,
    )
}

fn decode_visibility_bulk_read_cursor(
    value: &str,
    expected_tenant_id: Uuid,
    expected_user_id: Uuid,
    expected_scope: BulkReadScope,
    expected_channel_scope_token: &str,
) -> ForumResult<VisibilityBulkReadCursor> {
    let expected_tenant = expected_tenant_id.to_string();
    let expected_user = expected_user_id.to_string();
    let expected_scope_token = expected_scope.cursor_token();
    let mut parts = value.splitn(8, '|');
    if parts.next() != Some(VISIBILITY_BULK_READ_CURSOR_VERSION)
        || parts.next() != Some(expected_tenant.as_str())
        || parts.next() != Some(expected_user.as_str())
        || parts.next() != Some(expected_scope_token.as_str())
        || parts.next() != Some(expected_channel_scope_token)
    {
        return Err(invalid_visibility_bulk_read_cursor());
    }
    let snapshot_at = parts
        .next()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .ok_or_else(invalid_visibility_bulk_read_cursor)?;
    let created_at = parts
        .next()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .ok_or_else(invalid_visibility_bulk_read_cursor)?;
    let topic_id = parts
        .next()
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(invalid_visibility_bulk_read_cursor)?;
    if created_at > snapshot_at {
        return Err(invalid_visibility_bulk_read_cursor());
    }
    Ok(VisibilityBulkReadCursor {
        snapshot_at,
        created_at,
        topic_id,
    })
}

fn invalid_visibility_bulk_read_cursor() -> ForumError {
    ForumError::Validation("Invalid forum visibility-scoped bulk read cursor".to_string())
}
