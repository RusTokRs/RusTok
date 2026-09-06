use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Utc};
use rustok_api::{Action, Resource, RichTextDocument};
use rustok_content::normalize_locale_code;
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
    forum_audience_mention, forum_category, forum_category_lifecycle, forum_domain_event,
    forum_quote, forum_relation_revision, forum_reply, forum_reply_body, forum_reply_revision,
    forum_solution, forum_topic, forum_topic_channel_access, forum_topic_tag,
    forum_topic_translation, forum_user_mention,
};
use crate::error::{ForumError, ForumResult};
use crate::richtext::serialize_discussion;
use crate::state_machine::{ReplyStatus, TopicStatus};

use super::category_audience::lock_category_tree_in_tx;
use super::projection_invalidation::{
    publish_forum_category_projection_in_tx, publish_forum_topic_projection_in_tx,
};
use super::rbac::enforce_scope;
use super::topic::MAX_FORUM_TOPIC_TAGS;
use super::topic_audience::load_policy_for_topic;
use super::topic_audience_lock::lock_topic_audience_scopes_in_tx;
use super::topic_reply_create_audience::load_topic_reply_create_audience_policy_for_topic;
use super::topic_solution_lock::lock_topic_solution_scopes_in_tx;
use super::topic_tag_lock::lock_topic_tag_scopes_in_tx;
use super::user_stats::UserStatsService;

pub const MAX_FORUM_TOPIC_FORK_REASON_LEN: usize = 500;
pub const MAX_FORUM_TOPIC_FORK_REPLIES: usize = 500;
pub const MAX_FORUM_TOPIC_FORK_TITLE_LEN: usize = 500;
pub const MAX_FORUM_TOPIC_FORK_BODY_ROWS: usize = 2_000;
pub const MAX_FORUM_TOPIC_FORK_REPLY_REVISIONS: usize = 5_000;
pub const MAX_FORUM_TOPIC_FORK_RELATION_REVISIONS: usize = 5_000;
pub const MAX_FORUM_TOPIC_FORK_MENTIONS: usize = 10_000;
pub const MAX_FORUM_TOPIC_FORK_QUOTES: usize = 5_000;
const MAX_FORUM_TOPIC_FORK_SLUG_LEN: usize = 255;
const FORUM_TOPIC_FORK_EVENT_TYPE: &str = "forum.topic.forked";
const FORUM_TOPIC_FORK_AGGREGATE_TYPE: &str = "forum_topic";
const FORUM_TOPIC_FORK_SCHEMA_VERSION: i16 = 1;
const FORUM_TOPIC_FORK_REPLY_ID_DOMAIN: &[u8] = b"forum-topic-fork-reply-v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForkForumReplyBranchInput {
    pub operation_id: Uuid,
    pub target_topic_id: Uuid,
    pub root_reply_id: Uuid,
    pub locale: String,
    pub title: String,
    pub slug: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumTopicForkResult {
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub source_topic_id: Uuid,
    pub target_topic_id: Uuid,
    pub root_reply_id: Uuid,
    pub category_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub copied_reply_count: i32,
    pub copied_published_reply_count: i32,
    pub copied_body_count: i32,
    pub copied_reply_revision_count: i32,
    pub copied_relation_revision_count: i32,
    pub copied_mention_count: i32,
    pub copied_quote_count: i32,
    pub forked_at: DateTime<Utc>,
}

struct PreparedForkInput {
    operation_id: Uuid,
    target_topic_id: Uuid,
    root_reply_id: Uuid,
    locale: String,
    title: String,
    slug: Option<String>,
    stored_body: String,
    reason: String,
    command_fingerprint: String,
}

struct ForkSnapshot {
    replies: Vec<forum_reply::Model>,
    bodies: Vec<forum_reply_body::Model>,
    reply_revisions: Vec<forum_reply_revision::Model>,
    relation_revisions: Vec<forum_relation_revision::Model>,
    user_mentions: Vec<forum_user_mention::Model>,
    audience_mentions: Vec<forum_audience_mention::Model>,
    quotes: Vec<forum_quote::Model>,
}

struct ForkReplyAudit {
    source_reply_id: Uuid,
    target_reply_id: Uuid,
    source_parent_reply_id: Option<Uuid>,
    target_parent_reply_id: Option<Uuid>,
    source_position: i64,
    target_position: i64,
    was_published: bool,
}

struct ForkRevisionAudit {
    revision_kind: &'static str,
    source_revision_id: i64,
    target_revision_id: i64,
    source_reply_id: Uuid,
    target_reply_id: Uuid,
    locale: String,
}

struct StoredForkOperation {
    tenant_id: Uuid,
    operation_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    root_reply_id: Uuid,
    category_id: Uuid,
    actor_id: Uuid,
    reason: String,
    command_fingerprint: String,
    copied_reply_count: i32,
    copied_published_reply_count: i32,
    copied_body_count: i32,
    copied_reply_revision_count: i32,
    copied_relation_revision_count: i32,
    copied_mention_count: i32,
    copied_quote_count: i32,
    event_id: Uuid,
    forked_at: DateTimeWithTimeZone,
}

/// Idempotently copies one bounded reply subtree into a new same-category topic.
///
/// Source rows are immutable inputs. Copied replies receive deterministic new UUIDs, the copied
/// root is detached, descendants point only at copied parents, and all source ordering must remain
/// parent-before-child. Current localized bodies plus complete reply/relation revision history are
/// copied. Mention projections are copied without notification events. Quote targets intentionally
/// retain their original immutable IDs and revision IDs. Votes, subscriptions, read states and the
/// accepted solution remain source-only. Exact replay returns the immutable receipt.
pub struct ForumTopicForkService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ForumTopicForkService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, security, input))]
    pub async fn fork_reply_branch(
        &self,
        tenant_id: Uuid,
        source_topic_id: Uuid,
        security: SecurityContext,
        input: ForkForumReplyBranchInput,
    ) -> ForumResult<ForumTopicForkResult> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        let actor_id = security.user_id.ok_or_else(|| {
            ForumError::Validation("Forum topic fork requires a human actor".to_string())
        })?;
        let prepared = prepare_fork_input(tenant_id, source_topic_id, actor_id, input)?;

        let txn = self.db.begin().await?;
        lock_topic_fork_tenant_in_tx(&txn, tenant_id).await?;
        if let Some(existing) =
            load_fork_operation_in_tx(&txn, tenant_id, prepared.operation_id).await?
        {
            validate_replay_in_tx(&txn, &existing, source_topic_id, actor_id, &prepared).await?;
            txn.commit().await?;
            return Ok(operation_to_result(existing));
        }

        let preliminary_source = find_topic_in_tx(&txn, tenant_id, source_topic_id).await?;
        lock_fork_counter_scopes_in_tx(
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
                "Forum topic fork source category changed concurrently".to_string(),
            ));
        }
        if source.status == TopicStatus::Archived {
            return Err(ForumError::TopicArchived);
        }
        if source.reply_count < 0 {
            return Err(ForumError::Validation(
                "Forum topic fork source published reply counter is invalid".to_string(),
            ));
        }
        ensure_category_active_in_tx(&txn, tenant_id, source.category_id).await?;
        ensure_target_topic_absent_in_tx(&txn, prepared.target_topic_id).await?;

        lock_category_tree_in_tx(&txn, tenant_id).await?;
        let topic_pair = [source_topic_id, prepared.target_topic_id];
        lock_topic_audience_scopes_in_tx(&txn, tenant_id, &topic_pair).await?;
        lock_topic_reply_create_scopes_in_tx(&txn, tenant_id, &topic_pair).await?;
        lock_topic_solution_scopes_in_tx(&txn, tenant_id, &topic_pair).await?;
        lock_topic_tag_scopes_in_tx(&txn, tenant_id, &topic_pair).await?;

        let first_branch_ids =
            load_reply_branch_ids_in_tx(&txn, tenant_id, source_topic_id, prepared.root_reply_id)
                .await?;
        lock_reply_rows_in_tx(&txn, tenant_id, &first_branch_ids).await?;
        let branch_ids =
            load_reply_branch_ids_in_tx(&txn, tenant_id, source_topic_id, prepared.root_reply_id)
                .await?;
        if first_branch_ids != branch_ids {
            return Err(ForumError::Validation(
                "Forum topic fork reply branch changed concurrently".to_string(),
            ));
        }

        let snapshot = load_fork_snapshot_in_tx(
            &txn,
            tenant_id,
            source_topic_id,
            prepared.root_reply_id,
            &branch_ids,
        )
        .await?;
        lock_fork_author_scopes_in_tx(&txn, tenant_id, &snapshot.replies).await?;
        let source_solution =
            load_valid_source_solution_in_tx(&txn, tenant_id, source_topic_id).await?;
        let reply_id_map = derive_reply_id_map(prepared.operation_id, &snapshot.replies)?;
        ensure_target_reply_ids_absent_in_tx(&txn, &reply_id_map).await?;

        let copied_reply_count = checked_i32(snapshot.replies.len(), "copied reply")?;
        let copied_published_reply_count = checked_i32(
            snapshot
                .replies
                .iter()
                .filter(|reply| reply.status == ReplyStatus::Approved)
                .count(),
            "copied published reply",
        )?;
        let copied_body_count = checked_i32(snapshot.bodies.len(), "copied body")?;
        let copied_reply_revision_count =
            checked_i32(snapshot.reply_revisions.len(), "copied reply revision")?;
        let copied_relation_revision_count = checked_i32(
            snapshot.relation_revisions.len(),
            "copied relation revision",
        )?;
        let copied_mention_count = checked_i32(
            snapshot.user_mentions.len() + snapshot.audience_mentions.len(),
            "copied mention",
        )?;
        let copied_quote_count = checked_i32(snapshot.quotes.len(), "copied quote")?;

        let now = Utc::now();
        let target =
            create_target_topic_in_tx(&txn, tenant_id, &source, actor_id, &prepared, now).await?;
        clone_topic_access_in_tx(&txn, tenant_id, source_topic_id, prepared.target_topic_id)
            .await?;
        clone_topic_tags_in_tx(
            &txn,
            tenant_id,
            source_topic_id,
            prepared.target_topic_id,
            now,
        )
        .await?;
        validate_cloned_topic_shape_in_tx(&txn, tenant_id, &source, &target).await?;

        let reply_audit = copy_reply_rows_in_tx(
            &txn,
            tenant_id,
            prepared.target_topic_id,
            prepared.root_reply_id,
            &snapshot.replies,
            &reply_id_map,
        )
        .await?;
        copy_reply_bodies_in_tx(&txn, tenant_id, &snapshot.bodies, &reply_id_map).await?;
        let mut revision_audit =
            copy_reply_revisions_in_tx(&txn, tenant_id, &snapshot.reply_revisions, &reply_id_map)
                .await?;
        let (relation_revision_map, relation_audit) = copy_relation_revisions_in_tx(
            &txn,
            tenant_id,
            &snapshot.relation_revisions,
            &reply_id_map,
        )
        .await?;
        revision_audit.extend(relation_audit);
        copy_relation_children_in_tx(
            &txn,
            tenant_id,
            &snapshot,
            &reply_id_map,
            &relation_revision_map,
        )
        .await?;

        let target_last_reply_at = snapshot
            .replies
            .iter()
            .filter(|reply| reply.status == ReplyStatus::Approved)
            .map(|reply| reply.created_at)
            .max();
        let target = reconcile_target_topic_in_tx(
            &txn,
            target,
            copied_published_reply_count,
            target_last_reply_at,
            now,
        )
        .await?;
        increment_category_counters_in_tx(
            &txn,
            tenant_id,
            target.category_id,
            copied_published_reply_count,
            now,
        )
        .await?;
        UserStatsService::adjust_topic_count_in_tx(&txn, tenant_id, Some(actor_id), 1).await?;
        adjust_copied_reply_author_stats_in_tx(&txn, tenant_id, &snapshot.replies).await?;
        validate_source_unchanged_in_tx(&txn, tenant_id, &source, source_solution.as_ref()).await?;
        validate_target_solution_absent_in_tx(&txn, tenant_id, prepared.target_topic_id).await?;

        let payload = topic_fork_payload(
            prepared.operation_id,
            source_topic_id,
            prepared.target_topic_id,
            prepared.root_reply_id,
            target.category_id,
            actor_id,
            &prepared.reason,
            &prepared.command_fingerprint,
            copied_reply_count,
            copied_published_reply_count,
            copied_body_count,
            copied_reply_revision_count,
            copied_relation_revision_count,
            copied_mention_count,
            copied_quote_count,
        );
        forum_domain_event::ActiveModel {
            sequence_no: NotSet,
            event_id: Set(prepared.operation_id),
            tenant_id: Set(tenant_id),
            aggregate_type: Set(FORUM_TOPIC_FORK_AGGREGATE_TYPE.to_string()),
            aggregate_id: Set(prepared.target_topic_id),
            event_type: Set(FORUM_TOPIC_FORK_EVENT_TYPE.to_string()),
            schema_version: Set(FORUM_TOPIC_FORK_SCHEMA_VERSION),
            actor_id: Set(Some(actor_id)),
            payload: Set(payload),
            created_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        insert_fork_operation_in_tx(
            &txn,
            tenant_id,
            prepared.operation_id,
            source_topic_id,
            prepared.target_topic_id,
            prepared.root_reply_id,
            target.category_id,
            actor_id,
            &prepared.reason,
            &prepared.command_fingerprint,
            copied_reply_count,
            copied_published_reply_count,
            copied_body_count,
            copied_reply_revision_count,
            copied_relation_revision_count,
            copied_mention_count,
            copied_quote_count,
            now,
        )
        .await?;
        insert_fork_reply_audit_in_tx(&txn, tenant_id, prepared.operation_id, &reply_audit, now)
            .await?;
        insert_fork_revision_audit_in_tx(
            &txn,
            tenant_id,
            prepared.operation_id,
            &revision_audit,
            now,
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
        Ok(ForumTopicForkResult {
            operation_id: prepared.operation_id,
            event_id: prepared.operation_id,
            source_topic_id,
            target_topic_id: prepared.target_topic_id,
            root_reply_id: prepared.root_reply_id,
            category_id: target.category_id,
            actor_id,
            reason: prepared.reason,
            copied_reply_count,
            copied_published_reply_count,
            copied_body_count,
            copied_reply_revision_count,
            copied_relation_revision_count,
            copied_mention_count,
            copied_quote_count,
            forked_at: now,
        })
    }
}

fn prepare_fork_input(
    tenant_id: Uuid,
    source_topic_id: Uuid,
    actor_id: Uuid,
    input: ForkForumReplyBranchInput,
) -> ForumResult<PreparedForkInput> {
    for (label, value) in [
        ("tenant", tenant_id),
        ("source topic", source_topic_id),
        ("operation", input.operation_id),
        ("target topic", input.target_topic_id),
        ("root reply", input.root_reply_id),
        ("actor", actor_id),
    ] {
        if value.is_nil() {
            return Err(ForumError::Validation(format!(
                "Forum topic fork {label} must not be nil"
            )));
        }
    }
    if source_topic_id == input.target_topic_id {
        return Err(ForumError::Validation(
            "Forum topic fork source and target topics must differ".to_string(),
        ));
    }
    let locale = normalize_locale_code(&input.locale)
        .ok_or_else(|| ForumError::Validation("Forum topic fork locale is invalid".to_string()))?;
    let title = input.title.trim().to_string();
    validate_bounded_text(&title, MAX_FORUM_TOPIC_FORK_TITLE_LEN, "title")?;
    let slug = input
        .slug
        .as_deref()
        .map(normalize_slug)
        .filter(|value| !value.is_empty());
    if slug
        .as_ref()
        .is_some_and(|slug| slug.len() > MAX_FORUM_TOPIC_FORK_SLUG_LEN)
    {
        return Err(ForumError::Validation(format!(
            "Forum topic fork slug must not exceed {MAX_FORUM_TOPIC_FORK_SLUG_LEN} bytes"
        )));
    }
    let reason = input.reason.trim().to_string();
    validate_bounded_text(&reason, MAX_FORUM_TOPIC_FORK_REASON_LEN, "reason")?;
    let stored_body = serialize_discussion(RichTextDocument::single_paragraph(title.clone()))?;
    let command_fingerprint = fingerprint_command(
        source_topic_id,
        input.target_topic_id,
        input.root_reply_id,
        &locale,
        &title,
        slug.as_deref(),
        &reason,
    )?;
    Ok(PreparedForkInput {
        operation_id: input.operation_id,
        target_topic_id: input.target_topic_id,
        root_reply_id: input.root_reply_id,
        locale,
        title,
        slug,
        stored_body,
        reason,
        command_fingerprint,
    })
}

fn validate_bounded_text(value: &str, maximum: usize, label: &str) -> ForumResult<()> {
    if value.is_empty() {
        return Err(ForumError::Validation(format!(
            "Forum topic fork {label} must not be empty"
        )));
    }
    if value.chars().count() > maximum {
        return Err(ForumError::Validation(format!(
            "Forum topic fork {label} must not exceed {maximum} characters"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(ForumError::Validation(format!(
            "Forum topic fork {label} must not contain control characters"
        )));
    }
    Ok(())
}

fn fingerprint_command(
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    root_reply_id: Uuid,
    locale: &str,
    title: &str,
    slug: Option<&str>,
    reason: &str,
) -> ForumResult<String> {
    let canonical = serde_json::to_vec(&json!({
        "source_topic_id": source_topic_id,
        "target_topic_id": target_topic_id,
        "root_reply_id": root_reply_id,
        "locale": locale,
        "title": title,
        "slug": slug,
        "reason": reason,
        "reply_identity_policy": "sha256_uuid_v5_bits",
        "quote_identity_policy": "preserve_original_targets",
        "solution_policy": "source_only_not_copied",
    }))
    .map_err(|error| {
        ForumError::Validation(format!(
            "Forum topic fork command cannot be canonicalized: {error}"
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

fn derive_target_reply_id(operation_id: Uuid, source_reply_id: Uuid) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(FORUM_TOPIC_FORK_REPLY_ID_DOMAIN);
    hasher.update(operation_id.as_bytes());
    hasher.update(source_reply_id.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn derive_reply_id_map(
    operation_id: Uuid,
    replies: &[forum_reply::Model],
) -> ForumResult<HashMap<Uuid, Uuid>> {
    let mut mapping = HashMap::with_capacity(replies.len());
    let mut target_ids = HashSet::with_capacity(replies.len());
    for reply in replies {
        let target_id = derive_target_reply_id(operation_id, reply.id);
        if target_id.is_nil()
            || target_id == reply.id
            || !target_ids.insert(target_id)
            || mapping.insert(reply.id, target_id).is_some()
        {
            return Err(ForumError::Validation(
                "Forum topic fork deterministic reply identity mapping is invalid".to_string(),
            ));
        }
    }
    Ok(mapping)
}

fn checked_i32(value: usize, label: &str) -> ForumResult<i32> {
    i32::try_from(value).map_err(|_| {
        ForumError::Validation(format!(
            "Forum topic fork {label} count exceeds supported range"
        ))
    })
}

async fn load_reply_branch_ids_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    root_reply_id: Uuid,
) -> ForumResult<Vec<Uuid>> {
    let limit = i64::try_from(MAX_FORUM_TOPIC_FORK_REPLIES + 1).map_err(|_| {
        ForumError::Validation("Forum topic fork branch limit is invalid".to_string())
    })?;
    let (backend, sql, values) = match txn.get_database_backend() {
        DatabaseBackend::Postgres => (
            DatabaseBackend::Postgres,
            r#"
            WITH RECURSIVE branch(id) AS (
                SELECT id FROM forum_replies
                WHERE tenant_id = $1 AND topic_id = $2 AND id = $3
                UNION
                SELECT child.id FROM forum_replies child
                JOIN branch parent ON child.parent_reply_id = parent.id
                WHERE child.tenant_id = $1 AND child.topic_id = $2
            )
            SELECT id FROM branch ORDER BY id LIMIT $4
            "#,
            vec![
                tenant_id.into(),
                source_topic_id.into(),
                root_reply_id.into(),
                limit.into(),
            ],
        ),
        DatabaseBackend::Sqlite => (
            DatabaseBackend::Sqlite,
            r#"
            WITH RECURSIVE branch(id) AS (
                SELECT id FROM forum_replies
                WHERE tenant_id = ? AND topic_id = ? AND id = ?
                UNION
                SELECT child.id FROM forum_replies child
                JOIN branch parent ON child.parent_reply_id = parent.id
                WHERE child.tenant_id = ? AND child.topic_id = ?
            )
            SELECT id FROM branch ORDER BY id LIMIT ?
            "#,
            vec![
                tenant_id.into(),
                source_topic_id.into(),
                root_reply_id.into(),
                tenant_id.into(),
                source_topic_id.into(),
                limit.into(),
            ],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum topic fork branch traversal does not support {backend:?}"
            )));
        }
    };
    let rows = txn
        .query_all_raw(Statement::from_sql_and_values(backend, sql, values))
        .await?;
    if rows.is_empty() {
        return Err(ForumError::ReplyNotFound(root_reply_id));
    }
    if rows.len() > MAX_FORUM_TOPIC_FORK_REPLIES {
        return Err(ForumError::Validation(format!(
            "Forum topic fork branch must not exceed {MAX_FORUM_TOPIC_FORK_REPLIES} replies"
        )));
    }
    rows.into_iter()
        .map(|row| row.try_get("", "id").map_err(ForumError::from))
        .collect()
}

async fn load_fork_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
    root_reply_id: Uuid,
    reply_ids: &[Uuid],
) -> ForumResult<ForkSnapshot> {
    let replies = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::TopicId.eq(source_topic_id))
        .filter(forum_reply::Column::Id.is_in(reply_ids.to_vec()))
        .order_by_asc(forum_reply::Column::Position)
        .all(txn)
        .await?;
    if replies.len() != reply_ids.len() {
        return Err(ForumError::Validation(
            "Forum topic fork branch reply set is inconsistent".to_string(),
        ));
    }
    let branch_ids = replies.iter().map(|reply| reply.id).collect::<HashSet<_>>();
    if !branch_ids.contains(&root_reply_id) {
        return Err(ForumError::ReplyNotFound(root_reply_id));
    }
    let positions = replies
        .iter()
        .map(|reply| (reply.id, reply.position))
        .collect::<HashMap<_, _>>();
    for reply in &replies {
        if reply.id == root_reply_id {
            continue;
        }
        let parent_id = reply.parent_reply_id.ok_or_else(|| {
            ForumError::Validation(
                "Forum topic fork descendant reply is missing its parent".to_string(),
            )
        })?;
        if !branch_ids.contains(&parent_id) {
            return Err(ForumError::Validation(
                "Forum topic fork branch contains an external descendant parent".to_string(),
            ));
        }
        let parent_position = positions.get(&parent_id).copied().ok_or_else(|| {
            ForumError::Validation("Forum topic fork parent position is unavailable".to_string())
        })?;
        if parent_position >= reply.position {
            return Err(ForumError::Validation(
                "Forum topic fork requires parent-before-child reply positions".to_string(),
            ));
        }
    }

    let bodies = forum_reply_body::Entity::find()
        .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
        .filter(forum_reply_body::Column::ReplyId.is_in(reply_ids.to_vec()))
        .limit((MAX_FORUM_TOPIC_FORK_BODY_ROWS + 1) as u64)
        .all(txn)
        .await?;
    ensure_bound(
        bodies.len(),
        MAX_FORUM_TOPIC_FORK_BODY_ROWS,
        "localized body rows",
    )?;
    let body_reply_ids = bodies
        .iter()
        .map(|body| body.reply_id)
        .collect::<HashSet<_>>();
    if body_reply_ids.len() != replies.len() {
        return Err(ForumError::Validation(
            "Forum topic fork requires a current body for every copied reply".to_string(),
        ));
    }

    let reply_revisions = forum_reply_revision::Entity::find()
        .filter(forum_reply_revision::Column::TenantId.eq(tenant_id))
        .filter(forum_reply_revision::Column::ReplyId.is_in(reply_ids.to_vec()))
        .order_by_asc(forum_reply_revision::Column::Id)
        .limit((MAX_FORUM_TOPIC_FORK_REPLY_REVISIONS + 1) as u64)
        .all(txn)
        .await?;
    ensure_bound(
        reply_revisions.len(),
        MAX_FORUM_TOPIC_FORK_REPLY_REVISIONS,
        "reply revisions",
    )?;

    let relation_revisions = forum_relation_revision::Entity::find()
        .filter(forum_relation_revision::Column::TenantId.eq(tenant_id))
        .filter(forum_relation_revision::Column::TargetKind.eq("reply"))
        .filter(forum_relation_revision::Column::TargetId.is_in(reply_ids.to_vec()))
        .order_by_asc(forum_relation_revision::Column::RevisionId)
        .limit((MAX_FORUM_TOPIC_FORK_RELATION_REVISIONS + 1) as u64)
        .all(txn)
        .await?;
    ensure_bound(
        relation_revisions.len(),
        MAX_FORUM_TOPIC_FORK_RELATION_REVISIONS,
        "relation revisions",
    )?;
    let relation_ids = relation_revisions
        .iter()
        .map(|revision| revision.revision_id)
        .collect::<Vec<_>>();
    let (user_mentions, audience_mentions, quotes) = if relation_ids.is_empty() {
        (Vec::new(), Vec::new(), Vec::new())
    } else {
        let user_mentions = forum_user_mention::Entity::find()
            .filter(forum_user_mention::Column::TenantId.eq(tenant_id))
            .filter(forum_user_mention::Column::SourceRevisionId.is_in(relation_ids.clone()))
            .limit((MAX_FORUM_TOPIC_FORK_MENTIONS + 1) as u64)
            .all(txn)
            .await?;
        let audience_mentions = forum_audience_mention::Entity::find()
            .filter(forum_audience_mention::Column::TenantId.eq(tenant_id))
            .filter(forum_audience_mention::Column::SourceRevisionId.is_in(relation_ids.clone()))
            .limit((MAX_FORUM_TOPIC_FORK_MENTIONS + 1) as u64)
            .all(txn)
            .await?;
        ensure_bound(
            user_mentions.len() + audience_mentions.len(),
            MAX_FORUM_TOPIC_FORK_MENTIONS,
            "mention projections",
        )?;
        let quotes = forum_quote::Entity::find()
            .filter(forum_quote::Column::TenantId.eq(tenant_id))
            .filter(forum_quote::Column::SourceRevisionId.is_in(relation_ids))
            .limit((MAX_FORUM_TOPIC_FORK_QUOTES + 1) as u64)
            .all(txn)
            .await?;
        ensure_bound(
            quotes.len(),
            MAX_FORUM_TOPIC_FORK_QUOTES,
            "quote projections",
        )?;
        (user_mentions, audience_mentions, quotes)
    };

    Ok(ForkSnapshot {
        replies,
        bodies,
        reply_revisions,
        relation_revisions,
        user_mentions,
        audience_mentions,
        quotes,
    })
}

fn ensure_bound(actual: usize, maximum: usize, label: &str) -> ForumResult<()> {
    if actual > maximum {
        return Err(ForumError::Validation(format!(
            "Forum topic fork exceeds the bounded {label} limit of {maximum}"
        )));
    }
    Ok(())
}

async fn ensure_target_reply_ids_absent_in_tx(
    txn: &DatabaseTransaction,
    reply_id_map: &HashMap<Uuid, Uuid>,
) -> ForumResult<()> {
    let existing = forum_reply::Entity::find()
        .filter(forum_reply::Column::Id.is_in(reply_id_map.values().copied().collect::<Vec<_>>()))
        .count(txn)
        .await?;
    if existing != 0 {
        return Err(ForumError::Validation(
            "Forum topic fork deterministic target reply ID already exists".to_string(),
        ));
    }
    Ok(())
}

async fn copy_reply_rows_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    target_topic_id: Uuid,
    root_reply_id: Uuid,
    replies: &[forum_reply::Model],
    reply_id_map: &HashMap<Uuid, Uuid>,
) -> ForumResult<Vec<ForkReplyAudit>> {
    let mut audit = Vec::with_capacity(replies.len());
    for (index, reply) in replies.iter().enumerate() {
        let target_reply_id = mapped_reply_id(reply_id_map, reply.id)?;
        let target_parent_reply_id = if reply.id == root_reply_id {
            None
        } else {
            Some(mapped_reply_id(
                reply_id_map,
                reply.parent_reply_id.ok_or_else(|| {
                    ForumError::Validation(
                        "Forum topic fork descendant reply is missing its parent".to_string(),
                    )
                })?,
            )?)
        };
        let target_position = i64::try_from(index + 1).map_err(|_| {
            ForumError::Validation(
                "Forum topic fork target position exceeds supported range".to_string(),
            )
        })?;
        let inserted = forum_reply::ActiveModel {
            id: Set(target_reply_id),
            tenant_id: Set(tenant_id),
            topic_id: Set(target_topic_id),
            author_id: Set(reply.author_id),
            parent_reply_id: Set(target_parent_reply_id),
            status: Set(reply.status),
            position: Set(target_position),
            created_at: Set(reply.created_at),
            updated_at: Set(reply.updated_at),
        }
        .insert(txn)
        .await?;
        if inserted.position != target_position
            || inserted.parent_reply_id != target_parent_reply_id
            || inserted.status != reply.status
            || inserted.author_id != reply.author_id
        {
            return Err(ForumError::Validation(
                "Forum topic fork copied reply row is inconsistent".to_string(),
            ));
        }
        audit.push(ForkReplyAudit {
            source_reply_id: reply.id,
            target_reply_id,
            source_parent_reply_id: reply.parent_reply_id,
            target_parent_reply_id,
            source_position: reply.position,
            target_position,
            was_published: reply.status == ReplyStatus::Approved,
        });
    }
    Ok(audit)
}

async fn copy_reply_bodies_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    bodies: &[forum_reply_body::Model],
    reply_id_map: &HashMap<Uuid, Uuid>,
) -> ForumResult<()> {
    for body in bodies {
        forum_reply_body::ActiveModel {
            id: Set(Uuid::new_v4()),
            reply_id: Set(mapped_reply_id(reply_id_map, body.reply_id)?),
            tenant_id: Set(tenant_id),
            locale: Set(body.locale.clone()),
            body: Set(body.body.clone()),
            created_at: Set(body.created_at),
            updated_at: Set(body.updated_at),
        }
        .insert(txn)
        .await?;
    }
    Ok(())
}

async fn copy_reply_revisions_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    revisions: &[forum_reply_revision::Model],
    reply_id_map: &HashMap<Uuid, Uuid>,
) -> ForumResult<Vec<ForkRevisionAudit>> {
    let mut audit = Vec::with_capacity(revisions.len());
    for revision in revisions {
        let target_reply_id = mapped_reply_id(reply_id_map, revision.reply_id)?;
        let inserted = forum_reply_revision::ActiveModel {
            id: NotSet,
            tenant_id: Set(tenant_id),
            reply_id: Set(target_reply_id),
            locale: Set(revision.locale.clone()),
            body: Set(revision.body.clone()),
            revision_reason: Set(revision.revision_reason.clone()),
            created_at: Set(revision.created_at),
        }
        .insert(txn)
        .await?;
        audit.push(ForkRevisionAudit {
            revision_kind: "reply",
            source_revision_id: revision.id,
            target_revision_id: inserted.id,
            source_reply_id: revision.reply_id,
            target_reply_id,
            locale: revision.locale.clone(),
        });
    }
    Ok(audit)
}

async fn copy_relation_revisions_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    revisions: &[forum_relation_revision::Model],
    reply_id_map: &HashMap<Uuid, Uuid>,
) -> ForumResult<(HashMap<i64, i64>, Vec<ForkRevisionAudit>)> {
    let mut revision_map = HashMap::with_capacity(revisions.len());
    let mut audit = Vec::with_capacity(revisions.len());
    for revision in revisions {
        let target_reply_id = mapped_reply_id(reply_id_map, revision.target_id)?;
        let inserted = forum_relation_revision::ActiveModel {
            revision_id: NotSet,
            tenant_id: Set(tenant_id),
            target_kind: Set("reply".to_string()),
            target_id: Set(target_reply_id),
            locale: Set(revision.locale.clone()),
            projection_fingerprint: Set(revision.projection_fingerprint.clone()),
            created_at: Set(revision.created_at),
        }
        .insert(txn)
        .await?;
        if revision_map
            .insert(revision.revision_id, inserted.revision_id)
            .is_some()
        {
            return Err(ForumError::Validation(
                "Forum topic fork relation revision mapping is duplicated".to_string(),
            ));
        }
        audit.push(ForkRevisionAudit {
            revision_kind: "relation",
            source_revision_id: revision.revision_id,
            target_revision_id: inserted.revision_id,
            source_reply_id: revision.target_id,
            target_reply_id,
            locale: revision.locale.clone(),
        });
    }
    Ok((revision_map, audit))
}

async fn copy_relation_children_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    snapshot: &ForkSnapshot,
    reply_id_map: &HashMap<Uuid, Uuid>,
    revision_map: &HashMap<i64, i64>,
) -> ForumResult<()> {
    for mention in &snapshot.user_mentions {
        forum_user_mention::ActiveModel {
            tenant_id: Set(tenant_id),
            source_kind: Set("reply".to_string()),
            source_id: Set(mapped_reply_id(reply_id_map, mention.source_id)?),
            source_locale: Set(mention.source_locale.clone()),
            source_revision_id: Set(mapped_revision_id(
                revision_map,
                mention.source_revision_id,
            )?),
            mentioned_user_id: Set(mention.mentioned_user_id),
            handle_snapshot: Set(mention.handle_snapshot.clone()),
            created_at: Set(mention.created_at),
        }
        .insert(txn)
        .await?;
    }
    for mention in &snapshot.audience_mentions {
        forum_audience_mention::ActiveModel {
            tenant_id: Set(tenant_id),
            source_kind: Set("reply".to_string()),
            source_id: Set(mapped_reply_id(reply_id_map, mention.source_id)?),
            source_locale: Set(mention.source_locale.clone()),
            source_revision_id: Set(mapped_revision_id(
                revision_map,
                mention.source_revision_id,
            )?),
            audience: Set(mention.audience.clone()),
            created_at: Set(mention.created_at),
        }
        .insert(txn)
        .await?;
    }
    for quote in &snapshot.quotes {
        forum_quote::ActiveModel {
            tenant_id: Set(tenant_id),
            source_kind: Set("reply".to_string()),
            source_id: Set(mapped_reply_id(reply_id_map, quote.source_id)?),
            source_locale: Set(quote.source_locale.clone()),
            source_revision_id: Set(mapped_revision_id(revision_map, quote.source_revision_id)?),
            quoted_kind: Set(quote.quoted_kind.clone()),
            quoted_id: Set(quote.quoted_id),
            quoted_revision_id: Set(quote.quoted_revision_id),
            created_at: Set(quote.created_at),
        }
        .insert(txn)
        .await?;
    }
    Ok(())
}

fn mapped_reply_id(reply_id_map: &HashMap<Uuid, Uuid>, source_reply_id: Uuid) -> ForumResult<Uuid> {
    reply_id_map.get(&source_reply_id).copied().ok_or_else(|| {
        ForumError::Validation("Forum topic fork copied reply mapping is incomplete".to_string())
    })
}

fn mapped_revision_id(
    revision_map: &HashMap<i64, i64>,
    source_revision_id: i64,
) -> ForumResult<i64> {
    revision_map
        .get(&source_revision_id)
        .copied()
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum topic fork copied relation revision mapping is incomplete".to_string(),
            )
        })
}

async fn adjust_copied_reply_author_stats_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    replies: &[forum_reply::Model],
) -> ForumResult<()> {
    let mut deltas = BTreeMap::<Uuid, i32>::new();
    for reply in replies
        .iter()
        .filter(|reply| reply.status == ReplyStatus::Approved)
    {
        if let Some(author_id) = reply.author_id {
            let entry = deltas.entry(author_id).or_insert(0);
            *entry = entry.checked_add(1).ok_or_else(|| {
                ForumError::Validation("Forum topic fork author reply counter overflow".to_string())
            })?;
        }
    }
    for (author_id, delta) in deltas {
        UserStatsService::adjust_reply_count_in_tx(txn, tenant_id, Some(author_id), delta).await?;
    }
    Ok(())
}
