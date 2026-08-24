use std::collections::HashSet;

use chrono::{DateTime, Utc};
use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QueryResult, QuerySelect, Statement,
    TransactionTrait,
    prelude::DateTimeWithTimeZone,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use tracing::instrument;
use uuid::Uuid;

use crate::entities::{
    forum_category, forum_category_lifecycle, forum_domain_event, forum_reply, forum_solution,
    forum_topic, forum_topic_channel_access,
};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::{ReplyStatus, TopicStatus};

use super::projection_invalidation::{
    publish_forum_category_projection_in_tx, publish_forum_topic_projection_in_tx,
};
use super::rbac::enforce_scope;
use super::topic_audience::load_policy_for_topic;
use super::topic_audience_lock::lock_topic_audience_scopes_in_tx;
use super::topic_reply_create_audience::load_topic_reply_create_audience_policy_for_topic;
use super::topic_solution_lock::lock_topic_solution_scopes_in_tx;

pub const MAX_FORUM_REPLY_RANGE_MOVE_REASON_LEN: usize = 500;
pub const MAX_FORUM_REPLY_RANGE_MOVE_REPLIES: usize = 500;
const FORUM_REPLY_RANGE_MOVE_EVENT_TYPE: &str = "forum.topic.reply_range_moved";
const FORUM_REPLY_RANGE_MOVE_AGGREGATE_TYPE: &str = "forum_topic";
const FORUM_REPLY_RANGE_MOVE_SCHEMA_VERSION: i16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveForumReplyRangeInput {
    pub operation_id: Uuid,
    pub target_topic_id: Uuid,
    pub start_position: i64,
    pub end_position: i64,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumReplyRangeMoveResult {
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub source_topic_id: Uuid,
    pub target_topic_id: Uuid,
    pub source_category_id: Uuid,
    pub target_category_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub source_start_position: i64,
    pub source_end_position: i64,
    pub target_start_position: i64,
    pub target_end_position: i64,
    pub moved_reply_count: i32,
    pub moved_published_reply_count: i32,
    pub source_resulting_published_reply_count: i32,
    pub target_resulting_published_reply_count: i32,
    pub moved_solution_reply_id: Option<Uuid>,
    pub source_resulting_solution_reply_id: Option<Uuid>,
    pub target_resulting_solution_reply_id: Option<Uuid>,
    pub moved_at: DateTime<Utc>,
}

struct PreparedRangeMoveInput {
    operation_id: Uuid,
    target_topic_id: Uuid,
    start_position: i64,
    end_position: i64,
    reason: String,
    command_fingerprint: String,
}

#[derive(Clone)]
struct SolutionCandidate {
    reply_id: Uuid,
    marked_by_user_id: Option<Uuid>,
    marked_at: DateTimeWithTimeZone,
}

struct RangeReplyAudit {
    reply_id: Uuid,
    source_parent_reply_id: Option<Uuid>,
    target_parent_reply_id: Option<Uuid>,
    source_position: i64,
    target_position: i64,
    was_published: bool,
}

struct StoredRangeMoveOperation {
    tenant_id: Uuid,
    operation_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    source_category_id: Uuid,
    target_category_id: Uuid,
    actor_id: Uuid,
    reason: String,
    command_fingerprint: String,
    source_start_position: i64,
    source_end_position: i64,
    target_start_position: i64,
    target_end_position: i64,
    moved_reply_count: i32,
    moved_published_reply_count: i32,
    source_resulting_published_reply_count: i32,
    target_resulting_published_reply_count: i32,
    moved_solution_reply_id: Option<Uuid>,
    source_resulting_solution_reply_id: Option<Uuid>,
    target_resulting_solution_reply_id: Option<Uuid>,
    event_id: Uuid,
    moved_at: DateTimeWithTimeZone,
}

/// Idempotently moves one bounded inclusive source-position range into an existing topic.
///
/// Every current reply whose source position is inside the exact occupied endpoints is selected.
/// The source keeps at least one reply. Selected roots whose parent is outside the range are
/// detached, internal parent edges are preserved, and leaving an unselected child behind is
/// forbidden. Reply IDs and all ID-owned bodies, revisions, mentions, quotes, votes and authorship
/// remain unchanged. Target positions append deterministically after the current target maximum.
/// Effective visibility, reply-create narrowing and channel access must be exactly equal. A selected
/// source solution follows its unchanged reply only when the target is unsolved. Exact replay returns
/// the immutable receipt; command or actor drift fails closed.
pub struct ForumReplyRangeMoveService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ForumReplyRangeMoveService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, security, input))]
    pub async fn move_reply_range(
        &self,
        tenant_id: Uuid,
        source_topic_id: Uuid,
        security: SecurityContext,
        input: MoveForumReplyRangeInput,
    ) -> ForumResult<ForumReplyRangeMoveResult> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        let actor_id = security.user_id.ok_or_else(|| {
            ForumError::Validation("Forum reply range move requires a human actor".to_string())
        })?;
        let prepared = prepare_input(tenant_id, source_topic_id, actor_id, input)?;

        let txn = self.db.begin().await?;
        lock_range_move_tenant_in_tx(&txn, tenant_id).await?;
        if let Some(existing) = load_operation_in_tx(&txn, tenant_id, prepared.operation_id).await?
        {
            validate_replay_in_tx(&txn, &existing, source_topic_id, actor_id, &prepared).await?;
            txn.commit().await?;
            return Ok(operation_to_result(existing));
        }

        let preliminary_source = find_active_topic_in_tx(&txn, tenant_id, source_topic_id).await?;
        let preliminary_target =
            find_active_topic_in_tx(&txn, tenant_id, prepared.target_topic_id).await?;
        lock_counter_scopes_in_tx(
            &txn,
            tenant_id,
            preliminary_source.category_id,
            preliminary_target.category_id,
            source_topic_id,
            prepared.target_topic_id,
        )
        .await?;
        lock_topic_rows_in_tx(
            &txn,
            tenant_id,
            &[source_topic_id, prepared.target_topic_id],
        )
        .await?;
        let source = find_active_topic_in_tx(&txn, tenant_id, source_topic_id).await?;
        let target = find_active_topic_in_tx(&txn, tenant_id, prepared.target_topic_id).await?;
        if source.category_id != preliminary_source.category_id
            || target.category_id != preliminary_target.category_id
        {
            return Err(ForumError::Validation(
                "Forum reply range move topic category changed concurrently".to_string(),
            ));
        }
        validate_topic_pair(&source, &target)?;
        ensure_category_active_in_tx(&txn, tenant_id, source.category_id).await?;
        ensure_category_active_in_tx(&txn, tenant_id, target.category_id).await?;

        let topic_ids = [source_topic_id, prepared.target_topic_id];
        lock_topic_audience_scopes_in_tx(&txn, tenant_id, &topic_ids).await?;
        lock_topic_reply_create_scopes_in_tx(&txn, tenant_id, &topic_ids).await?;
        lock_topic_solution_scopes_in_tx(&txn, tenant_id, &topic_ids).await?;
        validate_equal_access_in_tx(&txn, tenant_id, &source, &target).await?;

        let selected = load_range_replies_in_tx(
            &txn,
            tenant_id,
            source_topic_id,
            prepared.start_position,
            prepared.end_position,
        )
        .await?;
        let selected_ids = selected
            .iter()
            .map(|reply| reply.id)
            .collect::<HashSet<_>>();
        validate_parent_boundary_in_tx(&txn, tenant_id, source_topic_id, &selected, &selected_ids)
            .await?;

        let source_total = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(source_topic_id))
            .count(&txn)
            .await?;
        if source_total <= selected.len() as u64 {
            return Err(ForumError::Validation(
                "Forum reply range move must leave at least one reply in the source topic"
                    .to_string(),
            ));
        }

        let source_published_before =
            approved_reply_count_in_tx(&txn, tenant_id, source_topic_id).await?;
        let target_published_before =
            approved_reply_count_in_tx(&txn, tenant_id, prepared.target_topic_id).await?;
        if source_published_before != source.reply_count
            || target_published_before != target.reply_count
        {
            return Err(ForumError::Validation(
                "Forum reply range move topic counters are inconsistent".to_string(),
            ));
        }
        let moved_published_reply_count = i32::try_from(
            selected
                .iter()
                .filter(|reply| reply.status == ReplyStatus::Approved)
                .count(),
        )
        .map_err(|_| {
            ForumError::Validation(
                "Forum reply range move published reply count exceeds supported range".to_string(),
            )
        })?;
        let moved_reply_count = i32::try_from(selected.len()).map_err(|_| {
            ForumError::Validation(
                "Forum reply range move reply count exceeds supported range".to_string(),
            )
        })?;
        let source_resulting_published_reply_count = source_published_before
            .checked_sub(moved_published_reply_count)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum reply range move source published counter underflow".to_string(),
                )
            })?;
        let target_resulting_published_reply_count = target_published_before
            .checked_add(moved_published_reply_count)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum reply range move target published counter overflow".to_string(),
                )
            })?;

        let source_solution = load_valid_solution_in_tx(&txn, tenant_id, source_topic_id).await?;
        let target_solution =
            load_valid_solution_in_tx(&txn, tenant_id, prepared.target_topic_id).await?;
        let moved_solution = source_solution
            .as_ref()
            .filter(|solution| selected_ids.contains(&solution.reply_id))
            .cloned();
        if moved_solution.is_some() && target_solution.is_some() {
            return Err(ForumError::TopicReplyRangeMoveSolutionConflict(
                prepared.operation_id,
            ));
        }

        let target_max_position =
            maximum_reply_position_in_tx(&txn, tenant_id, prepared.target_topic_id).await?;
        let target_start_position = target_max_position.checked_add(1).ok_or_else(|| {
            ForumError::Validation("Forum reply range move target position overflow".to_string())
        })?;
        let now = Utc::now();
        let audit = move_replies_in_tx(
            &txn,
            tenant_id,
            prepared.target_topic_id,
            selected,
            &selected_ids,
            target_start_position,
            now,
        )
        .await?;
        let target_end_position =
            audit
                .last()
                .map(|item| item.target_position)
                .ok_or_else(|| {
                    ForumError::Validation("Forum reply range move audit is empty".to_string())
                })?;

        let actual_source_published =
            approved_reply_count_in_tx(&txn, tenant_id, source_topic_id).await?;
        let actual_target_published =
            approved_reply_count_in_tx(&txn, tenant_id, prepared.target_topic_id).await?;
        if actual_source_published != source_resulting_published_reply_count
            || actual_target_published != target_resulting_published_reply_count
        {
            return Err(ForumError::Validation(
                "Forum reply range move published counter reconciliation failed".to_string(),
            ));
        }

        let source_resulting_solution_reply_id = if moved_solution.is_some() {
            None
        } else {
            source_solution.as_ref().map(|solution| solution.reply_id)
        };
        let target_resulting_solution_reply_id = moved_solution
            .as_ref()
            .map(|solution| solution.reply_id)
            .or_else(|| target_solution.as_ref().map(|solution| solution.reply_id));

        if let Some(solution) = moved_solution.as_ref() {
            forum_solution::Entity::delete_many()
                .filter(forum_solution::Column::TenantId.eq(tenant_id))
                .filter(forum_solution::Column::TopicId.eq(source_topic_id))
                .exec(&txn)
                .await?;
            forum_solution::ActiveModel {
                topic_id: Set(prepared.target_topic_id),
                tenant_id: Set(tenant_id),
                reply_id: Set(solution.reply_id),
                marked_by_user_id: Set(solution.marked_by_user_id),
                marked_at: Set(solution.marked_at),
            }
            .insert(&txn)
            .await?;
        }

        validate_solution_state_in_tx(
            &txn,
            tenant_id,
            source_topic_id,
            prepared.target_topic_id,
            source_solution.as_ref(),
            target_solution.as_ref(),
            moved_solution.as_ref(),
        )
        .await?;

        update_topic_counters_in_tx(
            &txn,
            source,
            source_resulting_published_reply_count,
            last_approved_reply_at_in_tx(&txn, tenant_id, source_topic_id).await?,
            now,
        )
        .await?;
        update_topic_counters_in_tx(
            &txn,
            target,
            target_resulting_published_reply_count,
            last_approved_reply_at_in_tx(&txn, tenant_id, prepared.target_topic_id).await?,
            now,
        )
        .await?;
        reconcile_category_counters_in_tx(
            &txn,
            tenant_id,
            preliminary_source.category_id,
            preliminary_target.category_id,
            moved_published_reply_count,
            now,
        )
        .await?;

        let payload = event_payload(
            prepared.operation_id,
            source_topic_id,
            prepared.target_topic_id,
            preliminary_source.category_id,
            preliminary_target.category_id,
            actor_id,
            &prepared,
            target_start_position,
            target_end_position,
            moved_reply_count,
            moved_published_reply_count,
            source_resulting_published_reply_count,
            target_resulting_published_reply_count,
            moved_solution.as_ref().map(|solution| solution.reply_id),
            source_resulting_solution_reply_id,
            target_resulting_solution_reply_id,
        );
        forum_domain_event::ActiveModel {
            sequence_no: NotSet,
            event_id: Set(prepared.operation_id),
            tenant_id: Set(tenant_id),
            aggregate_type: Set(FORUM_REPLY_RANGE_MOVE_AGGREGATE_TYPE.to_string()),
            aggregate_id: Set(prepared.target_topic_id),
            event_type: Set(FORUM_REPLY_RANGE_MOVE_EVENT_TYPE.to_string()),
            schema_version: Set(FORUM_REPLY_RANGE_MOVE_SCHEMA_VERSION),
            actor_id: Set(Some(actor_id)),
            payload: Set(payload),
            created_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        insert_operation_in_tx(
            &txn,
            tenant_id,
            source_topic_id,
            preliminary_source.category_id,
            preliminary_target.category_id,
            actor_id,
            &prepared,
            target_start_position,
            target_end_position,
            moved_reply_count,
            moved_published_reply_count,
            source_resulting_published_reply_count,
            target_resulting_published_reply_count,
            moved_solution.as_ref().map(|solution| solution.reply_id),
            source_resulting_solution_reply_id,
            target_resulting_solution_reply_id,
            now,
        )
        .await?;
        insert_reply_audit_in_tx(&txn, tenant_id, prepared.operation_id, &audit, now).await?;

        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            Some(actor_id),
            source_topic_id,
        )
        .await?;
        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            Some(actor_id),
            prepared.target_topic_id,
        )
        .await?;
        publish_forum_category_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            Some(actor_id),
            preliminary_source.category_id,
        )
        .await?;
        if preliminary_target.category_id != preliminary_source.category_id {
            publish_forum_category_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                Some(actor_id),
                preliminary_target.category_id,
            )
            .await?;
        }

        txn.commit().await?;
        Ok(ForumReplyRangeMoveResult {
            operation_id: prepared.operation_id,
            event_id: prepared.operation_id,
            source_topic_id,
            target_topic_id: prepared.target_topic_id,
            source_category_id: preliminary_source.category_id,
            target_category_id: preliminary_target.category_id,
            actor_id,
            reason: prepared.reason,
            source_start_position: prepared.start_position,
            source_end_position: prepared.end_position,
            target_start_position,
            target_end_position,
            moved_reply_count,
            moved_published_reply_count,
            source_resulting_published_reply_count,
            target_resulting_published_reply_count,
            moved_solution_reply_id: moved_solution.map(|solution| solution.reply_id),
            source_resulting_solution_reply_id,
            target_resulting_solution_reply_id,
            moved_at: now,
        })
    }
}

fn prepare_input(
    tenant_id: Uuid,
    source_topic_id: Uuid,
    actor_id: Uuid,
    input: MoveForumReplyRangeInput,
) -> ForumResult<PreparedRangeMoveInput> {
    for (label, value) in [
        ("tenant", tenant_id),
        ("source topic", source_topic_id),
        ("target topic", input.target_topic_id),
        ("operation", input.operation_id),
        ("actor", actor_id),
    ] {
        if value.is_nil() {
            return Err(ForumError::Validation(format!(
                "Forum reply range move {label} must not be nil"
            )));
        }
    }
    if source_topic_id == input.target_topic_id {
        return Err(ForumError::Validation(
            "Forum reply range move source and target topics must differ".to_string(),
        ));
    }
    if input.start_position < 1
        || input.end_position < 1
        || input.start_position > input.end_position
    {
        return Err(ForumError::Validation(
            "Forum reply range move requires positive ordered inclusive positions".to_string(),
        ));
    }
    let reason = input.reason.trim().to_string();
    if reason.is_empty() {
        return Err(ForumError::Validation(
            "Forum reply range move reason must not be empty".to_string(),
        ));
    }
    if reason.chars().count() > MAX_FORUM_REPLY_RANGE_MOVE_REASON_LEN {
        return Err(ForumError::Validation(format!(
            "Forum reply range move reason must not exceed {MAX_FORUM_REPLY_RANGE_MOVE_REASON_LEN} characters"
        )));
    }
    if reason.chars().any(char::is_control) {
        return Err(ForumError::Validation(
            "Forum reply range move reason must not contain control characters".to_string(),
        ));
    }
    let command_fingerprint = fingerprint_command(
        source_topic_id,
        input.target_topic_id,
        input.start_position,
        input.end_position,
        &reason,
    )?;
    Ok(PreparedRangeMoveInput {
        operation_id: input.operation_id,
        target_topic_id: input.target_topic_id,
        start_position: input.start_position,
        end_position: input.end_position,
        reason,
        command_fingerprint,
    })
}

fn fingerprint_command(
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    start_position: i64,
    end_position: i64,
    reason: &str,
) -> ForumResult<String> {
    let canonical = serde_json::to_vec(&json!({
        "source_topic_id": source_topic_id,
        "target_topic_id": target_topic_id,
        "start_position": start_position,
        "end_position": end_position,
        "reason": reason,
        "selection_policy": "all_current_replies_in_occupied_inclusive_position_range",
        "incoming_parent_policy": "detach",
        "outgoing_child_policy": "reject",
        "quote_reference_policy": "preserve_immutable_ids",
        "target_position_policy": "append_after_current_max",
        "acl_policy": "exact_effective_match",
    }))
    .map_err(|error| {
        ForumError::Validation(format!(
            "Forum reply range move command cannot be canonicalized: {error}"
        ))
    })?;
    Ok(hex::encode(Sha256::digest(canonical)))
}

fn validate_topic_pair(
    source: &forum_topic::Model,
    target: &forum_topic::Model,
) -> ForumResult<()> {
    if source.status == TopicStatus::Archived {
        return Err(ForumError::TopicArchived);
    }
    if target.status == TopicStatus::Archived {
        return Err(ForumError::Validation(
            "Forum reply range move target topic is archived".to_string(),
        ));
    }
    if source.reply_count < 0 || target.reply_count < 0 {
        return Err(ForumError::Validation(
            "Forum reply range move requires non-negative topic counters".to_string(),
        ));
    }
    Ok(())
}

async fn lock_range_move_tenant_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            for (scope, seed) in [
                (format!("forum-reply-range-move:{tenant_id}"), 24_i32),
                (tenant_id.to_string(), 0_i32),
            ] {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT pg_advisory_xact_lock(hashtextextended($1, $2))",
                    vec![scope.into(), seed.into()],
                ))
                .await?;
            }
            Ok(())
        }
        DatabaseBackend::Sqlite => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"
                INSERT INTO forum_reply_range_move_locks (tenant_id, touched_at)
                VALUES (?, CURRENT_TIMESTAMP)
                ON CONFLICT(tenant_id) DO UPDATE SET touched_at = CURRENT_TIMESTAMP
                "#,
                vec![tenant_id.into()],
            ))
            .await?;
            Ok(())
        }
        backend => Err(ForumError::Validation(format!(
            "Forum reply range move does not support database backend {backend:?}"
        ))),
    }
}

async fn lock_counter_scopes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_category_id: Uuid,
    target_category_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            let mut category_ids = vec![source_category_id, target_category_id];
            category_ids.sort();
            category_ids.dedup();
            for category_id in category_ids {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT forum_counter_lock($1)",
                    vec![format!("forum:category:{tenant_id}:{category_id}").into()],
                ))
                .await?;
            }
            let mut topic_ids = vec![source_topic_id, target_topic_id];
            topic_ids.sort();
            for topic_id in topic_ids {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT forum_counter_lock($1)",
                    vec![format!("forum:topic:{tenant_id}:{topic_id}").into()],
                ))
                .await?;
            }
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum reply range move counter locking does not support {backend:?}"
        ))),
    }
}

async fn lock_topic_rows_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_ids: &[Uuid],
) -> ForumResult<()> {
    let mut ids = topic_ids.to_vec();
    ids.sort();
    ids.dedup();
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
                    "Forum reply range move topic locking does not support {backend:?}"
                )));
            }
        };
        if txn.query_one(statement).await?.is_none() {
            return Err(ForumError::TopicNotFound(topic_id));
        }
    }
    Ok(())
}

async fn find_active_topic_in_tx(
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
            "Forum reply range move requires active source and target categories".to_string(),
        ));
    }
    Ok(())
}

async fn lock_topic_reply_create_scopes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_ids: &[Uuid],
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            let mut ids = topic_ids.to_vec();
            ids.sort();
            ids.dedup();
            for topic_id in ids {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT pg_advisory_xact_lock(hashtextextended($1, 5))",
                    vec![format!("{tenant_id}:{topic_id}:reply-create").into()],
                ))
                .await?;
            }
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum reply range move reply-create locking does not support {backend:?}"
        ))),
    }
}

async fn validate_equal_access_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source: &forum_topic::Model,
    target: &forum_topic::Model,
) -> ForumResult<()> {
    let source_visibility = load_policy_for_topic(txn, tenant_id, source).await?;
    let target_visibility = load_policy_for_topic(txn, tenant_id, target).await?;
    if source_visibility.inherited_category_layers != target_visibility.inherited_category_layers
        || source_visibility.configured_constraints != target_visibility.configured_constraints
    {
        return Err(ForumError::Validation(
            "Forum reply range move requires exactly equal effective visibility policy".to_string(),
        ));
    }
    let source_reply_create =
        load_topic_reply_create_audience_policy_for_topic(txn, tenant_id, source).await?;
    let target_reply_create =
        load_topic_reply_create_audience_policy_for_topic(txn, tenant_id, target).await?;
    if source_reply_create.inherited_category_layers
        != target_reply_create.inherited_category_layers
        || source_reply_create.configured_constraints != target_reply_create.configured_constraints
    {
        return Err(ForumError::Validation(
            "Forum reply range move requires exactly equal effective reply-create policy"
                .to_string(),
        ));
    }
    let source_channels = load_topic_channels_in_tx(txn, tenant_id, source.id).await?;
    let target_channels = load_topic_channels_in_tx(txn, tenant_id, target.id).await?;
    if source_channels != target_channels {
        return Err(ForumError::Validation(
            "Forum reply range move requires exactly equal topic channel access".to_string(),
        ));
    }
    Ok(())
}

async fn load_topic_channels_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Vec<String>> {
    let mut channels = forum_topic_channel_access::Entity::find()
        .filter(forum_topic_channel_access::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_channel_access::Column::TopicId.eq(topic_id))
        .all(txn)
        .await?
        .into_iter()
        .map(|row| row.channel_slug)
        .collect::<Vec<_>>();
    channels.sort();
    Ok(channels)
}

async fn load_range_replies_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    start_position: i64,
    end_position: i64,
) -> ForumResult<Vec<forum_reply::Model>> {
    let replies = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(source_topic_id))
        .filter(forum_reply::Column::Position.gte(start_position))
        .filter(forum_reply::Column::Position.lte(end_position))
        .order_by_asc(forum_reply::Column::Position)
        .limit((MAX_FORUM_REPLY_RANGE_MOVE_REPLIES + 1) as u64)
        .all(txn)
        .await?;
    if replies.is_empty() {
        return Err(ForumError::Validation(
            "Forum reply range move selected no replies".to_string(),
        ));
    }
    if replies.len() > MAX_FORUM_REPLY_RANGE_MOVE_REPLIES {
        return Err(ForumError::Validation(format!(
            "Forum reply range move must not exceed {MAX_FORUM_REPLY_RANGE_MOVE_REPLIES} replies"
        )));
    }
    if replies
        .first()
        .is_none_or(|reply| reply.position != start_position)
        || replies
            .last()
            .is_none_or(|reply| reply.position != end_position)
    {
        return Err(ForumError::Validation(
            "Forum reply range move endpoints must identify occupied source positions".to_string(),
        ));
    }
    Ok(replies)
}

async fn validate_parent_boundary_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    selected: &[forum_reply::Model],
    selected_ids: &HashSet<Uuid>,
) -> ForumResult<()> {
    for reply in selected {
        if let Some(parent_id) = reply
            .parent_reply_id
            .filter(|parent_id| selected_ids.contains(parent_id))
        {
            let parent = selected
                .iter()
                .find(|candidate| candidate.id == parent_id)
                .ok_or_else(|| {
                    ForumError::Validation(
                        "Forum reply range move internal parent is unavailable".to_string(),
                    )
                })?;
            if parent.position >= reply.position {
                return Err(ForumError::Validation(
                    "Forum reply range move requires parent-before-child source positions"
                        .to_string(),
                ));
            }
        }
    }
    let selected_vec = selected_ids.iter().copied().collect::<Vec<_>>();
    let crossing_child = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(source_topic_id))
        .filter(forum_reply::Column::ParentReplyId.is_in(selected_vec.clone()))
        .filter(forum_reply::Column::Id.is_not_in(selected_vec))
        .one(txn)
        .await?;
    if crossing_child.is_some() {
        return Err(ForumError::Validation(
            "Forum reply range move cannot leave a child behind its moved parent".to_string(),
        ));
    }
    Ok(())
}

async fn load_valid_solution_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Option<SolutionCandidate>> {
    let Some(solution) = forum_solution::Entity::find()
        .filter(forum_solution::Column::TenantId.eq(tenant_id))
        .filter(forum_solution::Column::TopicId.eq(topic_id))
        .one(txn)
        .await?
    else {
        return Ok(None);
    };
    let reply = forum_reply::Entity::find_by_id(solution.reply_id)
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(topic_id))
        .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
        .one(txn)
        .await?;
    if reply.is_none() {
        return Err(ForumError::Validation(
            "Forum reply range move found an invalid accepted solution".to_string(),
        ));
    }
    Ok(Some(SolutionCandidate {
        reply_id: solution.reply_id,
        marked_by_user_id: solution.marked_by_user_id,
        marked_at: solution.marked_at,
    }))
}

async fn maximum_reply_position_in_tx(
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
        .map(|reply| reply.position)
        .unwrap_or(0))
}

async fn move_replies_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    target_topic_id: Uuid,
    selected: Vec<forum_reply::Model>,
    selected_ids: &HashSet<Uuid>,
    target_start_position: i64,
    now: DateTime<Utc>,
) -> ForumResult<Vec<RangeReplyAudit>> {
    let mut audit = Vec::with_capacity(selected.len());
    for (index, reply) in selected.into_iter().enumerate() {
        let offset = i64::try_from(index).map_err(|_| {
            ForumError::Validation(
                "Forum reply range move target offset exceeds supported range".to_string(),
            )
        })?;
        let target_position = target_start_position.checked_add(offset).ok_or_else(|| {
            ForumError::Validation(
                "Forum reply range move target position exceeds supported range".to_string(),
            )
        })?;
        let target_parent_reply_id = reply
            .parent_reply_id
            .filter(|parent_id| selected_ids.contains(parent_id));
        audit.push(RangeReplyAudit {
            reply_id: reply.id,
            source_parent_reply_id: reply.parent_reply_id,
            target_parent_reply_id,
            source_position: reply.position,
            target_position,
            was_published: reply.status == ReplyStatus::Approved,
        });
        let mut active: forum_reply::ActiveModel = reply.into();
        active.topic_id = Set(target_topic_id);
        active.parent_reply_id = Set(target_parent_reply_id);
        active.position = Set(target_position);
        active.updated_at = Set(now.into());
        active.update(txn).await?;
    }
    let moved_ids = audit.iter().map(|item| item.reply_id).collect::<Vec<_>>();
    let target_count = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(target_topic_id))
        .filter(forum_reply::Column::Id.is_in(moved_ids.clone()))
        .count(txn)
        .await?;
    let source_count = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::Id.is_in(moved_ids))
        .filter(forum_reply::Column::TopicId.ne(target_topic_id))
        .count(txn)
        .await?;
    if target_count != audit.len() as u64 || source_count != 0 {
        return Err(ForumError::Validation(
            "Forum reply range move was partial".to_string(),
        ));
    }
    Ok(audit)
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
        ForumError::Validation(
            "Forum reply range move published reply count exceeds supported range".to_string(),
        )
    })
}

async fn last_approved_reply_at_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Option<DateTimeWithTimeZone>> {
    Ok(forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(topic_id))
        .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
        .order_by_desc(forum_reply::Column::CreatedAt)
        .one(txn)
        .await?
        .map(|reply| reply.created_at))
}

async fn validate_solution_state_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    source_before: Option<&SolutionCandidate>,
    target_before: Option<&SolutionCandidate>,
    moved: Option<&SolutionCandidate>,
) -> ForumResult<()> {
    let source_after = forum_solution::Entity::find_by_id((source_topic_id, tenant_id))
        .one(txn)
        .await?;
    let target_after = forum_solution::Entity::find_by_id((target_topic_id, tenant_id))
        .one(txn)
        .await?;
    let matches_candidate = |row: &forum_solution::Model, expected: &SolutionCandidate| {
        row.reply_id == expected.reply_id
            && row.marked_by_user_id == expected.marked_by_user_id
            && row.marked_at == expected.marked_at
    };
    match moved {
        Some(expected)
            if source_after.is_none()
                && target_after
                    .as_ref()
                    .is_some_and(|row| matches_candidate(row, expected)) =>
        {
            Ok(())
        }
        None => {
            let source_ok = match (source_before, source_after.as_ref()) {
                (None, None) => true,
                (Some(expected), Some(row)) => matches_candidate(row, expected),
                _ => false,
            };
            let target_ok = match (target_before, target_after.as_ref()) {
                (None, None) => true,
                (Some(expected), Some(row)) => matches_candidate(row, expected),
                _ => false,
            };
            if source_ok && target_ok {
                Ok(())
            } else {
                Err(ForumError::Validation(
                    "Forum reply range move accepted solution changed unexpectedly".to_string(),
                ))
            }
        }
        _ => Err(ForumError::Validation(
            "Forum reply range move accepted solution cascade is inconsistent".to_string(),
        )),
    }
}

async fn update_topic_counters_in_tx(
    txn: &DatabaseTransaction,
    topic: forum_topic::Model,
    reply_count: i32,
    last_reply_at: Option<DateTimeWithTimeZone>,
    now: DateTime<Utc>,
) -> ForumResult<()> {
    let mut active: forum_topic::ActiveModel = topic.into();
    active.reply_count = Set(reply_count);
    active.last_reply_at = Set(last_reply_at);
    active.updated_at = Set(now.into());
    active.update(txn).await?;
    Ok(())
}

async fn reconcile_category_counters_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_category_id: Uuid,
    target_category_id: Uuid,
    moved_published_reply_count: i32,
    now: DateTime<Utc>,
) -> ForumResult<()> {
    if source_category_id == target_category_id {
        return Ok(());
    }
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
    if source.reply_count < moved_published_reply_count || target.reply_count < 0 {
        return Err(ForumError::Validation(
            "Forum reply range move category counters are inconsistent".to_string(),
        ));
    }
    let source_count = source
        .reply_count
        .checked_sub(moved_published_reply_count)
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum reply range move source category counter underflow".to_string(),
            )
        })?;
    let target_count = target
        .reply_count
        .checked_add(moved_published_reply_count)
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum reply range move target category counter overflow".to_string(),
            )
        })?;
    let mut source_active: forum_category::ActiveModel = source.into();
    source_active.reply_count = Set(source_count);
    source_active.updated_at = Set(now.into());
    source_active.update(txn).await?;
    let mut target_active: forum_category::ActiveModel = target.into();
    target_active.reply_count = Set(target_count);
    target_active.updated_at = Set(now.into());
    target_active.update(txn).await?;
    Ok(())
}

fn event_payload(
    operation_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    source_category_id: Uuid,
    target_category_id: Uuid,
    actor_id: Uuid,
    prepared: &PreparedRangeMoveInput,
    target_start_position: i64,
    target_end_position: i64,
    moved_reply_count: i32,
    moved_published_reply_count: i32,
    source_resulting_published_reply_count: i32,
    target_resulting_published_reply_count: i32,
    moved_solution_reply_id: Option<Uuid>,
    source_resulting_solution_reply_id: Option<Uuid>,
    target_resulting_solution_reply_id: Option<Uuid>,
) -> JsonValue {
    json!({
        "operation_id": operation_id,
        "source_topic_id": source_topic_id,
        "target_topic_id": target_topic_id,
        "source_category_id": source_category_id,
        "target_category_id": target_category_id,
        "actor_id": actor_id,
        "reason": prepared.reason,
        "command_fingerprint": prepared.command_fingerprint,
        "source_start_position": prepared.start_position,
        "source_end_position": prepared.end_position,
        "target_start_position": target_start_position,
        "target_end_position": target_end_position,
        "moved_reply_count": moved_reply_count,
        "moved_published_reply_count": moved_published_reply_count,
        "source_resulting_published_reply_count": source_resulting_published_reply_count,
        "target_resulting_published_reply_count": target_resulting_published_reply_count,
        "moved_solution_reply_id": moved_solution_reply_id,
        "source_resulting_solution_reply_id": source_resulting_solution_reply_id,
        "target_resulting_solution_reply_id": target_resulting_solution_reply_id,
        "parent_policy": "detach_incoming_reject_outgoing_preserve_internal",
        "reference_policy": "preserve_reply_revision_mention_quote_and_vote_identity",
    })
}

#[allow(clippy::too_many_arguments)]
async fn insert_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    source_category_id: Uuid,
    target_category_id: Uuid,
    actor_id: Uuid,
    prepared: &PreparedRangeMoveInput,
    target_start_position: i64,
    target_end_position: i64,
    moved_reply_count: i32,
    moved_published_reply_count: i32,
    source_resulting_published_reply_count: i32,
    target_resulting_published_reply_count: i32,
    moved_solution_reply_id: Option<Uuid>,
    source_resulting_solution_reply_id: Option<Uuid>,
    target_resulting_solution_reply_id: Option<Uuid>,
    now: DateTime<Utc>,
) -> ForumResult<()> {
    let (sql, backend) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            r#"
            INSERT INTO forum_reply_range_move_operations (
                tenant_id, operation_id, source_topic_id, target_topic_id,
                source_category_id, target_category_id, actor_id, reason,
                command_fingerprint, source_start_position, source_end_position,
                target_start_position, target_end_position, moved_reply_count,
                moved_published_reply_count, source_resulting_published_reply_count,
                target_resulting_published_reply_count, moved_solution_reply_id,
                source_resulting_solution_reply_id, target_resulting_solution_reply_id,
                event_id, moved_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22
            )
            "#,
            DatabaseBackend::Postgres,
        ),
        DatabaseBackend::Sqlite => (
            r#"
            INSERT INTO forum_reply_range_move_operations (
                tenant_id, operation_id, source_topic_id, target_topic_id,
                source_category_id, target_category_id, actor_id, reason,
                command_fingerprint, source_start_position, source_end_position,
                target_start_position, target_end_position, moved_reply_count,
                moved_published_reply_count, source_resulting_published_reply_count,
                target_resulting_published_reply_count, moved_solution_reply_id,
                source_resulting_solution_reply_id, target_resulting_solution_reply_id,
                event_id, moved_at
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
            )
            "#,
            DatabaseBackend::Sqlite,
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply range move receipt does not support {backend:?}"
            )));
        }
    };
    txn.execute(Statement::from_sql_and_values(
        backend,
        sql,
        vec![
            tenant_id.into(),
            prepared.operation_id.into(),
            source_topic_id.into(),
            prepared.target_topic_id.into(),
            source_category_id.into(),
            target_category_id.into(),
            actor_id.into(),
            prepared.reason.clone().into(),
            prepared.command_fingerprint.clone().into(),
            prepared.start_position.into(),
            prepared.end_position.into(),
            target_start_position.into(),
            target_end_position.into(),
            moved_reply_count.into(),
            moved_published_reply_count.into(),
            source_resulting_published_reply_count.into(),
            target_resulting_published_reply_count.into(),
            moved_solution_reply_id.into(),
            source_resulting_solution_reply_id.into(),
            target_resulting_solution_reply_id.into(),
            prepared.operation_id.into(),
            now.into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn insert_reply_audit_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_id: Uuid,
    audit: &[RangeReplyAudit],
    now: DateTime<Utc>,
) -> ForumResult<()> {
    let (sql, backend) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            r#"
            INSERT INTO forum_reply_range_move_items (
                tenant_id, operation_id, reply_id, source_parent_reply_id,
                target_parent_reply_id, source_position, target_position,
                was_published, moved_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            DatabaseBackend::Postgres,
        ),
        DatabaseBackend::Sqlite => (
            r#"
            INSERT INTO forum_reply_range_move_items (
                tenant_id, operation_id, reply_id, source_parent_reply_id,
                target_parent_reply_id, source_position, target_position,
                was_published, moved_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            DatabaseBackend::Sqlite,
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply range move audit does not support {backend:?}"
            )));
        }
    };
    for item in audit {
        txn.execute(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                tenant_id.into(),
                operation_id.into(),
                item.reply_id.into(),
                item.source_parent_reply_id.into(),
                item.target_parent_reply_id.into(),
                item.source_position.into(),
                item.target_position.into(),
                item.was_published.into(),
                now.into(),
            ],
        ))
        .await?;
    }
    Ok(())
}

async fn load_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> ForumResult<Option<StoredRangeMoveOperation>> {
    let (sql, backend) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            "SELECT * FROM forum_reply_range_move_operations WHERE tenant_id = $1 AND operation_id = $2",
            DatabaseBackend::Postgres,
        ),
        DatabaseBackend::Sqlite => (
            "SELECT * FROM forum_reply_range_move_operations WHERE tenant_id = ? AND operation_id = ?",
            DatabaseBackend::Sqlite,
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply range move receipt lookup does not support {backend:?}"
            )));
        }
    };
    Ok(txn
        .query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![tenant_id.into(), operation_id.into()],
        ))
        .await?
        .map(stored_operation_from_row)
        .transpose()?)
}

fn stored_operation_from_row(row: QueryResult) -> Result<StoredRangeMoveOperation, sea_orm::DbErr> {
    Ok(StoredRangeMoveOperation {
        tenant_id: row.try_get("", "tenant_id")?,
        operation_id: row.try_get("", "operation_id")?,
        source_topic_id: row.try_get("", "source_topic_id")?,
        target_topic_id: row.try_get("", "target_topic_id")?,
        source_category_id: row.try_get("", "source_category_id")?,
        target_category_id: row.try_get("", "target_category_id")?,
        actor_id: row.try_get("", "actor_id")?,
        reason: row.try_get("", "reason")?,
        command_fingerprint: row.try_get("", "command_fingerprint")?,
        source_start_position: row.try_get("", "source_start_position")?,
        source_end_position: row.try_get("", "source_end_position")?,
        target_start_position: row.try_get("", "target_start_position")?,
        target_end_position: row.try_get("", "target_end_position")?,
        moved_reply_count: row.try_get("", "moved_reply_count")?,
        moved_published_reply_count: row.try_get("", "moved_published_reply_count")?,
        source_resulting_published_reply_count: row
            .try_get("", "source_resulting_published_reply_count")?,
        target_resulting_published_reply_count: row
            .try_get("", "target_resulting_published_reply_count")?,
        moved_solution_reply_id: row.try_get("", "moved_solution_reply_id")?,
        source_resulting_solution_reply_id: row
            .try_get("", "source_resulting_solution_reply_id")?,
        target_resulting_solution_reply_id: row
            .try_get("", "target_resulting_solution_reply_id")?,
        event_id: row.try_get("", "event_id")?,
        moved_at: row.try_get("", "moved_at")?,
    })
}

async fn validate_replay_in_tx(
    txn: &DatabaseTransaction,
    existing: &StoredRangeMoveOperation,
    source_topic_id: Uuid,
    actor_id: Uuid,
    prepared: &PreparedRangeMoveInput,
) -> ForumResult<()> {
    let matches = existing.tenant_id != Uuid::nil()
        && existing.operation_id == prepared.operation_id
        && existing.source_topic_id == source_topic_id
        && existing.target_topic_id == prepared.target_topic_id
        && existing.actor_id == actor_id
        && existing.reason == prepared.reason
        && existing.command_fingerprint == prepared.command_fingerprint
        && existing.source_start_position == prepared.start_position
        && existing.source_end_position == prepared.end_position
        && existing.event_id == existing.operation_id;
    if !matches {
        return Err(ForumError::TopicReplyRangeMoveOperationConflict(
            prepared.operation_id,
        ));
    }
    let item_count =
        count_audit_items_in_tx(txn, existing.tenant_id, existing.operation_id).await?;
    if item_count != i64::from(existing.moved_reply_count) {
        return Err(ForumError::TopicReplyRangeMoveOperationConflict(
            prepared.operation_id,
        ));
    }
    let event_count = count_event_in_tx(txn, existing.tenant_id, existing.event_id).await?;
    if event_count != 1 {
        return Err(ForumError::TopicReplyRangeMoveOperationConflict(
            prepared.operation_id,
        ));
    }
    Ok(())
}

async fn count_audit_items_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> ForumResult<i64> {
    let (sql, backend) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            "SELECT COUNT(*)::bigint AS value FROM forum_reply_range_move_items WHERE tenant_id = $1 AND operation_id = $2",
            DatabaseBackend::Postgres,
        ),
        DatabaseBackend::Sqlite => (
            "SELECT COUNT(*) AS value FROM forum_reply_range_move_items WHERE tenant_id = ? AND operation_id = ?",
            DatabaseBackend::Sqlite,
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum reply range move audit replay does not support {backend:?}"
            )));
        }
    };
    let row = txn
        .query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![tenant_id.into(), operation_id.into()],
        ))
        .await?
        .ok_or_else(|| {
            ForumError::Validation("Forum reply range move audit count is unavailable".to_string())
        })?;
    Ok(row.try_get("", "value")?)
}

async fn count_event_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    event_id: Uuid,
) -> ForumResult<i64> {
    let count = forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::TenantId.eq(tenant_id))
        .filter(forum_domain_event::Column::EventId.eq(event_id))
        .filter(forum_domain_event::Column::EventType.eq(FORUM_REPLY_RANGE_MOVE_EVENT_TYPE))
        .count(txn)
        .await?;
    i64::try_from(count).map_err(|_| {
        ForumError::Validation("Forum reply range move event count overflow".to_string())
    })
}

fn operation_to_result(operation: StoredRangeMoveOperation) -> ForumReplyRangeMoveResult {
    ForumReplyRangeMoveResult {
        operation_id: operation.operation_id,
        event_id: operation.event_id,
        source_topic_id: operation.source_topic_id,
        target_topic_id: operation.target_topic_id,
        source_category_id: operation.source_category_id,
        target_category_id: operation.target_category_id,
        actor_id: operation.actor_id,
        reason: operation.reason,
        source_start_position: operation.source_start_position,
        source_end_position: operation.source_end_position,
        target_start_position: operation.target_start_position,
        target_end_position: operation.target_end_position,
        moved_reply_count: operation.moved_reply_count,
        moved_published_reply_count: operation.moved_published_reply_count,
        source_resulting_published_reply_count: operation.source_resulting_published_reply_count,
        target_resulting_published_reply_count: operation.target_resulting_published_reply_count,
        moved_solution_reply_id: operation.moved_solution_reply_id,
        source_resulting_solution_reply_id: operation.source_resulting_solution_reply_id,
        target_resulting_solution_reply_id: operation.target_resulting_solution_reply_id,
        moved_at: operation.moved_at.with_timezone(&Utc),
    }
}
