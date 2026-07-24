use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, TransactionTrait, sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::dto::MAX_FORUM_CATEGORY_TREE_NODES;
use crate::entities::{
    forum_category, forum_reply, forum_topic, forum_topic_read_state, forum_topic_revision,
};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;
use crate::state_machine::ReplyStatus;

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
        let topic = forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))?;

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
            .map(|value| decode_bulk_read_cursor(value, scope))
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

        let mut topics = select
            .order_by_asc(forum_topic::Column::CreatedAt)
            .order_by_asc(forum_topic::Column::Id)
            .limit(limit + 1)
            .all(&txn)
            .await?;
        let has_more = topics.len() > limit as usize;
        topics.truncate(limit as usize);

        let topic_ids = topics.iter().map(|topic| topic.id).collect::<Vec<_>>();
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
                    last_read_position: public_positions
                        .get(&topic.id)
                        .copied()
                        .unwrap_or(0),
                    last_read_revision: topic_revisions.get(&topic.id).copied().unwrap_or(0),
                },
                &now,
            )
            .await?;
        }

        let next_cursor = has_more
            .then(|| {
                topics
                    .last()
                    .map(|topic| encode_bulk_read_cursor(scope, &snapshot_at, topic))
            })
            .flatten();
        let processed = topics.len() as u64;
        txn.commit().await?;

        Ok(MarkForumTopicsReadBatchResult {
            processed,
            next_cursor,
            has_more,
            snapshot_at: snapshot_at.to_rfc3339(),
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

    let mut children = HashMap::<Uuid, Vec<Uuid>>::new();
    for category in &categories {
        if let Some(parent_id) = category.parent_id {
            children.entry(parent_id).or_default().push(category.id);
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
        created_at: Set(observed_at.clone()),
        updated_at: Set(observed_at.clone()),
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
        .filter(
            forum_topic_read_state::Column::LastReadPosition.lt(high_water.last_read_position),
        )
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
        .filter(
            forum_topic_read_state::Column::LastReadRevision.lt(high_water.last_read_revision),
        )
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
        .filter(
            forum_topic_read_state::Column::LastReadPosition.lte(high_water.last_read_position),
        )
        .filter(
            forum_topic_read_state::Column::LastReadRevision.lte(high_water.last_read_revision),
        )
        .set(forum_topic_read_state::ActiveModel {
            updated_at: Set(observed_at.clone()),
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
    scope: BulkReadScope,
    snapshot_at: &sea_orm::prelude::DateTimeWithTimeZone,
    topic: &forum_topic::Model,
) -> String {
    format!(
        "{BULK_READ_CURSOR_VERSION}:{}:{}:{}:{}",
        scope.cursor_token(),
        snapshot_at.timestamp_millis(),
        topic.created_at.timestamp_millis(),
        topic.id
    )
}

fn decode_bulk_read_cursor(value: &str, expected_scope: BulkReadScope) -> ForumResult<BulkReadCursor> {
    let mut parts = value.splitn(5, ':');
    if parts.next() != Some(BULK_READ_CURSOR_VERSION)
        || parts.next() != Some(expected_scope.cursor_token().as_str())
    {
        return Err(invalid_bulk_read_cursor());
    }
    let snapshot_millis = parts
        .next()
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(invalid_bulk_read_cursor)?;
    let created_millis = parts
        .next()
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(invalid_bulk_read_cursor)?;
    let topic_id = parts
        .next()
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(invalid_bulk_read_cursor)?;
    let snapshot_at = DateTime::<Utc>::from_timestamp_millis(snapshot_millis)
        .ok_or_else(invalid_bulk_read_cursor)?
        .fixed_offset();
    let created_at = DateTime::<Utc>::from_timestamp_millis(created_millis)
        .ok_or_else(invalid_bulk_read_cursor)?
        .fixed_offset();
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

fn implicit_state(
    tenant_id: Uuid,
    topic_id: Uuid,
    user_id: Option<Uuid>,
) -> ForumTopicReadState {
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
