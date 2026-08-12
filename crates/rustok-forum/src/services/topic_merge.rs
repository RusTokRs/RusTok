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
    forum_category, forum_category_lifecycle, forum_domain_event, forum_reply, forum_solution,
    forum_topic, forum_topic_merge_operation, forum_topic_merge_solution_resolution,
};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::{ReplyStatus, TopicStatus};

use super::projection_invalidation::{
    publish_forum_category_projection_in_tx, publish_forum_topic_projection_in_tx,
};
use super::rbac::enforce_scope;
use super::topic_audience::load_policy_for_topic;
use super::topic_route::ForumTopicRouteService;
use super::user_stats::UserStatsService;

pub const MAX_FORUM_TOPIC_MERGE_REASON_LEN: usize = 500;
pub const MAX_FORUM_TOPIC_MERGE_REPLIES: u64 = 500;
const FORUM_TOPIC_MERGED_EVENT_TYPE: &str = "forum.topic.merged";
const FORUM_TOPIC_MERGED_AGGREGATE_TYPE: &str = "forum_topic";
const FORUM_TOPIC_MERGED_SCHEMA_VERSION: i16 = 1;

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
struct ForumTopicMergeSolutionCandidate {
    reply_id: Uuid,
    reply_author_id: Option<Uuid>,
    marked_by_user_id: Option<Uuid>,
    marked_at: DateTimeWithTimeZone,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ForumTopicMergeSolutionResolutionAudit {
    source_solution_reply_id: Uuid,
    target_solution_reply_id: Uuid,
    selected_solution_reply_id: Uuid,
    rejected_solution_reply_id: Uuid,
    rejected_solution_author_id: Option<Uuid>,
}

struct ForumTopicMergeSolutionPlan {
    source_solution_transfer: Option<ForumTopicMergeSolutionCandidate>,
    delete_source_solution: bool,
    delete_target_solution: bool,
    losing_solution_author_id: Option<Uuid>,
    audit: Option<ForumTopicMergeSolutionResolutionAudit>,
}

/// Idempotent merge of one active source topic into one retained active target topic.
///
/// Source and target may belong to the same active category or to two different active
/// categories. The target identity and topic-owned policy remain authoritative. Reply identities
/// and all reply-owned relations are retained while reply positions are shifted after the
/// target's current maximum. For a cross-category merge, the archived source tombstone stays in
/// its original category, both category topic counters remain unchanged, and only the source
/// published-reply contribution moves to the target category with checked arithmetic. A
/// source-only accepted solution follows its unchanged reply identity and preserves its marker
/// metadata. Competing accepted solutions require the explicit manager command, which selects one
/// reply, stores both candidates in an append-only audit row linked to the immutable merge receipt,
/// and decrements the losing reply author's solution statistic exactly once. Subscriptions, tags
/// and topic-level audience relations are reconciled by their dedicated bounded policies against
/// the unchanged schema-version-1 merge event.
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
        self.merge_topic_internal(tenant_id, target_topic_id, security, None, input)
            .await
    }

    #[instrument(skip(self, security, input))]
    pub async fn merge_topic_resolving_solution(
        &self,
        tenant_id: Uuid,
        target_topic_id: Uuid,
        security: SecurityContext,
        selected_solution_reply_id: Uuid,
        input: MergeForumTopicInput,
    ) -> ForumResult<ForumTopicMergeResult> {
        if selected_solution_reply_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum topic merge selected solution reply must not be nil".to_string(),
            ));
        }
        self.merge_topic_internal(
            tenant_id,
            target_topic_id,
            security,
            Some(selected_solution_reply_id),
            input,
        )
        .await
    }

    async fn merge_topic_internal(
        &self,
        tenant_id: Uuid,
        target_topic_id: Uuid,
        security: SecurityContext,
        selected_solution_reply_id: Option<Uuid>,
        input: MergeForumTopicInput,
    ) -> ForumResult<ForumTopicMergeResult> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        let actor_id = security.user_id.ok_or_else(|| {
            ForumError::Validation("Forum topic merge requires a human actor".to_string())
        })?;
        let reason = validate_merge_input(tenant_id, target_topic_id, actor_id, &input)?;

        let txn = self.db.begin().await?;
        lock_topic_merge_tenant_in_tx(&txn, tenant_id).await?;

        if let Some(existing) =
            forum_topic_merge_operation::Entity::find_by_id((tenant_id, input.operation_id))
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
            let stored_resolution = load_solution_resolution_audit_in_tx(
                &txn,
                existing.tenant_id,
                existing.operation_id,
            )
            .await?;
            if stored_resolution
                .as_ref()
                .map(|audit| audit.selected_solution_reply_id)
                != selected_solution_reply_id
            {
                return Err(ForumError::TopicMergeOperationConflict(input.operation_id));
            }
            txn.commit().await?;
            return Ok(operation_to_result(existing));
        }

        let preliminary_source = find_topic_in_tx(&txn, tenant_id, input.source_topic_id).await?;
        let preliminary_target = find_topic_in_tx(&txn, tenant_id, target_topic_id).await?;
        lock_merge_counter_scopes_in_tx(
            &txn,
            tenant_id,
            &[
                preliminary_source.category_id,
                preliminary_target.category_id,
            ],
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
        let source_category_id = source.category_id;
        let target_category_id = target.category_id;
        ensure_categories_active_in_tx(&txn, tenant_id, &[source_category_id, target_category_id])
            .await?;

        lock_topic_solution_scopes_in_tx(&txn, tenant_id, &[source.id, target.id]).await?;
        let source_solution =
            load_valid_solution_in_tx(&txn, tenant_id, source.id, "source").await?;
        let target_solution =
            load_valid_solution_in_tx(&txn, tenant_id, target.id, "target").await?;
        let solution_plan = plan_solution_merge(
            input.operation_id,
            selected_solution_reply_id,
            source_solution.as_ref(),
            target_solution.as_ref(),
        )?;

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

        transfer_cross_category_reply_counters_in_tx(
            &txn,
            tenant_id,
            source_category_id,
            target_category_id,
            moved_published_reply_count,
        )
        .await?;

        if solution_plan.delete_source_solution {
            delete_solution_in_tx(&txn, tenant_id, source.id, "source").await?;
        }
        if solution_plan.delete_target_solution {
            delete_solution_in_tx(&txn, tenant_id, target.id, "target").await?;
        }
        if solution_plan.audit.is_some() {
            UserStatsService::adjust_solution_count_in_tx(
                &txn,
                tenant_id,
                solution_plan.losing_solution_author_id,
                -1,
            )
            .await?;
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
        if let Some(solution) = solution_plan.source_solution_transfer.as_ref() {
            insert_transferred_solution_in_tx(&txn, tenant_id, target.id, solution).await?;
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

        ForumTopicRouteService::record_merge_redirect_aliases_in_tx(
            &txn,
            tenant_id,
            input.source_topic_id,
            target_topic_id,
            &reason,
        )
        .await?;

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
            schema_version: Set(FORUM_TOPIC_MERGED_SCHEMA_VERSION),
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

        if let Some(audit) = solution_plan.audit.as_ref() {
            forum_topic_merge_solution_resolution::ActiveModel {
                tenant_id: Set(tenant_id),
                operation_id: Set(input.operation_id),
                source_solution_reply_id: Set(audit.source_solution_reply_id),
                target_solution_reply_id: Set(audit.target_solution_reply_id),
                selected_solution_reply_id: Set(audit.selected_solution_reply_id),
                rejected_solution_reply_id: Set(audit.rejected_solution_reply_id),
                rejected_solution_author_id: Set(audit.rejected_solution_author_id),
                resolved_at: Set(now.into()),
            }
            .insert(&txn)
            .await?;
        }

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
            source_category_id,
        )
        .await?;
        if target_category_id != source_category_id {
            publish_forum_category_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                Some(actor_id),
                target_category_id,
            )
            .await?;
        }

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

fn plan_solution_merge(
    operation_id: Uuid,
    selected_solution_reply_id: Option<Uuid>,
    source_solution: Option<&ForumTopicMergeSolutionCandidate>,
    target_solution: Option<&ForumTopicMergeSolutionCandidate>,
) -> ForumResult<ForumTopicMergeSolutionPlan> {
    match (source_solution, target_solution, selected_solution_reply_id) {
        (None, None, None) | (None, Some(_), None) => Ok(ForumTopicMergeSolutionPlan {
            source_solution_transfer: None,
            delete_source_solution: false,
            delete_target_solution: false,
            losing_solution_author_id: None,
            audit: None,
        }),
        (Some(source), None, None) => Ok(ForumTopicMergeSolutionPlan {
            source_solution_transfer: Some(source.clone()),
            delete_source_solution: true,
            delete_target_solution: false,
            losing_solution_author_id: None,
            audit: None,
        }),
        (Some(_), Some(_), None) => Err(ForumError::TopicMergeSolutionConflict(operation_id)),
        (Some(source), Some(target), Some(selected)) => {
            let (
                source_solution_transfer,
                delete_target_solution,
                losing_solution_author_id,
                rejected_solution_reply_id,
                rejected_solution_author_id,
            ) = if selected == source.reply_id {
                (
                    Some(source.clone()),
                    true,
                    target.reply_author_id,
                    target.reply_id,
                    target.reply_author_id,
                )
            } else if selected == target.reply_id {
                (
                    None,
                    false,
                    source.reply_author_id,
                    source.reply_id,
                    source.reply_author_id,
                )
            } else {
                return Err(ForumError::Validation(
                    "Forum topic merge selected solution must identify one competing accepted reply"
                        .to_string(),
                ));
            };
            Ok(ForumTopicMergeSolutionPlan {
                source_solution_transfer,
                delete_source_solution: true,
                delete_target_solution,
                losing_solution_author_id,
                audit: Some(ForumTopicMergeSolutionResolutionAudit {
                    source_solution_reply_id: source.reply_id,
                    target_solution_reply_id: target.reply_id,
                    selected_solution_reply_id: selected,
                    rejected_solution_reply_id,
                    rejected_solution_author_id,
                }),
            })
        }
        (_, _, Some(_)) => Err(ForumError::Validation(
            "Forum topic merge solution selection requires competing accepted solutions"
                .to_string(),
        )),
    }
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
    category_ids: &[Uuid],
    source_topic_id: Uuid,
    target_topic_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            let mut categories = category_ids.to_vec();
            categories.sort();
            categories.dedup();
            let mut topic_ids = [source_topic_id, target_topic_id];
            topic_ids.sort();
            let mut scopes = categories
                .into_iter()
                .map(|category_id| format!("forum:category:{tenant_id}:{category_id}"))
                .collect::<Vec<_>>();
            scopes.extend([
                format!("forum:topic:{tenant_id}:{}", topic_ids[0]),
                format!("forum:topic:{tenant_id}:{}", topic_ids[1]),
            ]);
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

async fn ensure_categories_active_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_ids: &[Uuid],
) -> ForumResult<()> {
    let mut ids = category_ids.to_vec();
    ids.sort();
    ids.dedup();
    for category_id in ids {
        ensure_category_active_in_tx(txn, tenant_id, category_id).await?;
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
            "Forum topic merge requires active source and target categories".to_string(),
        ));
    }
    Ok(())
}

async fn transfer_cross_category_reply_counters_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_category_id: Uuid,
    target_category_id: Uuid,
    moved_published_reply_count: i32,
) -> ForumResult<()> {
    if source_category_id == target_category_id {
        return Ok(());
    }
    if moved_published_reply_count < 0 {
        return Err(ForumError::Validation(
            "Forum cross-category merge published reply count must not be negative".to_string(),
        ));
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

    if source.topic_count <= 0 || target.topic_count <= 0 {
        return Err(ForumError::Validation(
            "Forum cross-category merge category topic counters are inconsistent".to_string(),
        ));
    }
    if source.reply_count < moved_published_reply_count {
        return Err(ForumError::Validation(
            "Forum source category published reply counter is inconsistent".to_string(),
        ));
    }
    if target.reply_count < 0 {
        return Err(ForumError::Validation(
            "Forum target category published reply counter is inconsistent".to_string(),
        ));
    }

    let source_reply_count = source
        .reply_count
        .checked_sub(moved_published_reply_count)
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum source category published reply counter is inconsistent".to_string(),
            )
        })?;
    let target_reply_count = target
        .reply_count
        .checked_add(moved_published_reply_count)
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum target category published reply counter overflow".to_string(),
            )
        })?;
    let now = Utc::now();

    let mut source_active: forum_category::ActiveModel = source.into();
    source_active.reply_count = Set(source_reply_count);
    source_active.updated_at = Set(now.into());
    source_active.update(txn).await?;

    let mut target_active: forum_category::ActiveModel = target.into();
    target_active.reply_count = Set(target_reply_count);
    target_active.updated_at = Set(now.into());
    target_active.update(txn).await?;
    Ok(())
}

async fn load_valid_solution_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    label: &str,
) -> ForumResult<Option<ForumTopicMergeSolutionCandidate>> {
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
        .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
        .one(txn)
        .await?;
    let Some(reply) = reply else {
        return Err(ForumError::Validation(format!(
            "Forum topic merge requires a valid approved non-deleted {label} solution"
        )));
    };
    Ok(Some(ForumTopicMergeSolutionCandidate {
        reply_id: solution.reply_id,
        reply_author_id: reply.author_id,
        marked_by_user_id: solution.marked_by_user_id,
        marked_at: solution.marked_at,
    }))
}

async fn delete_solution_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    label: &str,
) -> ForumResult<()> {
    let result = forum_solution::Entity::delete_many()
        .filter(forum_solution::Column::TenantId.eq(tenant_id))
        .filter(forum_solution::Column::TopicId.eq(topic_id))
        .exec(txn)
        .await?;
    if result.rows_affected != 1 {
        return Err(ForumError::Validation(format!(
            "Forum {label} accepted solution changed concurrently"
        )));
    }
    Ok(())
}

async fn insert_transferred_solution_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    target_topic_id: Uuid,
    solution: &ForumTopicMergeSolutionCandidate,
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
        || event.schema_version != FORUM_TOPIC_MERGED_SCHEMA_VERSION
        || event.actor_id != Some(operation.actor_id)
        || event.payload != expected_payload
    {
        return Err(ForumError::Validation(
            "Forum topic merge operation semantic event does not match its receipt".to_string(),
        ));
    }
    Ok(())
}

async fn load_solution_resolution_audit_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> ForumResult<Option<ForumTopicMergeSolutionResolutionAudit>> {
    let row = forum_topic_merge_solution_resolution::Entity::find_by_id((tenant_id, operation_id))
        .one(txn)
        .await?;
    row.map(|row| {
        let audit = ForumTopicMergeSolutionResolutionAudit {
            source_solution_reply_id: row.source_solution_reply_id,
            target_solution_reply_id: row.target_solution_reply_id,
            selected_solution_reply_id: row.selected_solution_reply_id,
            rejected_solution_reply_id: row.rejected_solution_reply_id,
            rejected_solution_author_id: row.rejected_solution_author_id,
        };
        validate_solution_resolution_audit(&audit)?;
        Ok(audit)
    })
    .transpose()
}

fn validate_solution_resolution_audit(
    audit: &ForumTopicMergeSolutionResolutionAudit,
) -> ForumResult<()> {
    let ids = [
        audit.source_solution_reply_id,
        audit.target_solution_reply_id,
        audit.selected_solution_reply_id,
        audit.rejected_solution_reply_id,
    ];
    if ids.iter().any(|id| id.is_nil())
        || audit.source_solution_reply_id == audit.target_solution_reply_id
        || audit.selected_solution_reply_id == audit.rejected_solution_reply_id
        || !((audit.selected_solution_reply_id == audit.source_solution_reply_id
            && audit.rejected_solution_reply_id == audit.target_solution_reply_id)
            || (audit.selected_solution_reply_id == audit.target_solution_reply_id
                && audit.rejected_solution_reply_id == audit.source_solution_reply_id))
    {
        return Err(ForumError::Validation(
            "Forum topic merge solution-resolution audit is inconsistent".to_string(),
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
