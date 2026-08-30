use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Statement, TransactionTrait,
    prelude::DateTimeWithTimeZone,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::audience::{
    ForumAudienceConstraints, MAX_FORUM_AUDIENCE_CHANNELS, MAX_FORUM_AUDIENCE_EXPLICIT_USERS,
    MAX_FORUM_AUDIENCE_GROUPS, MAX_FORUM_AUDIENCE_ROLES,
};
use crate::entities::{
    forum_category_audience_user::ForumCategoryAudienceUserEffect, forum_domain_event,
    forum_topic_audience_channel, forum_topic_audience_group, forum_topic_audience_policy,
    forum_topic_audience_role, forum_topic_audience_user,
    forum_topic_merge_audience_reconciliation,
    forum_topic_merge_audience_reconciliation::ForumTopicMergeAudienceOutcome,
    forum_topic_merge_operation,
};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::TopicStatus;

use super::projection_invalidation::publish_forum_topic_projection_in_tx;
use super::rbac::enforce_scope;
use super::topic_audience_lock::{
    lock_topic_audience_scopes_in_tx, lock_topic_rows_for_audience_in_tx,
};

pub const MAX_FORUM_TOPIC_MERGE_AUDIENCE_REASON_LEN: usize = 500;
const FORUM_TOPIC_MERGE_AUDIENCE_RECONCILED_EVENT_TYPE: &str =
    "forum.topic.merge.audience_reconciled";
const FORUM_TOPIC_MERGE_AUDIENCE_AGGREGATE_TYPE: &str = "forum_topic";
const FORUM_TOPIC_MERGED_EVENT_TYPE: &str = "forum.topic.merged";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileForumTopicMergeAudienceInput {
    pub operation_id: Uuid,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumTopicMergeAudienceReconciliationResult {
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub merge_operation_id: Uuid,
    pub source_topic_id: Uuid,
    pub target_topic_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub outcome: ForumTopicMergeAudienceOutcome,
    pub reconciled_at: DateTime<Utc>,
}

/// Reconcile topic-local audience narrowing after one completed FORUM-21B merge.
///
/// Topic-local layers are conjunctive with inherited category layers, but each local layer contains
/// union-style positive selectors. Two differing local layers therefore cannot be flattened into one
/// equivalent row without risking visibility expansion. This owner handles only representable safe
/// outcomes and fails closed when both topics have different local layers.
pub struct ForumTopicMergeAudienceReconciliationService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ForumTopicMergeAudienceReconciliationService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, security, input))]
    pub async fn reconcile_merge_audience(
        &self,
        tenant_id: Uuid,
        merge_operation_id: Uuid,
        security: SecurityContext,
        input: ReconcileForumTopicMergeAudienceInput,
    ) -> ForumResult<ForumTopicMergeAudienceReconciliationResult> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        let actor_id = security.user_id.ok_or_else(|| {
            ForumError::Validation(
                "Forum topic merge audience reconciliation requires a human actor".to_string(),
            )
        })?;
        let reason = validate_input(tenant_id, merge_operation_id, actor_id, &input)?;

        let txn = self.db.begin().await?;
        lock_reconciliation_tenant_in_tx(&txn, tenant_id).await?;

        if let Some(existing) = forum_topic_merge_audience_reconciliation::Entity::find_by_id((
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
                return Err(ForumError::TopicMergeAudienceReconciliationConflict(
                    input.operation_id,
                ));
            }
            validate_reconciliation_event_in_tx(&txn, &existing).await?;
            lock_topic_audience_scopes_in_tx(
                &txn,
                tenant_id,
                &[existing.source_topic_id, existing.target_topic_id],
            )
            .await?;
            ensure_source_audience_empty_in_tx(&txn, tenant_id, existing.source_topic_id).await?;
            txn.commit().await?;
            return Ok(operation_to_result(existing));
        }

        if forum_topic_merge_audience_reconciliation::Entity::find()
            .filter(forum_topic_merge_audience_reconciliation::Column::TenantId.eq(tenant_id))
            .filter(
                forum_topic_merge_audience_reconciliation::Column::MergeOperationId
                    .eq(merge_operation_id),
            )
            .one(&txn)
            .await?
            .is_some()
        {
            return Err(ForumError::TopicMergeAudienceReconciliationConflict(
                input.operation_id,
            ));
        }

        let merge =
            forum_topic_merge_operation::Entity::find_by_id((tenant_id, merge_operation_id))
                .one(&txn)
                .await?
                .ok_or_else(|| {
                    ForumError::Validation(
                    "Forum topic merge audience reconciliation requires an existing merge receipt"
                        .to_string(),
                )
                })?;
        validate_merge_event_in_tx(&txn, &merge).await?;

        let topics = lock_topic_rows_for_audience_in_tx(
            &txn,
            tenant_id,
            &[merge.source_topic_id, merge.target_topic_id],
        )
        .await?;
        let topics = topics
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
                "Forum topic merge audience reconciliation requires the archived locked source tombstone"
                    .to_string(),
            ));
        }
        if target.status == TopicStatus::Archived {
            return Err(ForumError::Validation(
                "Forum topic merge audience reconciliation requires an active retained target"
                    .to_string(),
            ));
        }
        if source.category_id != merge.category_id
            || target.category_id != merge.category_id
            || source.category_id != target.category_id
        {
            return Err(ForumError::Validation(
                "Forum topic merge audience reconciliation does not match the merge category"
                    .to_string(),
            ));
        }

        lock_topic_audience_scopes_in_tx(
            &txn,
            tenant_id,
            &[merge.source_topic_id, merge.target_topic_id],
        )
        .await?;

        let source_layer = load_local_layer(&txn, tenant_id, merge.source_topic_id).await?;
        let target_layer = load_local_layer(&txn, tenant_id, merge.target_topic_id).await?;
        let outcome = match (&source_layer, &target_layer) {
            (None, None) => ForumTopicMergeAudienceOutcome::BothUnrestricted,
            (None, Some(_)) => ForumTopicMergeAudienceOutcome::TargetOnlyPreserved,
            (Some(source_constraints), None) => {
                let source_policy = forum_topic_audience_policy::Entity::find_by_id((
                    tenant_id,
                    merge.source_topic_id,
                ))
                .one(&txn)
                .await?
                .ok_or_else(|| {
                    ForumError::Validation(
                        "Forum source topic audience layer is missing its policy row".to_string(),
                    )
                })?;
                delete_local_layer_in_tx(&txn, tenant_id, merge.source_topic_id).await?;
                insert_local_layer_in_tx(
                    &txn,
                    tenant_id,
                    merge.target_topic_id,
                    source_constraints,
                    source_policy.updated_at,
                )
                .await?;
                ForumTopicMergeAudienceOutcome::SourceOnlyMoved
            }
            (Some(source_constraints), Some(target_constraints))
                if source_constraints == target_constraints =>
            {
                delete_local_layer_in_tx(&txn, tenant_id, merge.source_topic_id).await?;
                ForumTopicMergeAudienceOutcome::EqualLayersDeduplicated
            }
            (Some(_), Some(_)) => {
                return Err(ForumError::TopicMergeAudiencePolicyConflict(
                    merge_operation_id,
                ));
            }
        };

        ensure_source_audience_empty_in_tx(&txn, tenant_id, merge.source_topic_id).await?;
        match outcome {
            ForumTopicMergeAudienceOutcome::SourceOnlyMoved
            | ForumTopicMergeAudienceOutcome::EqualLayersDeduplicated => {
                let target_after = load_local_layer(&txn, tenant_id, merge.target_topic_id).await?;
                if target_after != source_layer {
                    return Err(ForumError::Validation(
                        "Forum retained topic audience layer does not match the reconciled source layer"
                            .to_string(),
                    ));
                }
            }
            ForumTopicMergeAudienceOutcome::BothUnrestricted => {
                if load_local_layer(&txn, tenant_id, merge.target_topic_id)
                    .await?
                    .is_some()
                {
                    return Err(ForumError::Validation(
                        "Forum retained topic unexpectedly gained an audience layer".to_string(),
                    ));
                }
            }
            ForumTopicMergeAudienceOutcome::TargetOnlyPreserved => {
                if load_local_layer(&txn, tenant_id, merge.target_topic_id).await? != target_layer {
                    return Err(ForumError::Validation(
                        "Forum retained topic audience layer changed during reconciliation"
                            .to_string(),
                    ));
                }
            }
        }

        let now = Utc::now();
        let payload = reconciliation_payload(
            input.operation_id,
            merge_operation_id,
            merge.source_topic_id,
            merge.target_topic_id,
            outcome,
            &reason,
        );
        forum_domain_event::ActiveModel {
            sequence_no: NotSet,
            event_id: Set(input.operation_id),
            tenant_id: Set(tenant_id),
            aggregate_type: Set(FORUM_TOPIC_MERGE_AUDIENCE_AGGREGATE_TYPE.to_string()),
            aggregate_id: Set(merge.target_topic_id),
            event_type: Set(FORUM_TOPIC_MERGE_AUDIENCE_RECONCILED_EVENT_TYPE.to_string()),
            schema_version: Set(1),
            actor_id: Set(Some(actor_id)),
            payload: Set(payload),
            created_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        let operation = forum_topic_merge_audience_reconciliation::ActiveModel {
            tenant_id: Set(tenant_id),
            operation_id: Set(input.operation_id),
            merge_operation_id: Set(merge_operation_id),
            source_topic_id: Set(merge.source_topic_id),
            target_topic_id: Set(merge.target_topic_id),
            actor_id: Set(actor_id),
            reason: Set(reason),
            outcome: Set(outcome),
            event_id: Set(input.operation_id),
            reconciled_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            Some(actor_id),
            merge.source_topic_id,
        )
        .await?;
        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            Some(actor_id),
            merge.target_topic_id,
        )
        .await?;

        txn.commit().await?;
        Ok(operation_to_result(operation))
    }
}

fn validate_input(
    tenant_id: Uuid,
    merge_operation_id: Uuid,
    actor_id: Uuid,
    input: &ReconcileForumTopicMergeAudienceInput,
) -> ForumResult<String> {
    for (label, value) in [
        ("tenant", tenant_id),
        ("operation", input.operation_id),
        ("merge operation", merge_operation_id),
        ("actor", actor_id),
    ] {
        if value.is_nil() {
            return Err(ForumError::Validation(format!(
                "Forum topic merge audience reconciliation {label} must not be nil"
            )));
        }
    }
    let reason = input.reason.trim();
    if reason.is_empty() {
        return Err(ForumError::Validation(
            "Forum topic merge audience reconciliation reason must not be empty".to_string(),
        ));
    }
    if reason.chars().count() > MAX_FORUM_TOPIC_MERGE_AUDIENCE_REASON_LEN {
        return Err(ForumError::Validation(format!(
            "Forum topic merge audience reconciliation reason must not exceed {MAX_FORUM_TOPIC_MERGE_AUDIENCE_REASON_LEN} characters"
        )));
    }
    if reason.chars().any(char::is_control) {
        return Err(ForumError::Validation(
            "Forum topic merge audience reconciliation reason must not contain control characters"
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
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 30))",
                vec![format!("forum-topic-merge-audience-reconciliation:{tenant_id}").into()],
            ))
            .await?;
        }
        DatabaseBackend::Sqlite => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"
                INSERT INTO forum_topic_merge_audience_reconciliation_locks (
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
                "Forum topic merge audience reconciliation does not support database backend {backend:?}"
            )));
        }
    }
    Ok(())
}

async fn load_local_layer<C>(
    db: &C,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Option<ForumAudienceConstraints>>
where
    C: ConnectionTrait,
{
    let policy = forum_topic_audience_policy::Entity::find_by_id((tenant_id, topic_id))
        .one(db)
        .await?;
    let roles = forum_topic_audience_role::Entity::find()
        .filter(forum_topic_audience_role::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_role::Column::TopicId.eq(topic_id))
        .limit((MAX_FORUM_AUDIENCE_ROLES + 1) as u64)
        .all(db)
        .await?;
    let channels = forum_topic_audience_channel::Entity::find()
        .filter(forum_topic_audience_channel::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_channel::Column::TopicId.eq(topic_id))
        .limit((MAX_FORUM_AUDIENCE_CHANNELS + 1) as u64)
        .all(db)
        .await?;
    let groups = forum_topic_audience_group::Entity::find()
        .filter(forum_topic_audience_group::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_group::Column::TopicId.eq(topic_id))
        .limit((MAX_FORUM_AUDIENCE_GROUPS + 1) as u64)
        .all(db)
        .await?;
    let users = forum_topic_audience_user::Entity::find()
        .filter(forum_topic_audience_user::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_user::Column::TopicId.eq(topic_id))
        .limit((MAX_FORUM_AUDIENCE_EXPLICIT_USERS * 2 + 1) as u64)
        .all(db)
        .await?;

    ensure_storage_bound(roles.len(), MAX_FORUM_AUDIENCE_ROLES, "role relations")?;
    ensure_storage_bound(
        channels.len(),
        MAX_FORUM_AUDIENCE_CHANNELS,
        "channel relations",
    )?;
    ensure_storage_bound(groups.len(), MAX_FORUM_AUDIENCE_GROUPS, "group relations")?;
    ensure_storage_bound(
        users.len(),
        MAX_FORUM_AUDIENCE_EXPLICIT_USERS * 2,
        "explicit user relations",
    )?;

    let Some(policy) = policy else {
        if !roles.is_empty() || !channels.is_empty() || !groups.is_empty() || !users.is_empty() {
            return Err(ForumError::Validation(
                "Forum topic audience relation is missing its local policy layer".to_string(),
            ));
        }
        return Ok(None);
    };

    let mut constraints = ForumAudienceConstraints {
        minimum_trust_level: policy
            .minimum_trust_level
            .map(|level| {
                u8::try_from(level).map_err(|_| {
                    ForumError::Validation(
                        "Forum topic audience storage contains an invalid trust level".to_string(),
                    )
                })
            })
            .transpose()?,
        roles_any: roles.into_iter().map(|row| row.role).collect(),
        channel_members_any: channels.into_iter().map(|row| row.channel_slug).collect(),
        group_members_any: groups.into_iter().map(|row| row.group_id).collect(),
        ..ForumAudienceConstraints::default()
    };
    for row in users {
        match row.effect {
            ForumCategoryAudienceUserEffect::Allow => constraints.allow_user_ids.push(row.user_id),
            ForumCategoryAudienceUserEffect::Deny => constraints.deny_user_ids.push(row.user_id),
        }
    }
    let constraints = constraints.normalize()?;
    if constraints_are_empty(&constraints) {
        return Err(ForumError::Validation(
            "Forum topic audience storage contains an empty local layer".to_string(),
        ));
    }
    Ok(Some(constraints))
}

async fn insert_local_layer_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    constraints: &ForumAudienceConstraints,
    updated_at: DateTimeWithTimeZone,
) -> ForumResult<()> {
    let constraints = constraints.clone().normalize()?;
    if constraints_are_empty(&constraints) {
        return Err(ForumError::Validation(
            "Forum topic merge audience cannot persist an empty local layer".to_string(),
        ));
    }
    forum_topic_audience_policy::ActiveModel {
        tenant_id: Set(tenant_id),
        topic_id: Set(topic_id),
        minimum_trust_level: Set(constraints.minimum_trust_level.map(i16::from)),
        updated_at: Set(updated_at),
    }
    .insert(txn)
    .await?;

    if !constraints.roles_any.is_empty() {
        forum_topic_audience_role::Entity::insert_many(
            constraints
                .roles_any
                .iter()
                .cloned()
                .map(|role| forum_topic_audience_role::ActiveModel {
                    tenant_id: Set(tenant_id),
                    topic_id: Set(topic_id),
                    role: Set(role),
                })
                .collect::<Vec<_>>(),
        )
        .exec(txn)
        .await?;
    }
    if !constraints.channel_members_any.is_empty() {
        forum_topic_audience_channel::Entity::insert_many(
            constraints
                .channel_members_any
                .iter()
                .cloned()
                .map(|channel_slug| forum_topic_audience_channel::ActiveModel {
                    tenant_id: Set(tenant_id),
                    topic_id: Set(topic_id),
                    channel_slug: Set(channel_slug),
                })
                .collect::<Vec<_>>(),
        )
        .exec(txn)
        .await?;
    }
    if !constraints.group_members_any.is_empty() {
        forum_topic_audience_group::Entity::insert_many(
            constraints
                .group_members_any
                .iter()
                .copied()
                .map(|group_id| forum_topic_audience_group::ActiveModel {
                    tenant_id: Set(tenant_id),
                    topic_id: Set(topic_id),
                    group_id: Set(group_id),
                })
                .collect::<Vec<_>>(),
        )
        .exec(txn)
        .await?;
    }
    let mut users =
        Vec::with_capacity(constraints.allow_user_ids.len() + constraints.deny_user_ids.len());
    users.extend(constraints.allow_user_ids.iter().copied().map(|user_id| {
        forum_topic_audience_user::ActiveModel {
            tenant_id: Set(tenant_id),
            topic_id: Set(topic_id),
            user_id: Set(user_id),
            effect: Set(ForumCategoryAudienceUserEffect::Allow),
        }
    }));
    users.extend(constraints.deny_user_ids.iter().copied().map(|user_id| {
        forum_topic_audience_user::ActiveModel {
            tenant_id: Set(tenant_id),
            topic_id: Set(topic_id),
            user_id: Set(user_id),
            effect: Set(ForumCategoryAudienceUserEffect::Deny),
        }
    }));
    if !users.is_empty() {
        forum_topic_audience_user::Entity::insert_many(users)
            .exec(txn)
            .await?;
    }
    Ok(())
}

async fn delete_local_layer_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    let result = forum_topic_audience_policy::Entity::delete_many()
        .filter(forum_topic_audience_policy::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_policy::Column::TopicId.eq(topic_id))
        .exec(txn)
        .await?;
    if result.rows_affected != 1 {
        return Err(ForumError::Validation(
            "Forum source topic audience layer changed concurrently".to_string(),
        ));
    }
    Ok(())
}

async fn ensure_source_audience_empty_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    source_topic_id: Uuid,
) -> ForumResult<()> {
    let policy_count = forum_topic_audience_policy::Entity::find()
        .filter(forum_topic_audience_policy::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_policy::Column::TopicId.eq(source_topic_id))
        .count(txn)
        .await?;
    let role_count = forum_topic_audience_role::Entity::find()
        .filter(forum_topic_audience_role::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_role::Column::TopicId.eq(source_topic_id))
        .count(txn)
        .await?;
    let channel_count = forum_topic_audience_channel::Entity::find()
        .filter(forum_topic_audience_channel::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_channel::Column::TopicId.eq(source_topic_id))
        .count(txn)
        .await?;
    let group_count = forum_topic_audience_group::Entity::find()
        .filter(forum_topic_audience_group::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_group::Column::TopicId.eq(source_topic_id))
        .count(txn)
        .await?;
    let user_count = forum_topic_audience_user::Entity::find()
        .filter(forum_topic_audience_user::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_audience_user::Column::TopicId.eq(source_topic_id))
        .count(txn)
        .await?;
    if policy_count != 0
        || role_count != 0
        || channel_count != 0
        || group_count != 0
        || user_count != 0
    {
        return Err(ForumError::Validation(
            "Forum source topic audience rows remain after reconciliation".to_string(),
        ));
    }
    Ok(())
}

fn ensure_storage_bound(actual: usize, maximum: usize, label: &str) -> ForumResult<()> {
    if actual > maximum {
        return Err(ForumError::Validation(format!(
            "Forum topic merge audience storage exceeds the bounded {label} limit of {maximum}"
        )));
    }
    Ok(())
}

fn constraints_are_empty(constraints: &ForumAudienceConstraints) -> bool {
    constraints.roles_any.is_empty()
        && constraints.minimum_trust_level.is_none()
        && constraints.channel_members_any.is_empty()
        && constraints.group_members_any.is_empty()
        && constraints.allow_user_ids.is_empty()
        && constraints.deny_user_ids.is_empty()
}

fn reconciliation_payload(
    operation_id: Uuid,
    merge_operation_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    outcome: ForumTopicMergeAudienceOutcome,
    reason: &str,
) -> JsonValue {
    json!({
        "operation_id": operation_id,
        "merge_operation_id": merge_operation_id,
        "source_topic_id": source_topic_id,
        "target_topic_id": target_topic_id,
        "outcome": outcome_value(outcome),
        "reason": reason,
    })
}

const fn outcome_value(outcome: ForumTopicMergeAudienceOutcome) -> &'static str {
    match outcome {
        ForumTopicMergeAudienceOutcome::BothUnrestricted => "both_unrestricted",
        ForumTopicMergeAudienceOutcome::TargetOnlyPreserved => "target_only_preserved",
        ForumTopicMergeAudienceOutcome::SourceOnlyMoved => "source_only_moved",
        ForumTopicMergeAudienceOutcome::EqualLayersDeduplicated => "equal_layers_deduplicated",
    }
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
    if event.aggregate_type != FORUM_TOPIC_MERGE_AUDIENCE_AGGREGATE_TYPE
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
    operation: &forum_topic_merge_audience_reconciliation::Model,
) -> ForumResult<()> {
    let event = forum_domain_event::Entity::find()
        .filter(forum_domain_event::Column::TenantId.eq(operation.tenant_id))
        .filter(forum_domain_event::Column::EventId.eq(operation.event_id))
        .one(txn)
        .await?
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum topic merge audience reconciliation is missing its semantic event"
                    .to_string(),
            )
        })?;
    let expected_payload = reconciliation_payload(
        operation.operation_id,
        operation.merge_operation_id,
        operation.source_topic_id,
        operation.target_topic_id,
        operation.outcome,
        &operation.reason,
    );
    if event.aggregate_type != FORUM_TOPIC_MERGE_AUDIENCE_AGGREGATE_TYPE
        || event.aggregate_id != operation.target_topic_id
        || event.event_type != FORUM_TOPIC_MERGE_AUDIENCE_RECONCILED_EVENT_TYPE
        || event.schema_version != 1
        || event.actor_id != Some(operation.actor_id)
        || event.payload != expected_payload
    {
        return Err(ForumError::Validation(
            "Forum topic merge audience reconciliation semantic event does not match its receipt"
                .to_string(),
        ));
    }
    Ok(())
}

fn operation_to_result(
    operation: forum_topic_merge_audience_reconciliation::Model,
) -> ForumTopicMergeAudienceReconciliationResult {
    ForumTopicMergeAudienceReconciliationResult {
        operation_id: operation.operation_id,
        event_id: operation.event_id,
        merge_operation_id: operation.merge_operation_id,
        source_topic_id: operation.source_topic_id,
        target_topic_id: operation.target_topic_id,
        actor_id: operation.actor_id,
        reason: operation.reason,
        outcome: operation.outcome,
        reconciled_at: operation.reconciled_at.with_timezone(&Utc),
    }
}
