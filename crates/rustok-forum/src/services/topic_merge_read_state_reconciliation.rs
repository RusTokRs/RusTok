use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::entities::{
    forum_domain_event, forum_topic_merge_operation, forum_topic_merge_read_state_reconciliation,
    forum_topic_read_state,
};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::TopicStatus;

use super::rbac::enforce_scope;
use super::topic_read_state_lock::{
    lock_topic_read_state_scopes_in_tx, lock_topic_rows_for_read_state_in_tx,
};

pub const MAX_FORUM_TOPIC_MERGE_READ_STATES: u64 = 500;
pub const MAX_FORUM_TOPIC_MERGE_READ_STATE_REASON_LEN: usize = 500;
const FORUM_TOPIC_MERGE_READ_STATES_RECONCILED_EVENT_TYPE: &str =
    "forum.topic.merge.read_state_reconciled";
const FORUM_TOPIC_AGGREGATE_TYPE: &str = "forum_topic";
const FORUM_TOPIC_MERGED_EVENT_TYPE: &str = "forum.topic.merged";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileForumTopicMergeReadStatesInput {
    pub operation_id: Uuid,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumTopicMergeReadStateReconciliationResult {
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub merge_operation_id: Uuid,
    pub source_topic_id: Uuid,
    pub target_topic_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub source_read_state_count: i32,
    pub discarded_source_only_count: i32,
    pub discarded_target_overlap_count: i32,
    pub reconciled_at: DateTime<Utc>,
}

/// Conservatively reconcile topic read state after one completed FORUM-21B merge.
///
/// A single target high-water cannot represent discontiguous source and target reading history.
/// The retained target state therefore remains authoritative and byte-semantic unchanged. Every
/// source state is discarded, with source-only and target-overlap rows counted separately.
pub struct ForumTopicMergeReadStateReconciliationService {
    db: DatabaseConnection,
}

impl ForumTopicMergeReadStateReconciliationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, security, input))]
    pub async fn reconcile_merge_read_states(
        &self,
        tenant_id: Uuid,
        merge_operation_id: Uuid,
        security: SecurityContext,
        input: ReconcileForumTopicMergeReadStatesInput,
    ) -> ForumResult<ForumTopicMergeReadStateReconciliationResult> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        let actor_id = security.user_id.ok_or_else(|| {
            ForumError::Validation(
                "Forum topic merge read-state reconciliation requires a human actor".to_string(),
            )
        })?;
        let reason = validate_input(tenant_id, merge_operation_id, actor_id, &input)?;

        let txn = self.db.begin().await?;
        lock_reconciliation_tenant_in_tx(&txn, tenant_id).await?;

        if let Some(existing) = forum_topic_merge_read_state_reconciliation::Entity::find_by_id((
            tenant_id,
            input.operation_id,
        ))
        .one(&txn)
        .await?
        {
            if existing.merge_operation_id != merge_operation_id
                || existing.actor_id != actor_id
                || existing.reason != reason
            {
                return Err(ForumError::TopicMergeReadStateReconciliationConflict(
                    input.operation_id,
                ));
            }
            validate_reconciliation_event_in_tx(&txn, &existing).await?;
            lock_topic_read_state_scopes_in_tx(
                &txn,
                tenant_id,
                &[existing.source_topic_id, existing.target_topic_id],
            )
            .await?;
            ensure_source_read_states_empty_in_tx(&txn, tenant_id, existing.source_topic_id)
                .await?;
            txn.commit().await?;
            return Ok(operation_to_result(existing));
        }

        if forum_topic_merge_read_state_reconciliation::Entity::find()
            .filter(forum_topic_merge_read_state_reconciliation::Column::TenantId.eq(tenant_id))
            .filter(
                forum_topic_merge_read_state_reconciliation::Column::MergeOperationId
                    .eq(merge_operation_id),
            )
            .one(&txn)
            .await?
            .is_some()
        {
            return Err(ForumError::TopicMergeReadStateReconciliationConflict(
                input.operation_id,
            ));
        }

        let merge =
            forum_topic_merge_operation::Entity::find_by_id((tenant_id, merge_operation_id))
                .one(&txn)
                .await?
                .ok_or_else(|| {
                    ForumError::Validation(
                    "Forum topic merge read-state reconciliation requires an existing merge receipt"
                        .to_string(),
                )
                })?;
        validate_merge_event_in_tx(&txn, &merge).await?;

        let topics = lock_topic_rows_for_read_state_in_tx(
            &txn,
            tenant_id,
            &[merge.source_topic_id, merge.target_topic_id],
        )
        .await?
        .into_iter()
        .map(|topic| (topic.id, topic))
        .collect::<HashMap<_, _>>();
        let source = topics
            .get(&merge.source_topic_id)
            .ok_or(ForumError::TopicNotFound(merge.source_topic_id))?;
        let target = topics
            .get(&merge.target_topic_id)
            .ok_or(ForumError::TopicNotFound(merge.target_topic_id))?;
        if source.status != TopicStatus::Archived || !source.is_locked {
            return Err(ForumError::Validation(
                "Forum topic merge read-state reconciliation requires the archived locked source tombstone"
                    .to_string(),
            ));
        }
        if target.status == TopicStatus::Archived {
            return Err(ForumError::Validation(
                "Forum topic merge read-state reconciliation requires a non-archived retained target"
                    .to_string(),
            ));
        }
        if source.category_id != merge.category_id
            || target.category_id != merge.category_id
            || source.category_id != target.category_id
        {
            return Err(ForumError::Validation(
                "Forum topic merge read-state reconciliation does not match the merge category"
                    .to_string(),
            ));
        }

        lock_topic_read_state_scopes_in_tx(
            &txn,
            tenant_id,
            &[merge.source_topic_id, merge.target_topic_id],
        )
        .await?;

        let source_rows = forum_topic_read_state::Entity::find()
            .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_read_state::Column::TopicId.eq(merge.source_topic_id))
            .order_by_asc(forum_topic_read_state::Column::UserId)
            .limit(MAX_FORUM_TOPIC_MERGE_READ_STATES + 1)
            .all(&txn)
            .await?;
        if source_rows.len() as u64 > MAX_FORUM_TOPIC_MERGE_READ_STATES {
            return Err(ForumError::Validation(format!(
                "Forum topic merge read-state source must not exceed {MAX_FORUM_TOPIC_MERGE_READ_STATES} rows"
            )));
        }

        let source_user_ids = source_rows
            .iter()
            .map(|row| row.user_id)
            .collect::<Vec<_>>();
        let target_users = if source_user_ids.is_empty() {
            HashSet::new()
        } else {
            forum_topic_read_state::Entity::find()
                .select_only()
                .column(forum_topic_read_state::Column::UserId)
                .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
                .filter(forum_topic_read_state::Column::TopicId.eq(merge.target_topic_id))
                .filter(forum_topic_read_state::Column::UserId.is_in(source_user_ids))
                .into_tuple::<Uuid>()
                .all(&txn)
                .await?
                .into_iter()
                .collect::<HashSet<_>>()
        };

        let discarded_target_overlap_count = i32::try_from(target_users.len()).map_err(|_| {
            ForumError::Validation(
                "Forum topic merge read-state overlap count exceeds supported range".to_string(),
            )
        })?;
        let source_read_state_count = i32::try_from(source_rows.len()).map_err(|_| {
            ForumError::Validation(
                "Forum topic merge read-state count exceeds supported range".to_string(),
            )
        })?;
        let discarded_source_only_count = source_read_state_count
            .checked_sub(discarded_target_overlap_count)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum topic merge read-state classification is inconsistent".to_string(),
                )
            })?;

        let deleted = forum_topic_read_state::Entity::delete_many()
            .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_read_state::Column::TopicId.eq(merge.source_topic_id))
            .exec(&txn)
            .await?;
        if deleted.rows_affected != source_rows.len() as u64 {
            return Err(ForumError::Validation(
                "Forum source topic read state changed concurrently".to_string(),
            ));
        }
        ensure_source_read_states_empty_in_tx(&txn, tenant_id, merge.source_topic_id).await?;

        let now = Utc::now();
        let payload = reconciliation_payload(
            input.operation_id,
            merge_operation_id,
            merge.source_topic_id,
            merge.target_topic_id,
            source_read_state_count,
            discarded_source_only_count,
            discarded_target_overlap_count,
            &reason,
        );
        forum_domain_event::ActiveModel {
            sequence_no: NotSet,
            event_id: Set(input.operation_id),
            tenant_id: Set(tenant_id),
            aggregate_type: Set(FORUM_TOPIC_AGGREGATE_TYPE.to_string()),
            aggregate_id: Set(merge.target_topic_id),
            event_type: Set(FORUM_TOPIC_MERGE_READ_STATES_RECONCILED_EVENT_TYPE.to_string()),
            schema_version: Set(1),
            actor_id: Set(Some(actor_id)),
            payload: Set(payload),
            created_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        let operation = forum_topic_merge_read_state_reconciliation::ActiveModel {
            tenant_id: Set(tenant_id),
            operation_id: Set(input.operation_id),
            merge_operation_id: Set(merge_operation_id),
            source_topic_id: Set(merge.source_topic_id),
            target_topic_id: Set(merge.target_topic_id),
            actor_id: Set(actor_id),
            reason: Set(reason),
            source_read_state_count: Set(source_read_state_count),
            discarded_source_only_count: Set(discarded_source_only_count),
            discarded_target_overlap_count: Set(discarded_target_overlap_count),
            event_id: Set(input.operation_id),
            reconciled_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        txn.commit().await?;
        Ok(operation_to_result(operation))
    }
}

fn validate_input(
    tenant_id: Uuid,
    merge_operation_id: Uuid,
    actor_id: Uuid,
    input: &ReconcileForumTopicMergeReadStatesInput,
) -> ForumResult<String> {
    for (label, value) in [
        ("tenant", tenant_id),
        ("operation", input.operation_id),
        ("merge operation", merge_operation_id),
        ("actor", actor_id),
    ] {
        if value.is_nil() {
            return Err(ForumError::Validation(format!(
                "Forum topic merge read-state reconciliation {label} must not be nil"
            )));
        }
    }
    let reason = input.reason.trim();
    if reason.is_empty() {
        return Err(ForumError::Validation(
            "Forum topic merge read-state reconciliation reason must not be empty".to_string(),
        ));
    }
    if reason.chars().count() > MAX_FORUM_TOPIC_MERGE_READ_STATE_REASON_LEN {
        return Err(ForumError::Validation(format!(
            "Forum topic merge read-state reconciliation reason must not exceed {MAX_FORUM_TOPIC_MERGE_READ_STATE_REASON_LEN} characters"
        )));
    }
    if reason.chars().any(char::is_control) {
        return Err(ForumError::Validation(
            "Forum topic merge read-state reconciliation reason must not contain control characters"
                .to_string(),
        ));
    }
    Ok(reason.to_string())
}

async fn lock_reconciliation_tenant_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 25))",
                vec![format!("forum-topic-merge-read-state-reconciliation:{tenant_id}").into()],
            ))
            .await?;
        }
        DatabaseBackend::Sqlite => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"
                INSERT INTO forum_topic_merge_read_state_reconciliation_locks (
                    tenant_id, touched_at
                ) VALUES (?, CURRENT_TIMESTAMP)
                ON CONFLICT(tenant_id) DO UPDATE SET touched_at = CURRENT_TIMESTAMP
                "#,
                vec![tenant_id.into()],
            ))
            .await?;
        }
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic merge read-state reconciliation does not support database backend {backend:?}"
            )));
        }
    }
    Ok(())
}

async fn ensure_source_read_states_empty_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
) -> ForumResult<()> {
    let remaining = forum_topic_read_state::Entity::find()
        .filter(forum_topic_read_state::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_read_state::Column::TopicId.eq(source_topic_id))
        .count(txn)
        .await?;
    if remaining != 0 {
        return Err(ForumError::Validation(
            "Forum source topic read states remain after reconciliation".to_string(),
        ));
    }
    Ok(())
}

fn reconciliation_payload(
    operation_id: Uuid,
    merge_operation_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    source_read_state_count: i32,
    discarded_source_only_count: i32,
    discarded_target_overlap_count: i32,
    reason: &str,
) -> JsonValue {
    json!({
        "operation_id": operation_id,
        "merge_operation_id": merge_operation_id,
        "source_topic_id": source_topic_id,
        "target_topic_id": target_topic_id,
        "source_read_state_count": source_read_state_count,
        "discarded_source_only_count": discarded_source_only_count,
        "discarded_target_overlap_count": discarded_target_overlap_count,
        "reason": reason,
    })
}

async fn validate_merge_event_in_tx(
    txn: &DatabaseTransaction,
    merge: &forum_topic_merge_operation::Model,
) -> ForumResult<()> {
    let event = forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::TenantId.eq(merge.tenant_id))
        .filter(forum_domain_event::Column::EventId.eq(merge.event_id))
        .one(txn)
        .await?
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum topic merge receipt is missing its semantic event".to_string(),
            )
        })?;
    let expected_payload = json!({
        "operation_id": merge.operation_id,
        "source_topic_id": merge.source_topic_id,
        "target_topic_id": merge.target_topic_id,
        "category_id": merge.category_id,
        "moved_reply_count": merge.moved_reply_count,
        "moved_published_reply_count": merge.moved_published_reply_count,
        "resulting_published_reply_count": merge.resulting_published_reply_count,
        "position_offset": merge.position_offset,
        "reason": merge.reason,
    });
    if event.aggregate_type != FORUM_TOPIC_AGGREGATE_TYPE
        || event.aggregate_id != merge.target_topic_id
        || event.event_type != FORUM_TOPIC_MERGED_EVENT_TYPE
        || event.schema_version != 1
        || event.actor_id != Some(merge.actor_id)
        || event.payload != expected_payload
    {
        return Err(ForumError::Validation(
            "Forum topic merge receipt semantic event does not match".to_string(),
        ));
    }
    Ok(())
}

async fn validate_reconciliation_event_in_tx(
    txn: &DatabaseTransaction,
    operation: &forum_topic_merge_read_state_reconciliation::Model,
) -> ForumResult<()> {
    let event = forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::TenantId.eq(operation.tenant_id))
        .filter(forum_domain_event::Column::EventId.eq(operation.event_id))
        .one(txn)
        .await?
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum topic merge read-state reconciliation is missing its semantic event"
                    .to_string(),
            )
        })?;
    let expected_payload = reconciliation_payload(
        operation.operation_id,
        operation.merge_operation_id,
        operation.source_topic_id,
        operation.target_topic_id,
        operation.source_read_state_count,
        operation.discarded_source_only_count,
        operation.discarded_target_overlap_count,
        &operation.reason,
    );
    if event.aggregate_type != FORUM_TOPIC_AGGREGATE_TYPE
        || event.aggregate_id != operation.target_topic_id
        || event.event_type != FORUM_TOPIC_MERGE_READ_STATES_RECONCILED_EVENT_TYPE
        || event.schema_version != 1
        || event.actor_id != Some(operation.actor_id)
        || event.payload != expected_payload
    {
        return Err(ForumError::Validation(
            "Forum topic merge read-state reconciliation semantic event does not match its receipt"
                .to_string(),
        ));
    }
    Ok(())
}

fn operation_to_result(
    operation: forum_topic_merge_read_state_reconciliation::Model,
) -> ForumTopicMergeReadStateReconciliationResult {
    ForumTopicMergeReadStateReconciliationResult {
        operation_id: operation.operation_id,
        event_id: operation.event_id,
        merge_operation_id: operation.merge_operation_id,
        source_topic_id: operation.source_topic_id,
        target_topic_id: operation.target_topic_id,
        actor_id: operation.actor_id,
        reason: operation.reason,
        source_read_state_count: operation.source_read_state_count,
        discarded_source_only_count: operation.discarded_source_only_count,
        discarded_target_overlap_count: operation.discarded_target_overlap_count,
        reconciled_at: operation.reconciled_at.with_timezone(&Utc),
    }
}
