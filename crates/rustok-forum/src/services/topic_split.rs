use std::collections::HashSet;

use chrono::{DateTime, Utc};
use rustok_api::{Action, Resource, RichTextDocument};
use rustok_content::normalize_locale_code;
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QueryResult, Statement, TransactionTrait,
    prelude::DateTimeWithTimeZone,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use tracing::instrument;
use uuid::Uuid;

use crate::entities::{
    forum_category, forum_category_lifecycle, forum_domain_event, forum_reply, forum_solution,
    forum_topic, forum_topic_channel_access, forum_topic_translation,
};
use crate::error::{ForumError, ForumResult};
use crate::richtext::serialize_discussion;
use crate::state_machine::{ReplyStatus, TopicStatus};

use super::category_audience::lock_category_tree_in_tx;
use super::projection_invalidation::{
    publish_forum_category_projection_in_tx, publish_forum_topic_projection_in_tx,
};
use super::rbac::enforce_scope;
use super::topic_audience::load_policy_for_topic;
use super::topic_audience_lock::lock_topic_audience_scopes_in_tx;
use super::topic_reply_create_audience::load_topic_reply_create_audience_policy_for_topic;
use super::user_stats::UserStatsService;

pub const MAX_FORUM_TOPIC_SPLIT_REASON_LEN: usize = 500;
pub const MAX_FORUM_TOPIC_SPLIT_REPLIES: usize = 500;
pub const MAX_FORUM_TOPIC_SPLIT_TITLE_LEN: usize = 500;
const MAX_FORUM_TOPIC_SPLIT_SLUG_LEN: usize = 255;
const FORUM_TOPIC_SPLIT_EVENT_TYPE: &str = "forum.topic.split";
const FORUM_TOPIC_SPLIT_AGGREGATE_TYPE: &str = "forum_topic";
const FORUM_TOPIC_SPLIT_SCHEMA_VERSION: i16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitForumTopicRepliesInput {
    pub operation_id: Uuid,
    pub target_topic_id: Uuid,
    pub reply_ids: Vec<Uuid>,
    pub locale: String,
    pub title: String,
    pub slug: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumTopicSplitResult {
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub source_topic_id: Uuid,
    pub target_topic_id: Uuid,
    pub category_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub moved_reply_count: i32,
    pub moved_published_reply_count: i32,
    pub source_resulting_published_reply_count: i32,
    pub target_resulting_published_reply_count: i32,
    pub solution_reply_id: Option<Uuid>,
    pub split_at: DateTime<Utc>,
}

struct PreparedSplitInput {
    operation_id: Uuid,
    target_topic_id: Uuid,
    reply_ids: Vec<Uuid>,
    locale: String,
    title: String,
    slug: Option<String>,
    stored_body: String,
    reason: String,
    command_fingerprint: String,
}

#[derive(Clone)]
struct SplitSolutionCandidate {
    reply_id: Uuid,
    marked_by_user_id: Option<Uuid>,
    marked_at: DateTimeWithTimeZone,
}

struct SplitReplyAudit {
    reply_id: Uuid,
    source_position: i64,
    target_position: i64,
    was_published: bool,
}

struct StoredSplitOperation {
    tenant_id: Uuid,
    operation_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    category_id: Uuid,
    actor_id: Uuid,
    reason: String,
    command_fingerprint: String,
    moved_reply_count: i32,
    moved_published_reply_count: i32,
    source_resulting_published_reply_count: i32,
    target_resulting_published_reply_count: i32,
    solution_reply_id: Option<Uuid>,
    event_id: Uuid,
    split_at: DateTimeWithTimeZone,
}

/// Idempotently creates one same-category topic and moves an exact selected reply set into it.
///
/// The selection must be bounded and no parent edge may cross the split boundary. Reply IDs,
/// bodies, revisions, attachments encoded by the reply body, mention projections, quote
/// projections and authorship remain unchanged. The new topic copies the source topic's channel,
/// visibility and reply-create narrowing layers before any reply moves. Category reply totals stay
/// unchanged, the category topic total increases by one, source/target published counters are
/// reconciled from authoritative reply rows, and an accepted solution follows its unchanged reply
/// identity through the existing composite foreign-key cascade when that reply is selected.
/// Replaying the exact command returns the immutable receipt; actor or command-shape drift fails
/// closed.
pub struct ForumTopicSplitService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ForumTopicSplitService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, security, input))]
    pub async fn split_selected_replies(
        &self,
        tenant_id: Uuid,
        source_topic_id: Uuid,
        security: SecurityContext,
        input: SplitForumTopicRepliesInput,
    ) -> ForumResult<ForumTopicSplitResult> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        let actor_id = security.user_id.ok_or_else(|| {
            ForumError::Validation("Forum topic split requires a human actor".to_string())
        })?;
        let prepared = prepare_split_input(tenant_id, source_topic_id, actor_id, input)?;

        let txn = self.db.begin().await?;
        lock_topic_split_tenant_in_tx(&txn, tenant_id).await?;

        if let Some(existing) =
            load_split_operation_in_tx(&txn, tenant_id, prepared.operation_id).await?
        {
            validate_replay_in_tx(&txn, &existing, source_topic_id, actor_id, &prepared).await?;
            txn.commit().await?;
            return Ok(operation_to_result(existing));
        }

        let preliminary_source = find_topic_in_tx(&txn, tenant_id, source_topic_id).await?;
        lock_split_counter_scopes_in_tx(
            &txn,
            tenant_id,
            preliminary_source.category_id,
            source_topic_id,
            prepared.target_topic_id,
        )
        .await?;
        lock_source_topic_in_tx(&txn, tenant_id, source_topic_id).await?;
        let source = find_topic_in_tx(&txn, tenant_id, source_topic_id).await?;
        if source.category_id != preliminary_source.category_id {
            return Err(ForumError::Validation(
                "Forum topic split source category changed concurrently".to_string(),
            ));
        }
        if source.status == TopicStatus::Archived {
            return Err(ForumError::TopicArchived);
        }
        if source.reply_count < 0 {
            return Err(ForumError::Validation(
                "Forum topic split source published reply counter is invalid".to_string(),
            ));
        }
        ensure_category_active_in_tx(&txn, tenant_id, source.category_id).await?;
        ensure_target_topic_absent_in_tx(&txn, prepared.target_topic_id).await?;

        lock_category_tree_in_tx(&txn, tenant_id).await?;
        lock_topic_audience_scopes_in_tx(
            &txn,
            tenant_id,
            &[source_topic_id, prepared.target_topic_id],
        )
        .await?;
        lock_topic_reply_create_scopes_in_tx(
            &txn,
            tenant_id,
            &[source_topic_id, prepared.target_topic_id],
        )
        .await?;

        let selected =
            load_selected_replies_in_tx(&txn, tenant_id, source_topic_id, &prepared.reply_ids)
                .await?;
        validate_split_boundary_in_tx(
            &txn,
            tenant_id,
            source_topic_id,
            &prepared.reply_ids,
            &selected,
        )
        .await?;

        let total_source_reply_count = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(source_topic_id))
            .count(&txn)
            .await?;
        if total_source_reply_count <= selected.len() as u64 {
            return Err(ForumError::Validation(
                "Forum topic split must leave at least one reply in the source topic".to_string(),
            ));
        }

        let source_published_reply_count =
            approved_reply_count_in_tx(&txn, tenant_id, source_topic_id).await?;
        if source_published_reply_count != source.reply_count {
            return Err(ForumError::Validation(
                "Forum topic split source published reply counter is inconsistent".to_string(),
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
                "Forum topic split selected published reply count exceeds supported range"
                    .to_string(),
            )
        })?;
        let source_resulting_published_reply_count = source_published_reply_count
            .checked_sub(moved_published_reply_count)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum topic split source published reply counter underflow".to_string(),
                )
            })?;
        let moved_reply_count = i32::try_from(selected.len()).map_err(|_| {
            ForumError::Validation(
                "Forum topic split selected reply count exceeds supported range".to_string(),
            )
        })?;

        let solution = load_valid_solution_in_tx(&txn, tenant_id, source_topic_id).await?;
        let selected_ids = prepared.reply_ids.iter().copied().collect::<HashSet<_>>();
        let moved_solution = solution
            .as_ref()
            .filter(|solution| selected_ids.contains(&solution.reply_id))
            .cloned();

        let now = Utc::now();
        let target_last_reply_at = selected
            .iter()
            .filter(|reply| reply.status == ReplyStatus::Approved)
            .map(|reply| reply.created_at)
            .max();
        let target = forum_topic::ActiveModel {
            id: Set(prepared.target_topic_id),
            tenant_id: Set(tenant_id),
            category_id: Set(source.category_id),
            author_id: Set(Some(actor_id)),
            status: Set(TopicStatus::Open),
            metadata: Set(json!({})),
            is_pinned: Set(false),
            is_locked: Set(false),
            reply_count: Set(moved_published_reply_count),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            last_reply_at: Set(target_last_reply_at),
        }
        .insert(&txn)
        .await?;
        forum_topic_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            topic_id: Set(prepared.target_topic_id),
            tenant_id: Set(tenant_id),
            locale: Set(prepared.locale.clone()),
            title: Set(prepared.title.clone()),
            slug: Set(prepared.slug.clone()),
            body: Set(prepared.stored_body.clone()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        clone_topic_access_in_tx(&txn, tenant_id, source_topic_id, prepared.target_topic_id)
            .await?;
        validate_cloned_access_in_tx(&txn, tenant_id, &source, &target).await?;

        let reply_audit =
            move_selected_replies_in_tx(&txn, tenant_id, prepared.target_topic_id, selected, now)
                .await?;
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

            validate_cascaded_solution_transfer_in_tx(
                &txn,
                tenant_id,
                source_topic_id,
                prepared.target_topic_id,
                solution,
            )
            .await?;
        }

        let actual_source_published =
            approved_reply_count_in_tx(&txn, tenant_id, source_topic_id).await?;
        let actual_target_published =
            approved_reply_count_in_tx(&txn, tenant_id, prepared.target_topic_id).await?;
        if actual_source_published != source_resulting_published_reply_count
            || actual_target_published != moved_published_reply_count
        {
            return Err(ForumError::Validation(
                "Forum topic split published reply reconciliation failed".to_string(),
            ));
        }
        validate_solution_after_split_in_tx(
            &txn,
            tenant_id,
            source_topic_id,
            prepared.target_topic_id,
            solution.as_ref(),
            moved_solution.is_some(),
        )
        .await?;

        let source_last_reply_at = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(source_topic_id))
            .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
            .order_by_desc(forum_reply::Column::CreatedAt)
            .one(&txn)
            .await?
            .map(|reply| reply.created_at);
        let mut source_active: forum_topic::ActiveModel = source.into();
        source_active.reply_count = Set(source_resulting_published_reply_count);
        source_active.last_reply_at = Set(source_last_reply_at);
        source_active.updated_at = Set(now.into());
        source_active.update(&txn).await?;

        increment_category_topic_count_in_tx(&txn, tenant_id, target.category_id, now).await?;
        UserStatsService::adjust_topic_count_in_tx(&txn, tenant_id, Some(actor_id), 1).await?;

        let solution_reply_id = moved_solution.as_ref().map(|solution| solution.reply_id);
        let payload = topic_split_payload(
            prepared.operation_id,
            source_topic_id,
            prepared.target_topic_id,
            target.category_id,
            actor_id,
            &prepared.reason,
            &prepared.command_fingerprint,
            moved_reply_count,
            moved_published_reply_count,
            source_resulting_published_reply_count,
            solution_reply_id,
        );
        forum_domain_event::ActiveModel {
            sequence_no: NotSet,
            event_id: Set(prepared.operation_id),
            tenant_id: Set(tenant_id),
            aggregate_type: Set(FORUM_TOPIC_SPLIT_AGGREGATE_TYPE.to_string()),
            aggregate_id: Set(prepared.target_topic_id),
            event_type: Set(FORUM_TOPIC_SPLIT_EVENT_TYPE.to_string()),
            schema_version: Set(FORUM_TOPIC_SPLIT_SCHEMA_VERSION),
            actor_id: Set(Some(actor_id)),
            payload: Set(payload),
            created_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        insert_split_operation_in_tx(
            &txn,
            tenant_id,
            prepared.operation_id,
            source_topic_id,
            prepared.target_topic_id,
            target.category_id,
            actor_id,
            &prepared.reason,
            &prepared.command_fingerprint,
            moved_reply_count,
            moved_published_reply_count,
            source_resulting_published_reply_count,
            solution_reply_id,
            now,
        )
        .await?;
        insert_split_reply_audit_in_tx(&txn, tenant_id, prepared.operation_id, &reply_audit)
            .await?;

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
            target.category_id,
        )
        .await?;

        txn.commit().await?;
        Ok(ForumTopicSplitResult {
            operation_id: prepared.operation_id,
            event_id: prepared.operation_id,
            source_topic_id,
            target_topic_id: prepared.target_topic_id,
            category_id: target.category_id,
            actor_id,
            reason: prepared.reason,
            moved_reply_count,
            moved_published_reply_count,
            source_resulting_published_reply_count,
            target_resulting_published_reply_count: moved_published_reply_count,
            solution_reply_id,
            split_at: now,
        })
    }
}

fn prepare_split_input(
    tenant_id: Uuid,
    source_topic_id: Uuid,
    actor_id: Uuid,
    input: SplitForumTopicRepliesInput,
) -> ForumResult<PreparedSplitInput> {
    for (label, value) in [
        ("tenant", tenant_id),
        ("source topic", source_topic_id),
        ("operation", input.operation_id),
        ("target topic", input.target_topic_id),
        ("actor", actor_id),
    ] {
        if value.is_nil() {
            return Err(ForumError::Validation(format!(
                "Forum topic split {label} must not be nil"
            )));
        }
    }
    if source_topic_id == input.target_topic_id {
        return Err(ForumError::Validation(
            "Forum topic split source and target topics must differ".to_string(),
        ));
    }
    if input.reply_ids.is_empty() {
        return Err(ForumError::Validation(
            "Forum topic split requires at least one selected reply".to_string(),
        ));
    }
    if input.reply_ids.len() > MAX_FORUM_TOPIC_SPLIT_REPLIES {
        return Err(ForumError::Validation(format!(
            "Forum topic split must not exceed {MAX_FORUM_TOPIC_SPLIT_REPLIES} selected replies"
        )));
    }
    if input.reply_ids.iter().any(Uuid::is_nil) {
        return Err(ForumError::Validation(
            "Forum topic split selected reply IDs must not be nil".to_string(),
        ));
    }
    let mut reply_ids = input.reply_ids;
    reply_ids.sort();
    if reply_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ForumError::Validation(
            "Forum topic split selected reply IDs must be unique".to_string(),
        ));
    }

    let locale = normalize_locale_code(&input.locale)
        .ok_or_else(|| ForumError::Validation("Forum topic split locale is invalid".to_string()))?;
    let title = input.title.trim().to_string();
    if title.is_empty() {
        return Err(ForumError::Validation(
            "Forum topic split title must not be empty".to_string(),
        ));
    }
    if title.chars().count() > MAX_FORUM_TOPIC_SPLIT_TITLE_LEN {
        return Err(ForumError::Validation(format!(
            "Forum topic split title must not exceed {MAX_FORUM_TOPIC_SPLIT_TITLE_LEN} characters"
        )));
    }
    if title.chars().any(char::is_control) {
        return Err(ForumError::Validation(
            "Forum topic split title must not contain control characters".to_string(),
        ));
    }
    let slug = input
        .slug
        .as_deref()
        .map(normalize_slug)
        .filter(|value| !value.is_empty());
    if slug
        .as_ref()
        .is_some_and(|slug| slug.len() > MAX_FORUM_TOPIC_SPLIT_SLUG_LEN)
    {
        return Err(ForumError::Validation(format!(
            "Forum topic split slug must not exceed {MAX_FORUM_TOPIC_SPLIT_SLUG_LEN} bytes"
        )));
    }

    let reason = input.reason.trim().to_string();
    if reason.is_empty() {
        return Err(ForumError::Validation(
            "Forum topic split reason must not be empty".to_string(),
        ));
    }
    if reason.chars().count() > MAX_FORUM_TOPIC_SPLIT_REASON_LEN {
        return Err(ForumError::Validation(format!(
            "Forum topic split reason must not exceed {MAX_FORUM_TOPIC_SPLIT_REASON_LEN} characters"
        )));
    }
    if reason.chars().any(char::is_control) {
        return Err(ForumError::Validation(
            "Forum topic split reason must not contain control characters".to_string(),
        ));
    }

    let stored_body = serialize_discussion(RichTextDocument::single_paragraph(title.clone()))?;
    let command_fingerprint = fingerprint_command(
        source_topic_id,
        input.target_topic_id,
        &reply_ids,
        &locale,
        &title,
        slug.as_deref(),
        &reason,
    )?;
    Ok(PreparedSplitInput {
        operation_id: input.operation_id,
        target_topic_id: input.target_topic_id,
        reply_ids,
        locale,
        title,
        slug,
        stored_body,
        reason,
        command_fingerprint,
    })
}

fn fingerprint_command(
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    reply_ids: &[Uuid],
    locale: &str,
    title: &str,
    slug: Option<&str>,
    reason: &str,
) -> ForumResult<String> {
    let canonical = serde_json::to_vec(&json!({
        "source_topic_id": source_topic_id,
        "target_topic_id": target_topic_id,
        "reply_ids": reply_ids,
        "locale": locale,
        "title": title,
        "slug": slug,
        "reason": reason,
    }))
    .map_err(|error| {
        ForumError::Validation(format!(
            "Forum topic split command cannot be canonicalized: {error}"
        ))
    })?;
    Ok(hex::encode(Sha256::digest(canonical)))
}

fn normalize_slug(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_dash = false;
    for character in value.chars().flat_map(|character| character.to_lowercase()) {
        if character.is_ascii_alphanumeric() {
            normalized.push(character);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    normalized.trim_matches('-').to_string()
}

async fn lock_topic_split_tenant_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            for (scope, seed) in [
                (format!("forum-topic-split:{tenant_id}"), 21_i32),
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
                INSERT INTO forum_topic_split_locks (tenant_id, touched_at)
                VALUES (?, CURRENT_TIMESTAMP)
                ON CONFLICT(tenant_id) DO UPDATE SET touched_at = CURRENT_TIMESTAMP
                "#,
                vec![tenant_id.into()],
            ))
            .await?;
            Ok(())
        }
        backend => Err(ForumError::Validation(format!(
            "Forum topic split does not support database backend {backend:?}"
        ))),
    }
}

async fn lock_split_counter_scopes_in_tx(
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
            for scope in [
                format!("forum:category:{tenant_id}:{category_id}"),
                format!("forum:topic:{tenant_id}:{}", topic_ids[0]),
                format!("forum:topic:{tenant_id}:{}", topic_ids[1]),
            ] {
                txn.execute(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT forum_counter_lock($1)",
                    vec![scope.into()],
                ))
                .await?;
            }
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(ForumError::Validation(format!(
            "Forum topic split does not support database backend {backend:?}"
        ))),
    }
}

async fn lock_source_topic_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT id FROM forum_topics WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE",
            vec![tenant_id.into(), source_topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT id FROM forum_topics WHERE tenant_id = ? AND id = ? AND deleted_at IS NULL",
            vec![tenant_id.into(), source_topic_id.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic split does not support database backend {backend:?}"
            )));
        }
    };
    if txn.query_one(statement).await?.is_none() {
        return Err(ForumError::TopicNotFound(source_topic_id));
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

async fn ensure_target_topic_absent_in_tx(
    txn: &DatabaseTransaction,
    target_topic_id: Uuid,
) -> ForumResult<()> {
    if forum_topic::Entity::find_by_id(target_topic_id)
        .one(txn)
        .await?
        .is_some()
    {
        return Err(ForumError::Validation(
            "Forum topic split target topic ID already exists".to_string(),
        ));
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
            "Forum topic split requires an active source category".to_string(),
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
            "Forum topic split reply-create locking does not support {backend:?}"
        ))),
    }
}

async fn load_selected_replies_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    reply_ids: &[Uuid],
) -> ForumResult<Vec<forum_reply::Model>> {
    let replies = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(source_topic_id))
        .filter(forum_reply::Column::Id.is_in(reply_ids.to_vec()))
        .order_by_asc(forum_reply::Column::Position)
        .all(txn)
        .await?;
    if replies.len() != reply_ids.len() {
        return Err(ForumError::Validation(
            "Forum topic split selected replies must all belong to the active source topic"
                .to_string(),
        ));
    }
    Ok(replies)
}

async fn validate_split_boundary_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    reply_ids: &[Uuid],
    selected: &[forum_reply::Model],
) -> ForumResult<()> {
    let selected_ids = reply_ids.iter().copied().collect::<HashSet<_>>();
    if selected.iter().any(|reply| {
        reply
            .parent_reply_id
            .is_some_and(|parent_reply_id| !selected_ids.contains(&parent_reply_id))
    }) {
        return Err(ForumError::Validation(
            "Forum topic split cannot detach a selected reply from its parent".to_string(),
        ));
    }
    let crossing_child = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(source_topic_id))
        .filter(forum_reply::Column::ParentReplyId.is_in(reply_ids.to_vec()))
        .filter(forum_reply::Column::Id.is_not_in(reply_ids.to_vec()))
        .one(txn)
        .await?;
    if crossing_child.is_some() {
        return Err(ForumError::Validation(
            "Forum topic split cannot leave a child reply behind its selected parent".to_string(),
        ));
    }
    Ok(())
}

async fn load_valid_solution_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
) -> ForumResult<Option<SplitSolutionCandidate>> {
    let Some(solution) = forum_solution::Entity::find()
        .filter(forum_solution::Column::TenantId.eq(tenant_id))
        .filter(forum_solution::Column::TopicId.eq(source_topic_id))
        .one(txn)
        .await?
    else {
        return Ok(None);
    };
    let reply = forum_reply::Entity::find_by_id(solution.reply_id)
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(source_topic_id))
        .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
        .one(txn)
        .await?;
    if reply.is_none() {
        return Err(ForumError::Validation(
            "Forum topic split requires a valid approved source solution".to_string(),
        ));
    }
    Ok(Some(SplitSolutionCandidate {
        reply_id: solution.reply_id,
        marked_by_user_id: solution.marked_by_user_id,
        marked_at: solution.marked_at,
    }))
}

async fn validate_cascaded_solution_transfer_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    expected: &SplitSolutionCandidate,
) -> ForumResult<()> {
    let source = forum_solution::Entity::find_by_id((source_topic_id, tenant_id))
        .one(txn)
        .await?;
    let target = forum_solution::Entity::find_by_id((target_topic_id, tenant_id))
        .one(txn)
        .await?;
    if source.is_none()
        && target.is_some_and(|target| {
            target.reply_id == expected.reply_id
                && target.marked_by_user_id == expected.marked_by_user_id
                && target.marked_at == expected.marked_at
        })
    {
        Ok(())
    } else {
        Err(ForumError::Validation(
            "Forum topic split solution foreign-key cascade is inconsistent".to_string(),
        ))
    }
}

async fn validate_solution_after_split_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    original: Option<&SplitSolutionCandidate>,
    moved: bool,
) -> ForumResult<()> {
    let source = forum_solution::Entity::find_by_id((source_topic_id, tenant_id))
        .one(txn)
        .await?;
    let target = forum_solution::Entity::find_by_id((target_topic_id, tenant_id))
        .one(txn)
        .await?;
    match (original, moved, source, target) {
        (None, false, None, None) => Ok(()),
        (Some(expected), false, Some(source), None)
            if source.reply_id == expected.reply_id
                && source.marked_by_user_id == expected.marked_by_user_id
                && source.marked_at == expected.marked_at =>
        {
            Ok(())
        }
        (Some(expected), true, None, Some(target))
            if target.reply_id == expected.reply_id
                && target.marked_by_user_id == expected.marked_by_user_id
                && target.marked_at == expected.marked_at =>
        {
            Ok(())
        }
        _ => Err(ForumError::Validation(
            "Forum topic split accepted solution reconciliation failed".to_string(),
        )),
    }
}

async fn move_selected_replies_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    target_topic_id: Uuid,
    selected: Vec<forum_reply::Model>,
    now: DateTime<Utc>,
) -> ForumResult<Vec<SplitReplyAudit>> {
    let mut audit = Vec::with_capacity(selected.len());
    for (index, reply) in selected.into_iter().enumerate() {
        let target_position = i64::try_from(index + 1).map_err(|_| {
            ForumError::Validation(
                "Forum topic split target position exceeds supported range".to_string(),
            )
        })?;
        audit.push(SplitReplyAudit {
            reply_id: reply.id,
            source_position: reply.position,
            target_position,
            was_published: reply.status == ReplyStatus::Approved,
        });
        let mut active: forum_reply::ActiveModel = reply.into();
        active.topic_id = Set(target_topic_id);
        active.position = Set(target_position);
        active.updated_at = Set(now.into());
        active.update(txn).await?;
    }
    let moved_count = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(target_topic_id))
        .count(txn)
        .await?;
    if moved_count != audit.len() as u64 {
        return Err(ForumError::Validation(
            "Forum topic split selected reply movement was partial".to_string(),
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
        ForumError::Validation("Forum published reply count exceeds supported range".to_string())
    })
}

async fn clone_topic_access_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
) -> ForumResult<()> {
    let specifications = [
        (
            "forum_topic_channel_access",
            "tenant_id, topic_id, channel_slug",
            "channel_slug",
        ),
        (
            "forum_topic_audience_policies",
            "tenant_id, topic_id, minimum_trust_level, updated_at",
            "minimum_trust_level, CURRENT_TIMESTAMP",
        ),
        (
            "forum_topic_audience_roles",
            "tenant_id, topic_id, role",
            "role",
        ),
        (
            "forum_topic_audience_channels",
            "tenant_id, topic_id, channel_slug",
            "channel_slug",
        ),
        (
            "forum_topic_audience_groups",
            "tenant_id, topic_id, group_id",
            "group_id",
        ),
        (
            "forum_topic_audience_users",
            "tenant_id, topic_id, user_id, effect",
            "user_id, effect",
        ),
        (
            "forum_topic_reply_create_audience_policies",
            "tenant_id, topic_id, minimum_trust_level, updated_at",
            "minimum_trust_level, CURRENT_TIMESTAMP",
        ),
        (
            "forum_topic_reply_create_audience_roles",
            "tenant_id, topic_id, role",
            "role",
        ),
        (
            "forum_topic_reply_create_audience_channels",
            "tenant_id, topic_id, channel_slug",
            "channel_slug",
        ),
        (
            "forum_topic_reply_create_audience_groups",
            "tenant_id, topic_id, group_id",
            "group_id",
        ),
        (
            "forum_topic_reply_create_audience_users",
            "tenant_id, topic_id, user_id, effect",
            "user_id, effect",
        ),
    ];
    let backend = txn.get_database_backend();
    let placeholders = match backend {
        DatabaseBackend::Postgres => ("$1", "$2", "$3"),
        DatabaseBackend::Sqlite => ("?", "?", "?"),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic split access cloning does not support {backend:?}"
            )));
        }
    };
    for (table, insert_columns, selected_columns) in specifications {
        let sql = format!(
            "INSERT INTO {table} ({insert_columns}) SELECT tenant_id, {}, {selected_columns} FROM {table} WHERE tenant_id = {} AND topic_id = {}",
            placeholders.0, placeholders.1, placeholders.2
        );
        txn.execute(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                target_topic_id.into(),
                tenant_id.into(),
                source_topic_id.into(),
            ],
        ))
        .await?;
    }
    Ok(())
}

async fn validate_cloned_access_in_tx(
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
            "Forum topic split visibility policy clone is inconsistent".to_string(),
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
            "Forum topic split reply-create policy clone is inconsistent".to_string(),
        ));
    }
    let source_channels = load_topic_channels_in_tx(txn, tenant_id, source.id).await?;
    let target_channels = load_topic_channels_in_tx(txn, tenant_id, target.id).await?;
    if source_channels != target_channels {
        return Err(ForumError::Validation(
            "Forum topic split channel access clone is inconsistent".to_string(),
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

async fn increment_category_topic_count_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    now: DateTime<Utc>,
) -> ForumResult<()> {
    let category = forum_category::Entity::find_by_id(category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(ForumError::CategoryNotFound(category_id))?;
    if category.topic_count < 0 || category.reply_count < 0 {
        return Err(ForumError::Validation(
            "Forum topic split category counters are inconsistent".to_string(),
        ));
    }
    let topic_count = category.topic_count.checked_add(1).ok_or_else(|| {
        ForumError::Validation("Forum topic split category topic counter overflow".to_string())
    })?;
    let mut active: forum_category::ActiveModel = category.into();
    active.topic_count = Set(topic_count);
    active.updated_at = Set(now.into());
    active.update(txn).await?;
    Ok(())
}

async fn insert_split_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    category_id: Uuid,
    actor_id: Uuid,
    reason: &str,
    command_fingerprint: &str,
    moved_reply_count: i32,
    moved_published_reply_count: i32,
    source_resulting_published_reply_count: i32,
    solution_reply_id: Option<Uuid>,
    now: DateTime<Utc>,
) -> ForumResult<()> {
    let (sql, backend) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            r#"
            INSERT INTO forum_topic_split_operations (
                tenant_id, operation_id, source_topic_id, target_topic_id, category_id,
                actor_id, reason, command_fingerprint, moved_reply_count,
                moved_published_reply_count, source_resulting_published_reply_count,
                target_resulting_published_reply_count, solution_reply_id, event_id, split_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
            DatabaseBackend::Postgres,
        ),
        DatabaseBackend::Sqlite => (
            r#"
            INSERT INTO forum_topic_split_operations (
                tenant_id, operation_id, source_topic_id, target_topic_id, category_id,
                actor_id, reason, command_fingerprint, moved_reply_count,
                moved_published_reply_count, source_resulting_published_reply_count,
                target_resulting_published_reply_count, solution_reply_id, event_id, split_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            DatabaseBackend::Sqlite,
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic split receipt does not support {backend:?}"
            )));
        }
    };
    txn.execute(Statement::from_sql_and_values(
        backend,
        sql,
        vec![
            tenant_id.into(),
            operation_id.into(),
            source_topic_id.into(),
            target_topic_id.into(),
            category_id.into(),
            actor_id.into(),
            reason.to_string().into(),
            command_fingerprint.to_string().into(),
            moved_reply_count.into(),
            moved_published_reply_count.into(),
            source_resulting_published_reply_count.into(),
            moved_published_reply_count.into(),
            solution_reply_id.into(),
            operation_id.into(),
            now.into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn insert_split_reply_audit_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_id: Uuid,
    audit: &[SplitReplyAudit],
) -> ForumResult<()> {
    let (sql, backend) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            "INSERT INTO forum_topic_split_reply_items (tenant_id, operation_id, reply_id, source_position, target_position, was_published) VALUES ($1, $2, $3, $4, $5, $6)",
            DatabaseBackend::Postgres,
        ),
        DatabaseBackend::Sqlite => (
            "INSERT INTO forum_topic_split_reply_items (tenant_id, operation_id, reply_id, source_position, target_position, was_published) VALUES (?, ?, ?, ?, ?, ?)",
            DatabaseBackend::Sqlite,
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic split reply audit does not support {backend:?}"
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
                item.source_position.into(),
                item.target_position.into(),
                item.was_published.into(),
            ],
        ))
        .await?;
    }
    Ok(())
}

async fn load_split_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> ForumResult<Option<StoredSplitOperation>> {
    let (sql, backend) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            "SELECT * FROM forum_topic_split_operations WHERE tenant_id = $1 AND operation_id = $2",
            DatabaseBackend::Postgres,
        ),
        DatabaseBackend::Sqlite => (
            "SELECT * FROM forum_topic_split_operations WHERE tenant_id = ? AND operation_id = ?",
            DatabaseBackend::Sqlite,
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic split receipt lookup does not support {backend:?}"
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

fn stored_operation_from_row(row: QueryResult) -> Result<StoredSplitOperation, sea_orm::DbErr> {
    Ok(StoredSplitOperation {
        tenant_id: row.try_get("", "tenant_id")?,
        operation_id: row.try_get("", "operation_id")?,
        source_topic_id: row.try_get("", "source_topic_id")?,
        target_topic_id: row.try_get("", "target_topic_id")?,
        category_id: row.try_get("", "category_id")?,
        actor_id: row.try_get("", "actor_id")?,
        reason: row.try_get("", "reason")?,
        command_fingerprint: row.try_get("", "command_fingerprint")?,
        moved_reply_count: row.try_get("", "moved_reply_count")?,
        moved_published_reply_count: row.try_get("", "moved_published_reply_count")?,
        source_resulting_published_reply_count: row
            .try_get("", "source_resulting_published_reply_count")?,
        target_resulting_published_reply_count: row
            .try_get("", "target_resulting_published_reply_count")?,
        solution_reply_id: row.try_get("", "solution_reply_id")?,
        event_id: row.try_get("", "event_id")?,
        split_at: row.try_get("", "split_at")?,
    })
}

async fn validate_replay_in_tx(
    txn: &DatabaseTransaction,
    existing: &StoredSplitOperation,
    source_topic_id: Uuid,
    actor_id: Uuid,
    prepared: &PreparedSplitInput,
) -> ForumResult<()> {
    if existing.source_topic_id != source_topic_id
        || existing.target_topic_id != prepared.target_topic_id
        || existing.actor_id != actor_id
        || existing.reason != prepared.reason
        || existing.command_fingerprint != prepared.command_fingerprint
    {
        return Err(ForumError::Validation(format!(
            "Forum topic split operation conflicts with existing command: {}",
            prepared.operation_id
        )));
    }
    let audit_count =
        split_reply_audit_count_in_tx(txn, existing.tenant_id, existing.operation_id).await?;
    if audit_count != i64::from(existing.moved_reply_count) {
        return Err(ForumError::Validation(
            "Forum topic split immutable reply audit is inconsistent".to_string(),
        ));
    }
    validate_existing_semantic_event_in_tx(txn, existing).await
}

async fn split_reply_audit_count_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> ForumResult<i64> {
    let (sql, backend) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            "SELECT COUNT(*) AS count FROM forum_topic_split_reply_items WHERE tenant_id = $1 AND operation_id = $2",
            DatabaseBackend::Postgres,
        ),
        DatabaseBackend::Sqlite => (
            "SELECT COUNT(*) AS count FROM forum_topic_split_reply_items WHERE tenant_id = ? AND operation_id = ?",
            DatabaseBackend::Sqlite,
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic split audit lookup does not support {backend:?}"
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
            ForumError::Validation("Forum topic split audit count is unavailable".to_string())
        })?;
    Ok(row.try_get("", "count")?)
}

async fn validate_existing_semantic_event_in_tx(
    txn: &DatabaseTransaction,
    operation: &StoredSplitOperation,
) -> ForumResult<()> {
    let event = forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::EventId.eq(operation.event_id))
        .filter(forum_domain_event::Column::TenantId.eq(operation.tenant_id))
        .one(txn)
        .await?
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum topic split immutable semantic event is missing".to_string(),
            )
        })?;
    let expected_payload = topic_split_payload(
        operation.operation_id,
        operation.source_topic_id,
        operation.target_topic_id,
        operation.category_id,
        operation.actor_id,
        &operation.reason,
        &operation.command_fingerprint,
        operation.moved_reply_count,
        operation.moved_published_reply_count,
        operation.source_resulting_published_reply_count,
        operation.solution_reply_id,
    );
    if event.aggregate_type != FORUM_TOPIC_SPLIT_AGGREGATE_TYPE
        || event.aggregate_id != operation.target_topic_id
        || event.event_type != FORUM_TOPIC_SPLIT_EVENT_TYPE
        || event.schema_version != FORUM_TOPIC_SPLIT_SCHEMA_VERSION
        || event.actor_id != Some(operation.actor_id)
        || event.payload != expected_payload
    {
        return Err(ForumError::Validation(
            "Forum topic split immutable semantic event is inconsistent".to_string(),
        ));
    }
    Ok(())
}

fn topic_split_payload(
    operation_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    category_id: Uuid,
    actor_id: Uuid,
    reason: &str,
    command_fingerprint: &str,
    moved_reply_count: i32,
    moved_published_reply_count: i32,
    source_resulting_published_reply_count: i32,
    solution_reply_id: Option<Uuid>,
) -> JsonValue {
    json!({
        "operation_id": operation_id,
        "source_topic_id": source_topic_id,
        "target_topic_id": target_topic_id,
        "category_id": category_id,
        "actor_id": actor_id,
        "reason": reason,
        "command_fingerprint": command_fingerprint,
        "moved_reply_count": moved_reply_count,
        "moved_published_reply_count": moved_published_reply_count,
        "source_resulting_published_reply_count": source_resulting_published_reply_count,
        "target_resulting_published_reply_count": moved_published_reply_count,
        "solution_reply_id": solution_reply_id,
    })
}

fn operation_to_result(operation: StoredSplitOperation) -> ForumTopicSplitResult {
    ForumTopicSplitResult {
        operation_id: operation.operation_id,
        event_id: operation.event_id,
        source_topic_id: operation.source_topic_id,
        target_topic_id: operation.target_topic_id,
        category_id: operation.category_id,
        actor_id: operation.actor_id,
        reason: operation.reason,
        moved_reply_count: operation.moved_reply_count,
        moved_published_reply_count: operation.moved_published_reply_count,
        source_resulting_published_reply_count: operation.source_resulting_published_reply_count,
        target_resulting_published_reply_count: operation.target_resulting_published_reply_count,
        solution_reply_id: operation.solution_reply_id,
        split_at: operation.split_at.with_timezone(&Utc),
    }
}
