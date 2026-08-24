use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{HostRuntimeContext, PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_events::DomainEvent;
use rustok_moderation_api::{
    ApplyModerationDecisionCommand, ModerationDecisionApplication, ModerationDecisionEffectAction,
    ModerationSubjectAdapterBuildError, ModerationSubjectAdapterFactory,
    ModerationSubjectAdapterKey, ModerationSubjectCommandPort, ModerationSubjectKind,
    ModerationVisibilityState,
};
use rustok_outbox::{TransactionalEventBus, idempotency};
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    IsolationLevel, Statement, TransactionTrait,
};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};
use crate::services::projection_invalidation::{
    publish_forum_category_projection_direct_in_tx, publish_forum_topic_projection_direct_in_tx,
};
use crate::services::user_stats::UserStatsService;
use crate::services::{CategoryService, ReplyService, TopicService};
use crate::state_machine::ReplyStatus;

pub const FORUM_MODERATION_MODULE: &str = "forum";
const FORUM_OWNER_SLUG: &str = "forum";
const APPLY_MODERATION_DECISION_OPERATION: &str = "apply_moderation_decision";

#[derive(Clone, Copy, Debug)]
pub struct ForumModerationSubjectAdapterFactory {
    kind: ModerationSubjectKind,
}

impl ForumModerationSubjectAdapterFactory {
    pub const fn topic() -> Self {
        Self {
            kind: ModerationSubjectKind::ForumTopic,
        }
    }

    pub const fn reply() -> Self {
        Self {
            kind: ModerationSubjectKind::ForumPost,
        }
    }

    fn adapter_key(self) -> ModerationSubjectAdapterKey {
        ModerationSubjectAdapterKey::new(FORUM_MODERATION_MODULE, self.kind)
            .expect("static Forum moderation adapter key is valid")
    }
}

impl ModerationSubjectAdapterFactory for ForumModerationSubjectAdapterFactory {
    fn key(&self) -> ModerationSubjectAdapterKey {
        self.adapter_key()
    }

    fn build(
        &self,
        host: &HostRuntimeContext,
    ) -> Result<Arc<dyn ModerationSubjectCommandPort>, ModerationSubjectAdapterBuildError> {
        Ok(Arc::new(ForumModerationSubjectAdapter {
            db: host.db_clone(),
            key: self.adapter_key(),
        }))
    }
}

#[derive(Clone)]
struct ForumModerationSubjectAdapter {
    db: DatabaseConnection,
    key: ModerationSubjectAdapterKey,
}

#[async_trait]
impl ModerationSubjectCommandPort for ForumModerationSubjectAdapter {
    fn key(&self) -> ModerationSubjectAdapterKey {
        self.key.clone()
    }

    async fn apply_moderation_decision(
        &self,
        context: PortContext,
        command: ApplyModerationDecisionCommand,
    ) -> Result<ModerationDecisionApplication, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        validate_trusted_caller(&context)?;
        validate_command_for_adapter(&self.key, &command)?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = trusted_actor_uuid(&context);

        let expected_idempotency_key = command.decision_id.to_string();
        if context.idempotency_key.as_deref() != Some(expected_idempotency_key.as_str()) {
            return Err(PortError::validation(
                "forum.moderation_decision_idempotency_mismatch",
                "moderation decision UUID must equal the port idempotency key",
            ));
        }

        let lease = match idempotency::admit(
            &self.db,
            idempotency::OwnerOperationScope::Tenant(tenant_id),
            FORUM_OWNER_SLUG,
            expected_idempotency_key.as_str(),
            APPLY_MODERATION_DECISION_OPERATION,
            &command,
        )
        .await?
        {
            idempotency::Admission::Run(lease) => lease,
            idempotency::Admission::Replay(value) => return decode_replay(value),
            idempotency::Admission::ReplayError(error) => return Err(error),
        };

        let result = self
            .execute_apply(tenant_id, actor_id, lease, &command)
            .await;
        if let Err(error) = &result {
            // Retryable storage/serialization failures leave the processing lease
            // reclaimable instead of freezing a transient failure into the
            // decision's immutable replay result.
            if !error.retryable {
                let fail_res = idempotency::fail(&self.db, lease, error).await;
                if let Err(receipt_error) = fail_res {
                    tracing::error!(
                        operation_id = %lease.operation_id,
                        error = %receipt_error.message,
                        "failed to persist Forum moderation application failure receipt"
                    );
                }
            }
        }
        result
    }
}

impl ForumModerationSubjectAdapter {
    async fn execute_apply(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        lease: idempotency::Lease,
        command: &ApplyModerationDecisionCommand,
    ) -> Result<ModerationDecisionApplication, PortError> {
        let transaction = begin_application_transaction(&self.db).await?;
        let result =
            apply_inside_transaction(&transaction, tenant_id, actor_id, &self.key, command).await;
        match result {
            Ok(application) => {
                idempotency::complete(&transaction, lease, &application).await?;
                transaction.commit().await.map_err(database_error)?;
                Ok(application)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

async fn apply_inside_transaction(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    key: &ModerationSubjectAdapterKey,
    command: &ApplyModerationDecisionCommand,
) -> Result<ModerationDecisionApplication, PortError> {
    lock_active_subject_and_revision(transaction, tenant_id, key.kind(), command.subject.id)
        .await?;
    let reviewed_revision =
        current_subject_revision(transaction, tenant_id, key.kind(), command.subject.id).await?;
    if reviewed_revision != command.subject.revision {
        return Err(PortError::conflict(
            "forum.moderation_subject_revision_conflict",
            "Forum subject changed after the moderation decision was reviewed",
        ));
    }

    let changed = match (key.kind(), &command.effect.action) {
        (_, ModerationDecisionEffectAction::NoDomainMutation) => false,
        (
            ModerationSubjectKind::ForumTopic,
            ModerationDecisionEffectAction::Lock {
                effective_until: None,
            },
        ) => {
            let topic = TopicService::find_topic_in_tx(transaction, tenant_id, command.subject.id)
                .await
                .map_err(forum_error)?;
            if topic.is_locked {
                false
            } else {
                TopicService::set_locked_in_tx(transaction, tenant_id, command.subject.id, true)
                    .await
                    .map_err(forum_error)?;
                publish_forum_topic_projection_direct_in_tx(
                    transaction,
                    tenant_id,
                    actor_id,
                    command.subject.id,
                )
                .await
                .map_err(forum_error)?;
                true
            }
        }
        (
            ModerationSubjectKind::ForumTopic,
            ModerationDecisionEffectAction::Lock {
                effective_until: Some(_),
            },
        ) => {
            return Err(PortError::validation(
                "forum.moderation_temporary_lock_unsupported",
                "Forum does not yet own an expiry-safe moderation lock state",
            ));
        }
        (
            ModerationSubjectKind::ForumPost,
            ModerationDecisionEffectAction::SetVisibility {
                state: ModerationVisibilityState::Hidden,
            },
        ) => apply_reply_hidden_effect_in_tx(transaction, tenant_id, actor_id, command.subject.id)
            .await
            .map_err(forum_error)?,
        (
            ModerationSubjectKind::ForumPost,
            ModerationDecisionEffectAction::SetVisibility {
                state: ModerationVisibilityState::Removed,
            },
        ) => apply_reply_removed_effect_in_tx(transaction, tenant_id, actor_id, command.subject.id)
            .await
            .map_err(forum_error)?,
        (ModerationSubjectKind::ForumPost, ModerationDecisionEffectAction::RejectPublication) => {
            apply_reply_rejected_effect_in_tx(transaction, tenant_id, actor_id, command.subject.id)
                .await
                .map_err(forum_error)?
        }
        _ => {
            return Err(PortError::validation(
                "forum.moderation_effect_unsupported",
                "the requested moderation effect is not supported by this Forum subject adapter",
            ));
        }
    };

    let applied_revision =
        current_subject_revision(transaction, tenant_id, key.kind(), command.subject.id).await?;
    if changed && applied_revision <= reviewed_revision {
        return Err(PortError::invariant_violation(
            "forum.moderation_subject_revision_not_advanced",
            "Forum mutation did not advance the moderation subject revision",
        ));
    }
    if !changed && applied_revision != reviewed_revision {
        return Err(PortError::conflict(
            "forum.moderation_subject_revision_changed_during_application",
            "Forum subject revision changed during moderation application",
        ));
    }

    Ok(ModerationDecisionApplication {
        decision_id: command.decision_id,
        subject: command.subject.clone(),
        applied_revision,
        applied_at: Utc::now(),
    })
}

async fn apply_reply_hidden_effect_in_tx(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    reply_id: Uuid,
) -> ForumResult<bool> {
    apply_reply_non_public_status_effect_in_tx(
        transaction,
        tenant_id,
        actor_id,
        reply_id,
        ReplyStatus::Hidden,
    )
    .await
}

async fn apply_reply_rejected_effect_in_tx(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    reply_id: Uuid,
) -> ForumResult<bool> {
    apply_reply_non_public_status_effect_in_tx(
        transaction,
        tenant_id,
        actor_id,
        reply_id,
        ReplyStatus::Rejected,
    )
    .await
}

async fn apply_reply_non_public_status_effect_in_tx(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    reply_id: Uuid,
    target: ReplyStatus,
) -> ForumResult<bool> {
    let reply = ReplyService::find_reply_in_tx(transaction, tenant_id, reply_id).await?;
    if reply.status == target {
        return Ok(false);
    }

    reply.status.validate_transition(&target)?;
    let topic_id = reply.topic_id;
    let old_status = reply.status.to_string();
    ReplyService::set_status_in_tx(transaction, tenant_id, reply_id, target).await?;

    let changed_category_id = if reply.status == ReplyStatus::Approved {
        let topic =
            TopicService::adjust_reply_count_in_tx(transaction, tenant_id, topic_id, -1).await?;
        CategoryService::adjust_counters_in_tx(transaction, tenant_id, topic.category_id, 0, -1)
            .await?;
        UserStatsService::adjust_reply_count_in_tx(transaction, tenant_id, reply.author_id, -1)
            .await?;
        Some(topic.category_id)
    } else {
        None
    };

    TransactionalEventBus::publish_root_in_tx(
        transaction,
        tenant_id,
        actor_id,
        DomainEvent::ForumReplyStatusChanged {
            reply_id,
            topic_id,
            old_status,
            new_status: target.to_string(),
            moderator_id: actor_id,
        },
    )
    .await?;

    if let Some(category_id) = changed_category_id {
        publish_forum_category_projection_direct_in_tx(
            transaction,
            tenant_id,
            actor_id,
            category_id,
        )
        .await?;
    }

    Ok(true)
}

async fn apply_reply_removed_effect_in_tx(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    reply_id: Uuid,
) -> ForumResult<bool> {
    let outcome = ReplyService::remove_in_tx(transaction, tenant_id, reply_id).await?;

    TransactionalEventBus::publish_root_in_tx(
        transaction,
        tenant_id,
        actor_id,
        DomainEvent::ForumReplyStatusChanged {
            reply_id,
            topic_id: outcome.topic_id,
            old_status: outcome.old_status.to_string(),
            new_status: ReplyStatus::Deleted.to_string(),
            moderator_id: actor_id,
        },
    )
    .await?;

    if outcome.was_public {
        publish_forum_category_projection_direct_in_tx(
            transaction,
            tenant_id,
            actor_id,
            outcome.category_id,
        )
        .await?;
    }

    Ok(true)
}

async fn begin_application_transaction(
    db: &DatabaseConnection,
) -> Result<DatabaseTransaction, PortError> {
    if db.get_database_backend() == DatabaseBackend::Postgres {
        db.begin_with_config(
            Some(IsolationLevel::Serializable),
            Some(AccessMode::ReadWrite),
        )
        .await
        .map_err(database_error)
    } else {
        db.begin().await.map_err(database_error)
    }
}

async fn lock_active_subject_and_revision(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    kind: ModerationSubjectKind,
    subject_id: Uuid,
) -> Result<(), PortError> {
    let backend = transaction.get_database_backend();
    let subject_table = subject_table(kind)?;
    let (revision_table, revision_id_column) = revision_table(kind)?;

    match backend {
        DatabaseBackend::Postgres => {
            let subject_sql = format!(
                "SELECT id FROM {subject_table} WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE"
            );
            let subject = transaction
                .query_one(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    subject_sql,
                    vec![tenant_id.into(), subject_id.into()],
                ))
                .await
                .map_err(database_error)?;
            if subject.is_none() {
                return Err(subject_unavailable());
            }

            let revision_sql = format!(
                "SELECT revision FROM {revision_table} WHERE tenant_id = $1 AND {revision_id_column} = $2 FOR UPDATE"
            );
            let revision = transaction
                .query_one(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    revision_sql,
                    vec![tenant_id.into(), subject_id.into()],
                ))
                .await
                .map_err(database_error)?;
            if revision.is_none() {
                return Err(missing_revision_state());
            }
        }
        DatabaseBackend::Sqlite => {
            // SQLite has no SELECT FOR UPDATE. Reserve the database writer through
            // the dedicated revision row instead of touching the subject row and
            // invoking unrelated Forum UPDATE triggers.
            let reserve_sql = format!(
                "UPDATE {revision_table} SET revision = revision WHERE tenant_id = ? AND {revision_id_column} = ?"
            );
            transaction
                .execute(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    reserve_sql,
                    vec![tenant_id.into(), subject_id.into()],
                ))
                .await
                .map_err(database_error)?;

            let revision_sql = format!(
                "SELECT revision FROM {revision_table} WHERE tenant_id = ? AND {revision_id_column} = ?"
            );
            let revision = transaction
                .query_one(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    revision_sql,
                    vec![tenant_id.into(), subject_id.into()],
                ))
                .await
                .map_err(database_error)?;
            if revision.is_none() {
                return Err(missing_revision_state());
            }

            let subject_sql = format!(
                "SELECT id FROM {subject_table} WHERE tenant_id = ? AND id = ? AND deleted_at IS NULL"
            );
            let subject = transaction
                .query_one(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    subject_sql,
                    vec![tenant_id.into(), subject_id.into()],
                ))
                .await
                .map_err(database_error)?;
            if subject.is_none() {
                return Err(subject_unavailable());
            }
        }
        _ => {
            return Err(PortError::unavailable(
                "forum.moderation_database_backend_unsupported",
                "Forum moderation application database backend is unsupported",
            ));
        }
    }

    Ok(())
}

async fn current_subject_revision(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    kind: ModerationSubjectKind,
    subject_id: Uuid,
) -> Result<i64, PortError> {
    let backend = transaction.get_database_backend();
    let (table, id_column) = revision_table(kind)?;
    let sql = match backend {
        DatabaseBackend::Postgres => {
            format!("SELECT revision FROM {table} WHERE tenant_id = $1 AND {id_column} = $2")
        }
        DatabaseBackend::Sqlite => {
            format!("SELECT revision FROM {table} WHERE tenant_id = ? AND {id_column} = ?")
        }
        _ => {
            return Err(PortError::unavailable(
                "forum.moderation_database_backend_unsupported",
                "Forum moderation application database backend is unsupported",
            ));
        }
    };

    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![tenant_id.into(), subject_id.into()],
        ))
        .await
        .map_err(database_error)?
        .ok_or_else(missing_revision_state)?;
    let revision: i64 = row.try_get("", "revision").map_err(database_error)?;
    if revision <= 0 {
        return Err(PortError::invariant_violation(
            "forum.moderation_subject_revision_invalid",
            "Forum moderation subject revision must be positive",
        ));
    }
    Ok(revision)
}

fn subject_table(kind: ModerationSubjectKind) -> Result<&'static str, PortError> {
    match kind {
        ModerationSubjectKind::ForumTopic => Ok("forum_topics"),
        ModerationSubjectKind::ForumPost => Ok("forum_replies"),
        _ => Err(subject_mismatch()),
    }
}

fn revision_table(kind: ModerationSubjectKind) -> Result<(&'static str, &'static str), PortError> {
    match kind {
        ModerationSubjectKind::ForumTopic => {
            Ok(("forum_topic_moderation_subject_revisions", "topic_id"))
        }
        ModerationSubjectKind::ForumPost => {
            Ok(("forum_reply_moderation_subject_revisions", "reply_id"))
        }
        _ => Err(subject_mismatch()),
    }
}

fn validate_command_for_adapter(
    key: &ModerationSubjectAdapterKey,
    command: &ApplyModerationDecisionCommand,
) -> Result<(), PortError> {
    if command.decision_id.is_nil() || command.subject.id.is_nil() || command.subject.revision <= 0
    {
        return Err(PortError::validation(
            "forum.moderation_identity_invalid",
            "moderation decision and Forum subject identities must be non-nil and revisioned",
        ));
    }
    if command.subject.module != key.module() || command.subject.kind != key.kind() {
        return Err(subject_mismatch());
    }
    if command.decision_hash.len() != 64
        || !command
            .decision_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(PortError::validation(
            "forum.moderation_decision_hash_invalid",
            "moderation decision hash must be canonical lowercase SHA-256 hex",
        ));
    }
    command
        .effect
        .validate_for_decision_kind(command.decision_kind)
        .map_err(|_| {
            PortError::validation(
                "forum.moderation_effect_invalid",
                "moderation decision kind and effect are incompatible",
            )
        })?;
    Ok(())
}

fn validate_trusted_caller(context: &PortContext) -> Result<(), PortError> {
    match &context.actor.kind {
        PortActorKind::Service | PortActorKind::System => Ok(()),
        PortActorKind::User => Err(PortError::forbidden(
            "forum.moderation_application_caller_forbidden",
            "Forum moderation decision application is restricted to trusted orchestration callers",
        )),
    }
}

fn trusted_actor_uuid(context: &PortContext) -> Option<Uuid> {
    Uuid::parse_str(context.actor.id.trim())
        .ok()
        .filter(|actor_id| !actor_id.is_nil())
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    let tenant_id = Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
        PortError::validation(
            "forum.moderation_tenant_invalid",
            "moderation application tenant must be a UUID",
        )
    })?;
    if tenant_id.is_nil() {
        return Err(PortError::validation(
            "forum.moderation_tenant_invalid",
            "moderation application tenant must be non-nil",
        ));
    }
    Ok(tenant_id)
}

fn decode_replay(value: serde_json::Value) -> Result<ModerationDecisionApplication, PortError> {
    serde_json::from_value(value).map_err(|_| {
        PortError::invariant_violation(
            "forum.moderation_application_receipt_corrupt",
            "stored Forum moderation application receipt is invalid",
        )
    })
}

fn forum_error(error: ForumError) -> PortError {
    match error {
        ForumError::TopicNotFound(_)
        | ForumError::ReplyNotFound(_)
        | ForumError::TopicDeleted
        | ForumError::ReplyDeleted => subject_unavailable(),
        ForumError::Validation(_)
        | ForumError::InvalidTopicTransition(_)
        | ForumError::InvalidReplyTransition(_) => PortError::conflict(
            "forum.moderation_domain_state_conflict",
            "Forum subject state conflicts with the moderation decision",
        ),
        ForumError::Forbidden(_) => PortError::forbidden(
            "forum.moderation_domain_forbidden",
            "Forum subject moderation operation is not permitted",
        ),
        other if other.is_retryable() => PortError::unavailable(
            "forum.moderation_domain_unavailable",
            "Forum moderation application is temporarily unavailable",
        ),
        _ => PortError::invariant_violation(
            "forum.moderation_domain_invariant",
            "Forum moderation application could not be completed safely",
        ),
    }
}

fn database_error(_error: sea_orm::DbErr) -> PortError {
    PortError::unavailable(
        "forum.moderation_database_unavailable",
        "Forum moderation application storage is temporarily unavailable",
    )
}

fn subject_unavailable() -> PortError {
    PortError::not_found(
        "forum.moderation_subject_unavailable",
        "Forum moderation subject is unavailable",
    )
}

fn missing_revision_state() -> PortError {
    PortError::invariant_violation(
        "forum.moderation_subject_revision_missing",
        "Forum moderation subject revision state is missing",
    )
}

fn subject_mismatch() -> PortError {
    PortError::validation(
        "forum.moderation_subject_mismatch",
        "moderation decision does not target this Forum subject adapter",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;
    use rustok_moderation_api::{
        ModerationDecisionEffect, ModerationDecisionKind, ModerationReasonCode,
        ModerationSubjectRef,
    };

    fn command(kind: ModerationSubjectKind) -> ApplyModerationDecisionCommand {
        ApplyModerationDecisionCommand {
            decision_id: Uuid::new_v4(),
            subject: ModerationSubjectRef {
                module: FORUM_MODERATION_MODULE.to_string(),
                kind,
                id: Uuid::new_v4(),
                revision: 1,
            },
            decision_kind: ModerationDecisionKind::Warning,
            reason_code: ModerationReasonCode::Other,
            effect: ModerationDecisionEffect::v1(ModerationDecisionEffectAction::NoDomainMutation)
                .expect("valid effect"),
            decision_hash: "a".repeat(64),
        }
    }

    #[test]
    fn adapter_accepts_only_matching_subject_kind() {
        let factory = ForumModerationSubjectAdapterFactory::topic();
        let topic = command(ModerationSubjectKind::ForumTopic);
        assert!(validate_command_for_adapter(&factory.adapter_key(), &topic).is_ok());
        let reply = command(ModerationSubjectKind::ForumPost);
        assert!(validate_command_for_adapter(&factory.adapter_key(), &reply).is_err());
    }

    #[test]
    fn direct_user_callers_are_rejected() {
        let context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user(Uuid::new_v4().to_string()),
            "en",
            Uuid::new_v4().to_string(),
        );
        assert!(validate_trusted_caller(&context).is_err());
    }
}
