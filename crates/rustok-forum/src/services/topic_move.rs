use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::entities::{
    forum_category, forum_category_lifecycle, forum_domain_event, forum_reply, forum_solution,
    forum_topic, forum_topic_move_operation,
};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::{ReplyStatus, TopicStatus};

use super::category_audience::load_category_audience_policy;
use super::projection_invalidation::{
    publish_forum_category_projection_in_tx, publish_forum_topic_projection_in_tx,
};
use super::rbac::enforce_scope;
use super::topic_audience::load_policy_for_topic;

pub const MAX_FORUM_TOPIC_MOVE_REASON_LEN: usize = 500;
const FORUM_TOPIC_MOVED_EVENT_TYPE: &str = "forum.topic.moved";
const FORUM_TOPIC_MOVED_AGGREGATE_TYPE: &str = "forum_topic";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveForumTopicInput {
    pub operation_id: Uuid,
    pub target_category_id: Uuid,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumTopicMoveResult {
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub topic_id: Uuid,
    pub source_category_id: Uuid,
    pub target_category_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub published_reply_count: i32,
    pub moved_at: DateTime<Utc>,
}

/// Idempotent owner command for moving one active topic between active categories.
///
/// The command serializes all topic moves for one tenant, transfers category
/// counters, revalidates the current solution and effective audience structure,
/// appends one immutable operation receipt and one Forum-local semantic event,
/// then publishes topic/source/target projection invalidations in the same
/// transaction. Reusing the same operation ID with the same command returns the
/// original result; any payload or actor drift fails closed.
pub struct ForumTopicMoveService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ForumTopicMoveService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, security, input))]
    pub async fn move_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: MoveForumTopicInput,
    ) -> ForumResult<ForumTopicMoveResult> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        let actor_id = security.user_id.ok_or_else(|| {
            ForumError::Validation("Forum topic move requires a human actor".to_string())
        })?;
        let reason = validate_move_input(tenant_id, topic_id, actor_id, &input)?;

        let txn = self.db.begin().await?;
        lock_topic_move_tenant_in_tx(&txn, tenant_id).await?;

        if let Some(existing) =
            forum_topic_move_operation::Entity::find_by_id((tenant_id, input.operation_id))
                .one(&txn)
                .await?
        {
            if existing.topic_id != topic_id
                || existing.target_category_id != input.target_category_id
                || existing.actor_id != actor_id
                || existing.reason != reason
            {
                return Err(ForumError::TopicMoveOperationConflict(input.operation_id));
            }
            validate_existing_semantic_event_in_tx(&txn, &existing).await?;
            txn.commit().await?;
            return Ok(operation_to_result(existing));
        }

        lock_topic_in_tx(&txn, tenant_id, topic_id).await?;
        let topic = forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))?;
        if topic.status == TopicStatus::Archived {
            return Err(ForumError::TopicArchived);
        }
        if topic.category_id == input.target_category_id {
            return Err(ForumError::Validation(
                "Forum topic is already assigned to the target category".to_string(),
            ));
        }
        if topic.reply_count < 0 {
            return Err(ForumError::Validation(
                "Forum topic contains an invalid published reply count".to_string(),
            ));
        }

        lock_category_in_tx(&txn, tenant_id, topic.category_id).await?;
        lock_category_in_tx(&txn, tenant_id, input.target_category_id).await?;
        ensure_category_active_in_tx(&txn, tenant_id, topic.category_id).await?;
        ensure_category_active_in_tx(&txn, tenant_id, input.target_category_id).await?;
        validate_solution_in_tx(&txn, tenant_id, topic_id).await?;
        load_category_audience_policy(&txn, tenant_id, input.target_category_id).await?;

        let now = Utc::now();
        let source_category_id = topic.category_id;
        let mut active: forum_topic::ActiveModel = topic.into();
        active.category_id = Set(input.target_category_id);
        active.updated_at = Set(now.into());
        let moved_topic = active.update(&txn).await?;
        load_policy_for_topic(&txn, tenant_id, &moved_topic).await?;

        transfer_category_counters_in_tx(
            &txn,
            tenant_id,
            source_category_id,
            input.target_category_id,
            moved_topic.reply_count,
            now,
        )
        .await?;

        let event_payload = topic_moved_payload(
            input.operation_id,
            topic_id,
            source_category_id,
            input.target_category_id,
            moved_topic.reply_count,
            &reason,
        );
        forum_domain_event::ActiveModel {
            sequence_no: NotSet,
            event_id: Set(input.operation_id),
            tenant_id: Set(tenant_id),
            aggregate_type: Set(FORUM_TOPIC_MOVED_AGGREGATE_TYPE.to_string()),
            aggregate_id: Set(topic_id),
            event_type: Set(FORUM_TOPIC_MOVED_EVENT_TYPE.to_string()),
            schema_version: Set(1),
            actor_id: Set(Some(actor_id)),
            payload: Set(event_payload),
            created_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        let operation = forum_topic_move_operation::ActiveModel {
            tenant_id: Set(tenant_id),
            operation_id: Set(input.operation_id),
            topic_id: Set(topic_id),
            source_category_id: Set(source_category_id),
            target_category_id: Set(input.target_category_id),
            actor_id: Set(actor_id),
            reason: Set(reason),
            published_reply_count: Set(moved_topic.reply_count),
            event_id: Set(input.operation_id),
            moved_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            Some(actor_id),
            topic_id,
        )
        .await?;
        publish_forum_category_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            Some(actor_id),
            source_category_id,
        )
        .await?;
        publish_forum_category_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            Some(actor_id),
            input.target_category_id,
        )
        .await?;

        txn.commit().await?;
        Ok(operation_to_result(operation))
    }
}

fn validate_move_input(
    tenant_id: Uuid,
    topic_id: Uuid,
    actor_id: Uuid,
    input: &MoveForumTopicInput,
) -> ForumResult<String> {
    for (label, value) in [
        ("tenant", tenant_id),
        ("topic", topic_id),
        ("operation", input.operation_id),
        ("target category", input.target_category_id),
        ("actor", actor_id),
    ] {
        if value.is_nil() {
            return Err(ForumError::Validation(format!(
                "Forum topic move {label} must not be nil"
            )));
        }
    }
    let reason = input.reason.trim();
    if reason.is_empty() {
        return Err(ForumError::Validation(
            "Forum topic move reason must not be empty".to_string(),
        ));
    }
    if reason.chars().count() > MAX_FORUM_TOPIC_MOVE_REASON_LEN {
        return Err(ForumError::Validation(format!(
            "Forum topic move reason must not exceed {MAX_FORUM_TOPIC_MOVE_REASON_LEN} characters"
        )));
    }
    if reason.chars().any(char::is_control) {
        return Err(ForumError::Validation(
            "Forum topic move reason must not contain control characters".to_string(),
        ));
    }
    Ok(reason.to_string())
}

async fn lock_topic_move_tenant_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 21))",
                vec![format!("forum-topic-move:{tenant_id}").into()],
            ))
            .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"
                INSERT INTO forum_topic_move_locks (tenant_id, touched_at)
                VALUES (?, CURRENT_TIMESTAMP)
                ON CONFLICT(tenant_id) DO UPDATE SET touched_at = CURRENT_TIMESTAMP
                "#,
                vec![tenant_id.into()],
            ))
            .await?;
            Ok(())
        }
        backend => Err(ForumError::Validation(format!(
            "Forum topic move does not support database backend {backend:?}"
        ))),
    }
}

async fn lock_topic_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
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
                "Forum topic move does not support database backend {backend:?}"
            )));
        }
    };
    if txn.query_one(statement).await?.is_none() {
        return Err(ForumError::TopicNotFound(topic_id));
    }
    Ok(())
}

async fn lock_category_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT id FROM forum_categories WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
            vec![tenant_id.into(), category_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT id FROM forum_categories WHERE tenant_id = ? AND id = ?",
            vec![tenant_id.into(), category_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic move does not support database backend {backend:?}"
            )));
        }
    };
    if txn.query_one(statement).await?.is_none() {
        return Err(ForumError::CategoryNotFound(category_id));
    }
    Ok(())
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
            "Forum topic move requires active source and target categories".to_string(),
        ));
    }
    Ok(())
}

async fn validate_solution_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    let Some(solution) = forum_solution::Entity::find()
        .filter(forum_solution::Column::TenantId.eq(tenant_id))
        .filter(forum_solution::Column::TopicId.eq(topic_id))
        .one(txn)
        .await?
    else {
        return Ok(());
    };
    let reply = forum_reply::Entity::find_by_id(solution.reply_id)
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(topic_id))
        .one(txn)
        .await?;
    if !reply.is_some_and(|reply| reply.status == ReplyStatus::Approved) {
        return Err(ForumError::Validation(
            "Forum topic move requires a valid approved solution owned by the topic".to_string(),
        ));
    }
    Ok(())
}

async fn transfer_category_counters_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_category_id: Uuid,
    target_category_id: Uuid,
    published_reply_count: i32,
    now: DateTime<Utc>,
) -> ForumResult<()> {
    let source = forum_category::Entity::find_by_id(source_category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(ForumError::CategoryNotFound(source_category_id))?;
    let target = forum_category::Entity::find_by_id(target_category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(ForumError::CategoryNotFound(target_category_id))?;

    let source_topic_count = source.topic_count.checked_sub(1).ok_or_else(|| {
        ForumError::Validation("Forum source category topic counter is inconsistent".to_string())
    })?;
    let source_reply_count = source
        .reply_count
        .checked_sub(published_reply_count)
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum source category reply counter is inconsistent".to_string(),
            )
        })?;
    let target_topic_count = target.topic_count.checked_add(1).ok_or_else(|| {
        ForumError::Validation("Forum target category topic counter overflow".to_string())
    })?;
    let target_reply_count = target
        .reply_count
        .checked_add(published_reply_count)
        .ok_or_else(|| {
            ForumError::Validation("Forum target category reply counter overflow".to_string())
        })?;

    let mut source_active: forum_category::ActiveModel = source.into();
    source_active.topic_count = Set(source_topic_count);
    source_active.reply_count = Set(source_reply_count);
    source_active.updated_at = Set(now.into());
    source_active.update(txn).await?;

    let mut target_active: forum_category::ActiveModel = target.into();
    target_active.topic_count = Set(target_topic_count);
    target_active.reply_count = Set(target_reply_count);
    target_active.updated_at = Set(now.into());
    target_active.update(txn).await?;
    Ok(())
}

fn topic_moved_payload(
    operation_id: Uuid,
    topic_id: Uuid,
    source_category_id: Uuid,
    target_category_id: Uuid,
    published_reply_count: i32,
    reason: &str,
) -> JsonValue {
    json!({
        "operation_id": operation_id,
        "topic_id": topic_id,
        "source_category_id": source_category_id,
        "target_category_id": target_category_id,
        "published_reply_count": published_reply_count,
        "reason": reason,
    })
}

async fn validate_existing_semantic_event_in_tx(
    txn: &DatabaseTransaction,
    operation: &forum_topic_move_operation::Model,
) -> ForumResult<()> {
    let event = forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::TenantId.eq(operation.tenant_id))
        .filter(forum_domain_event::Column::EventId.eq(operation.event_id))
        .one(txn)
        .await?
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum topic move operation is missing its semantic event".to_string(),
            )
        })?;
    let expected_payload = topic_moved_payload(
        operation.operation_id,
        operation.topic_id,
        operation.source_category_id,
        operation.target_category_id,
        operation.published_reply_count,
        &operation.reason,
    );
    if event.aggregate_type != FORUM_TOPIC_MOVED_AGGREGATE_TYPE
        || event.aggregate_id != operation.topic_id
        || event.event_type != FORUM_TOPIC_MOVED_EVENT_TYPE
        || event.schema_version != 1
        || event.actor_id != Some(operation.actor_id)
        || event.payload != expected_payload
    {
        return Err(ForumError::Validation(
            "Forum topic move operation semantic event does not match its receipt".to_string(),
        ));
    }
    Ok(())
}

fn operation_to_result(operation: forum_topic_move_operation::Model) -> ForumTopicMoveResult {
    ForumTopicMoveResult {
        operation_id: operation.operation_id,
        event_id: operation.event_id,
        topic_id: operation.topic_id,
        source_category_id: operation.source_category_id,
        target_category_id: operation.target_category_id,
        actor_id: operation.actor_id,
        reason: operation.reason,
        published_reply_count: operation.published_reply_count,
        moved_at: operation.moved_at.with_timezone(&Utc),
    }
}
