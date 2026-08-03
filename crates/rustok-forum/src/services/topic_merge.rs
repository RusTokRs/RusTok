use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Statement, TransactionTrait,
    prelude::DateTimeWithTimeZone,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::entities::{
    forum_category_lifecycle, forum_domain_event, forum_reply, forum_solution, forum_topic,
    forum_topic_merge_operation,
};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::{ReplyStatus, TopicStatus};

use super::projection_invalidation::{
    publish_forum_category_projection_in_tx, publish_forum_topic_projection_in_tx,
};
use super::rbac::enforce_scope;
use super::topic_audience::load_policy_for_topic;

pub const MAX_FORUM_TOPIC_MERGE_REASON_LEN: usize = 500;
pub const MAX_FORUM_TOPIC_MERGE_REPLIES: u64 = 500;
const FORUM_TOPIC_MERGED_EVENT_TYPE: &str = "forum.topic.merged";
const FORUM_TOPIC_MERGED_AGGREGATE_TYPE: &str = "forum_topic";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeForumTopicInput {
    pub operation_id: Uuid,
    pub source_topic_id: Uuid,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumTopicMergeResult {
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub source_topic_id: Uuid,
    pub target_topic_id: Uuid,
    pub category_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub moved_reply_count: i32,
    pub moved_published_reply_count: i32,
    pub resulting_published_reply_count: i32,
    pub position_offset: i64,
    pub merged_at: DateTime<Utc>,
}

#[derive(Clone)]
struct ForumTopicMergeSolutionTransfer {
    reply_id: Uuid,
    marked_by_user_id: Option<Uuid>,
    marked_at: DateTimeWithTimeZone,
}

/// Idempotent same-category merge of one active source topic into one retained target topic.
///
/// The target identity and topic-owned policy remain authoritative. Reply identities and all
/// reply-owned relations are retained while reply positions are shifted after the target's
/// current maximum. A source-only accepted solution follows its unchanged reply identity and
/// preserves its marker metadata; two accepted solutions require explicit resolution. The source
/// topic becomes an archived, locked redirect-ready identity. Topic subscriptions, tags and
/// topic-level audience relations are reconciled by their dedicated bounded policies.
pub struct ForumTopicMergeService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ForumTopicMergeService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, security, input))]
    pub async fn merge_topic(
        &self,
        tenant_id: Uuid,
        target_topic_id: Uuid,
        security: SecurityContext,
        input: MergeForumTopicInput,
    ) -> ForumResult<ForumTopicMergeResult> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        let actor_id = security.user_id.ok_or_else(|| {
            ForumError::Validation("Forum topic merge requires a human actor".to_string())
        })?;
        let reason = validate_merge_input(tenant_id, target_topic_id, actor_id, &input)?;

        let txn = self.db.begin().await?;
        lock_topic_merge_tenant_in_tx(&txn, tenant_id).await?;

        if let Some(existing) = forum_topic_merge_operation::Entity::find_by_id((
            tenant_id,
            input.operation_id,
        ))
        .one(&txn)
        .await?
        {
            if existing.source_topic_id != input.source_topic_id
                || existing.target_topic_id != target_topic_id
                || existing.actor_id != actor_id
                || existing.reason != reason
            {
                return Err(ForumError::TopicMergeOperationConflict(input.operation_id));
            }
            validate_existing_semantic_event_in_tx(&txn, &existing).await?;
            txn.commit().await?;
            return Ok(operation_to_result(existing));
        }

        let preliminary_source =
            find_topic_in_tx(&txn, tenant_id, input.source_topic_id).await?;
        let preliminary_target = find_topic_in_tx(&txn, tenant_id, target_topic_id).await?;
        if preliminary_source.category_id != preliminary_target.category_id {
            return Err(ForumError::Validation(
                "Forum topic merge requires source and target topics in the same category"
                    .to_string(),
            ));
        }
        lock_merge_counter_scopes_in_tx(
            &txn,
            tenant_id,
            preliminary_target.category_id,
            input.source_topic_id,
            target_topic_id,
        )
        .await?;
        lock_topics_in_tx(&txn, tenant_id, input.source_topic_id, target_topic_id).await?;

        let source = find_topic_in_tx(&txn, tenant_id, input.source_topic_id).await?;
        let target = find_topic_in_tx(&txn, tenant_id, target_topic_id).await?;
        if source.category_id != preliminary_source.category_id
            || target.category_id != preliminary_target.category_id
        {
            return Err(ForumError::Validation(
                "Forum topic merge category changed concurrently".to_string(),
            ));
        }
        if source.status == TopicStatus::Archived || target.status == TopicStatus::Archived {
            return Err(ForumError::Validation(
                "Forum topic merge requires active source and target topics".to_string(),
            ));
        }
        source.status.validate_transition(&TopicStatus::Archived)?;
        if source.category_id != target.category_id {
            return Err(ForumError::Validation(
                "Forum topic merge requires source and target topics in the same category"
                    .to_string(),
            ));
        }
        ensure_category_active_in_tx(&txn, tenant_id, target.category_id).await?;

        lock_topic_solution_scopes_in_tx(&txn, tenant_id, &[source.id, target.id]).await?;
        let source_solution =
            load_valid_solution_in_tx(&txn, tenant_id, source.id, "source").await?;
        let target_solution =
            load_valid_solution_in_tx(&txn, tenant_id, target.id, "target").await?;
        if source_solution.is_some() && target_solution.is_some() {
            return Err(ForumError::TopicMergeSolutionConflict(input.operation_id));
        }
        let source_solution_transfer = source_solution.map(|solution| {
            ForumTopicMergeSolutionTransfer {
                reply_id: solution.reply_id,
                marked_by_user_id: solution.marked_by_user_id,
                marked_at: solution.marked_at,
            }
        });

        let source_reply_count = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(source.id))
            .count(&txn)
            .await?;
        if source_reply_count > MAX_FORUM_TOPIC_MERGE_REPLIES {
            return Err(ForumError::Validation(format!(
                "Forum topic merge source must not exceed {MAX_FORUM_TOPIC_MERGE_REPLIES} replies"
            )));
        }
        let moved_reply_count = i32::try_from(source_reply_count).map_err(|_| {
            ForumError::Validation("Forum topic merge reply count exceeds supported range".into())
        })?;
        let moved_published_reply_count =
            approved_reply_count_in_tx(&txn, tenant_id, source.id).await?;
        if moved_published_reply_count != source.reply_count {
            return Err(ForumError::Validation(
                "Forum source topic published reply counter is inconsistent".to_string(),
            ));
        }
        let target_published_reply_count =
            approved_reply_count_in_tx(&txn, tenant_id, target.id).await?;
        if target_published_reply_count != target.reply_count {
            return Err(ForumError::Validation(
                "Forum target topic published reply counter is inconsistent".to_string(),
            ));
        }
        let resulting_published_reply_count = target_published_reply_count
            .checked_add(moved_published_reply_count)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum target topic published reply counter overflow".to_string(),
                )
            })?;

        let position_offset = max_reply_position_in_tx(&txn, tenant_id, target.id).await?;
        let source_max_position = max_reply_position_in_tx(&txn, tenant_id, source.id).await?;
        position_offset
            .checked_add(source_max_position)
            .ok_or_else(|| {
                ForumError::Validation("Forum merged reply position overflow".to_string())
            })?;

        if source_solution_transfer.is_some() {
            delete_source_solution_in_tx(&txn, tenant_id, source.id).await?;
        }
        move_replies_in_tx(
            &txn,
            tenant_id,
            source.id,
            target.id,
            position_offset,
            source_reply_count,
        )
        .await?;
        if let Some(solution) = source_solution_transfer {
            insert_transferred_solution_in_tx(&txn, tenant_id, target.id, &solution).await?;
            let transferred =
                load_valid_solution_in_tx(&txn, tenant_id, target.id, "transferred target")
                    .await?
                    .ok_or_else(|| {
                        ForumError::Validation(
                            "Forum transferred accepted solution is missing".to_string(),
                        )
                    })?;
            if transferred.reply_id != solution.reply_id
                || transferred.marked_by_user_id != solution.marked_by_user_id
                || transferred.marked_at != solution.marked_at
            {
                return Err(ForumError::Validation(
                    "Forum transferred accepted solution metadata changed".to_string(),
                ));
            }
        }

        let now = Utc::now();
        let mut target_active: forum_topic::ActiveModel = target.into();
        target_active.reply_count = Set(resulting_published_reply_count);
        target_active.updated_at = Set(now.into());
        let target = target_active.update(&txn).await?;
        load_policy_for_topic(&txn, tenant_id, &target).await?;

        let mut source_active: forum_topic::ActiveModel = source.into();
        source_active.status = Set(TopicStatus::Archived);
        source_active.is_locked = Set(true);
        source_active.reply_count = Set(0);
        source_active.last_reply_at = Set(None);
        source_active.updated_at = Set(now.into());
        source_active.update(&txn).await?;

        let payload = topic_merged_payload(
            input.operation_id,
            input.source_topic_id,
            target_topic_id,
            target.category_id,
            moved_reply_count,
            moved_published_reply_count,
            resulting_published_reply_count,
            position_offset,
            &reason,
        );
        forum_domain_event::ActiveModel {
            sequence_no: NotSet,
            event_id: Set(input.operation_id),
            tenant_id: Set(tenant_id),
            aggregate_type: Set(FORUM_TOPIC_MERGED_AGGREGATE_TYPE.to_string()),
            aggregate_id: Set(target_topic_id),
            event_type: Set(FORUM_TOPIC_MERGED_EVENT_TYPE.to_string()),
            schema_version: Set(1),
            actor_id: Set(Some(actor_id)),
            payload: Set(payload),
            created_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        let operation = forum_topic_merge_operation::ActiveModel {
            tenant_id: Set(tenant_id),
            operation_id: Set(input.operation_id),
            source_topic_id: Set(input.source_topic_id),
            target_topic_id: Set(target_topic_id),
            category_id: Set(target.category_id),
            actor_id: Set(actor_id),
            reason: Set(reason),
            moved_reply_count: Set(moved_reply_count),
            moved_published_reply_count: Set(moved_published_reply_count),
            resulting_published_reply_count: Set(resulting_published_reply_count),
            position_offset: Set(position_offset),
            event_id: Set(input.operation_id),
            merged_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            Some(actor_id),
            input.source_topic_id,
        )
        .await?;
        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            Some(actor_id),
            target_topic_id,
        )
        .await?;
        publish_forum_category_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            Some(actor_id),
            target.category_id,
        )
        .await?;

        txn.commit().await?;
        Ok(operation_to_result(operation))
    }
}

fn validate_merge_input(
    tenant_id: Uuid,
    target_topic_id: Uuid,
    actor_id: Uuid,
    input: &MergeForumTopicInput,
) -> ForumResult<String> {
    for (label, value) in [
        ("tenant", tenant_id),
        ("operation", input.operation_id),
        ("source topic", input.source_topic_id),
        ("target topic", target_topic_id),
        ("actor", actor_id),
    ] {
        if value.is_nil() {
            return Err(ForumError::Validation(format!(
                "Forum topic merge {label} must not be nil"
            )));
        }
    }
    if input.source_topic_id == target_topic_id {
        return Err(ForumError::Validation(
            "Forum topic merge source and target must differ".to_string(),
        ));
    }
    let reason = input.reason.trim();
    if reason.is_empty() {
        return Err(ForumError::Validation(
            "Forum topic merge reason must not be empty".to_string(),
        ));
    }
    if reason.chars().count() > MAX_FORUM_TOPIC_MERGE_REASON_LEN {
        return Err(ForumError::Validation(format!(
            "Forum topic merge reason must not exceed {MAX_FORUM_TOPIC_MERGE_REASON_LEN} characters"
        )));
    }
    if reason.chars().any(char::is_control) {
        return Err(ForumError::Validation(
            "Forum topic merge reason must not contain control characters".to_string(),
        ));
    }
    Ok(reason.to_string())
}

async fn lock_topic_merge_tenant_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 21))",
                vec![format!("forum-topic-merge:{tenant_id}").into()],
            ))
            .await?;
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                vec![tenant_id.to_string().into()],
            ))
            .await?;
        }
        DatabaseBackend::Sqlite => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"
                INSERT INTO forum_topic_merge_locks (tenant_id, touched_at)
                VALUES (?, CURRENT_TIMESTAMP)
                ON CONFLICT(tenant_id) DO UPDATE SET touched_at = CURRENT_TIMESTAMP
                "#,
                vec![tenant_id.into()],
            ))
            .await?;
        }
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic merge does not support database backend {backend:?}"
            )));
        }
    }
    Ok(())
}

async fn lock_merge_counter_scopes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            let mut topic_ids = [source_topic_id, target_topic_id];
            topic_ids.sort();
            let scopes = [
                format!("forum:category:{tenant_id}:{category_id}"),
                format!("forum:topic:{tenant_id}:{}", topic_ids[0]),
                format!("forum:topic:{tenant_id}:{}", topic_ids[1]),
            ];
            for scope in scopes {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT forum_counter_lock($1)",
                    vec![scope.into()],
                ))
                .await?;
            }
        }
        DatabaseBackend::Sqlite => {}
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic merge does not support database backend {backend:?}"
            )));
        }
    }
    Ok(())
}

async fn lock_topics_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
) -> ForumResult<()> {
    let mut ids = [source_topic_id, target_topic_id];
    ids.sort();
    for topic_id in ids {
        let statement = match txn.get_database_backend() {
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT id FROM forum_topics WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE",
                vec![tenant_id.into(), topic_id.into()],
            ),
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "SELECT id FROM forum_topics WHERE tenant_id = ? AND id = ? AND deleted_at IS NULL",
                vec![tenant_id.into(), topic_id.into()],
            ),
            backend => {
                return Err(ForumError::Validation(format!(
                    "Forum topic merge does not support database backend {backend:?}"
                )));
            }
        };
        if txn.query_one(statement).await?.is_none() {
            return Err(ForumError::TopicNotFound(topic_id));
        }
    }
    Ok(())
}

async fn lock_topic_solution_scopes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_ids: &[Uuid],
) -> ForumResult<()> {
    let mut ids = topic_ids.to_vec();
    ids.sort();
    ids.dedup();
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            for topic_id in ids {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT pg_advisory_xact_lock(hashtextextended($1, 31))",
                    vec![format!("{tenant_id}:{topic_id}").into()],
                ))
                .await?;
            }
        }
        DatabaseBackend::Sqlite => {
            for topic_id in ids {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    r#"
                    INSERT INTO forum_topic_solution_locks (tenant_id, topic_id, touched_at)
                    VALUES (?, ?, CURRENT_TIMESTAMP)
                    ON CONFLICT(tenant_id, topic_id)
                    DO UPDATE SET touched_at = CURRENT_TIMESTAMP
                    "#,
                    vec![tenant_id.into(), topic_id.into()],
                ))
                .await?;
            }
        }
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic merge solution policy does not support database backend {backend:?}"
            )));
        }
    }
    Ok(())
}

async fn find_topic_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<forum_topic::Model> {
    forum_topic::Entity::find_by_id(topic_id)
        .filter(forum_topic::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(ForumError::TopicNotFound(topic_id))
}

async fn ensure_category_active_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<()> {
    if forum_category_lifecycle::Entity::find()
        .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
        .filter(forum_category_lifecycle::Column::CategoryId.eq(category_id))
        .one(txn)
        .await?
        .is_some()
    {
        return Err(ForumError::Validation(
            "Forum topic merge requires an active category".to_string(),
        ));
    }
    Ok(())
}

async fn load_valid_solution_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    label: &str,
) -> ForumResult<Option<forum_solution::Model>> {
    let solution = forum_solution::Entity::find()
        .filter(forum_solution::Column::TenantId.eq(tenant_id))
        .filter(forum_solution::Column::TopicId.eq(topic_id))
        .one(txn)
        .await?;
    let Some(solution) = solution else {
        return Ok(None);
    };
    let reply = forum_reply::Entity::find_by_id(solution.reply_id)
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(topic_id))
        .one(txn)
        .await?;
    if !reply.is_some_and(|reply| reply.status == ReplyStatus::Approved) {
        return Err(ForumError::Validation(format!(
            "Forum topic merge requires a valid approved {label} solution"
        )));
    }
    Ok(Some(solution))
}

async fn delete_source_solution_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
) -> ForumResult<()> {
    let result = forum_solution::Entity::delete_many()
        .filter(forum_solution::Column::TenantId.eq(tenant_id))
        .filter(forum_solution::Column::TopicId.eq(source_topic_id))
        .exec(txn)
        .await?;
    if result.rows_affected != 1 {
        return Err(ForumError::Validation(
            "Forum source accepted solution changed concurrently".to_string(),
        ));
    }
    Ok(())
}

async fn insert_transferred_solution_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    target_topic_id: Uuid,
    solution: &ForumTopicMergeSolutionTransfer,
) -> ForumResult<()> {
    forum_solution::ActiveModel {
        topic_id: Set(target_topic_id),
        tenant_id: Set(tenant_id),
        reply_id: Set(solution.reply_id),
        marked_by_user_id: Set(solution.marked_by_user_id),
        marked_at: Set(solution.marked_at),
    }
    .insert(txn)
    .await?;
    Ok(())
}

async fn approved_reply_count_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<i32> {
    let count = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(topic_id))
        .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
        .count(txn)
        .await?;
    i32::try_from(count).map_err(|_| {
        ForumError::Validation("Forum published reply count exceeds supported range".to_string())
    })
}

async fn max_reply_position_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<i64> {
    Ok(forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(topic_id))
        .order_by_desc(forum_reply::Column::Position)
        .one(txn)
        .await?
        .map_or(0, |reply| reply.position))
}

async fn move_replies_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    position_offset: i64,
    expected_rows: u64,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            UPDATE forum_replies
               SET topic_id = $1,
                   position = position + $2,
                   updated_at = CURRENT_TIMESTAMP
             WHERE tenant_id = $3
               AND topic_id = $4
            "#,
            vec![
                target_topic_id.into(),
                position_offset.into(),
                tenant_id.into(),
                source_topic_id.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            UPDATE forum_replies
               SET topic_id = ?,
                   position = position + ?,
                   updated_at = CURRENT_TIMESTAMP
             WHERE tenant_id = ?
               AND topic_id = ?
            "#,
            vec![
                target_topic_id.into(),
                position_offset.into(),
                tenant_id.into(),
                source_topic_id.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic merge does not support database backend {backend:?}"
            )));
        }
    };
    let result = txn.execute(statement).await?;
    if result.rows_affected() != expected_rows {
        return Err(ForumError::Validation(
            "Forum topic merge reply set changed concurrently".to_string(),
        ));
    }
    Ok(())
}

fn topic_merged_payload(
    operation_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    category_id: Uuid,
    moved_reply_count: i32,
    moved_published_reply_count: i32,
    resulting_published_reply_count: i32,
    position_offset: i64,
    reason: &str,
) -> JsonValue {
    json!({
        "operation_id": operation_id,
        "source_topic_id": source_topic_id,
        "target_topic_id": target_topic_id,
        "category_id": category_id,
        "moved_reply_count": moved_reply_count,
        "moved_published_reply_count": moved_published_reply_count,
        "resulting_published_reply_count": resulting_published_reply_count,
        "position_offset": position_offset,
        "reason": reason,
    })
}

async fn validate_existing_semantic_event_in_tx(
    txn: &DatabaseTransaction,
    operation: &forum_topic_merge_operation::Model,
) -> ForumResult<()> {
    let event = forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::TenantId.eq(operation.tenant_id))
        .filter(forum_domain_event::Column::EventId.eq(operation.event_id))
        .one(txn)
        .await?
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum topic merge operation is missing its semantic event".to_string(),
            )
        })?;
    let expected_payload = topic_merged_payload(
        operation.operation_id,
        operation.source_topic_id,
        operation.target_topic_id,
        operation.category_id,
        operation.moved_reply_count,
        operation.moved_published_reply_count,
        operation.resulting_published_reply_count,
        operation.position_offset,
        &operation.reason,
    );
    if event.aggregate_type != FORUM_TOPIC_MERGED_AGGREGATE_TYPE
        || event.aggregate_id != operation.target_topic_id
        || event.event_type != FORUM_TOPIC_MERGED_EVENT_TYPE
        || event.schema_version != 1
        || event.actor_id != Some(operation.actor_id)
        || event.payload != expected_payload
    {
        return Err(ForumError::Validation(
            "Forum topic merge operation semantic event does not match its receipt".to_string(),
        ));
    }
    Ok(())
}

fn operation_to_result(operation: forum_topic_merge_operation::Model) -> ForumTopicMergeResult {
    ForumTopicMergeResult {
        operation_id: operation.operation_id,
        event_id: operation.event_id,
        source_topic_id: operation.source_topic_id,
        target_topic_id: operation.target_topic_id,
        category_id: operation.category_id,
        actor_id: operation.actor_id,
        reason: operation.reason,
        moved_reply_count: operation.moved_reply_count,
        moved_published_reply_count: operation.moved_published_reply_count,
        resulting_published_reply_count: operation.resulting_published_reply_count,
        position_offset: operation.position_offset,
        merged_at: operation.merged_at.with_timezone(&Utc),
    }
}
