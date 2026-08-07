use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{
    HostRuntimeContext, PortActorKind, PortCallPolicy, PortContext, PortError,
};
use rustok_moderation_api::{
    ApplyModerationDecisionCommand, ModerationDecisionApplication, ModerationDecisionEffectAction,
    ModerationSubjectAdapterBuildError, ModerationSubjectAdapterFactory,
    ModerationSubjectAdapterKey, ModerationSubjectCommandPort, ModerationSubjectKind,
};
use rustok_outbox::idempotency;
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    IsolationLevel, Statement, TransactionTrait,
};
use uuid::Uuid;

use crate::error::ForumError;
use crate::services::projection_invalidation::publish_forum_topic_projection_direct_in_tx;
use crate::services::TopicService;

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

        let expected_idempotency_key = command.decision_id.to_string();
        if context.idempotency_key.as_deref() != Some(expected_idempotency_key.as_str()) {
            return Err(PortError::validation(
                "forum.moderation_decision_idempotency_mismatch",
                "moderation decision UUID must equal the port idempotency key",
            ));
        }

        let lease = match idempotency::admit(
            &self.db,
            tenant_id,
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

        let result = self.execute_apply(tenant_id, lease, &command).await;
        if let Err(error) = &result {
            // Retryable storage/serialization failures leave the processing lease
            // reclaimable instead of freezing a transient failure into the
            // decision's immutable replay result.
            if !error.retryable {
                if let Err(receipt_error) = idempotency::fail(&self.db, lease, error).await {
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
        lease: idempotency::Lease,
        command: &ApplyModerationDecisionCommand,
    ) -> Result<ModerationDecisionApplication, PortError> {
        let transaction = begin_application_transaction(&self.db).await?;
        let result = apply_inside_transaction(&transaction, tenant_id, &self.key, command).await;
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
    key: &ModerationSubjectAdapterKey,
    command: &ApplyModerationDecisionCommand,
) -> Result<ModerationDecisionApplication, PortError> {
    lock_active_subject_row(transaction, tenant_id, key.kind(), command.subject.id).await?;
    let reviewed_revision = current_subject_revision(
        transaction,
        tenant_id,
        key.kind(),
        command.subject.id,
    )
    .await?;
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
                TopicService::set_locked_in_tx(
                    transaction,
                    tenant_id,
                    command.subject.id,
                    true,
                )
                .await
                .map_err(forum_error)?;
                publish_forum_topic_projection_direct_in_tx(
                    transaction,
                    tenant_id,
                    None,
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
        _ => {
            return Err(PortError::validation(
                "forum.moderation_effect_unsupported",
                "the requested moderation effect is not supported by this Forum subject adapter",
            ));
        }
    };

    let applied_revision = current_subject_revision(
        transaction,
        tenant_id,
        key.kind(),
        command.subject.id,
    )
    .await?;
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

async fn lock_active_subject_row(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    kind: ModerationSubjectKind,
    subject_id: Uuid,
) -> Result<(), PortError> {
    let backend = transaction.get_database_backend();
    let table = subject_table(kind)?;
    let row = match backend {
        DatabaseBackend::Postgres => {
            let sql = format!(
                "SELECT id FROM {table} WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE"
            );
            transaction
                .query_one(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    sql,
                    vec![tenant_id.into(), subject_id.into()],
                ))
                .await
                .map_err(database_error)?
        }
        DatabaseBackend::Sqlite => {
            let reserve = format!(
                "UPDATE {table} SET updated_at = updated_at WHERE tenant_id = ? AND id = ? AND deleted_at IS NULL"
            );
            transaction
                .execute(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    reserve,
                    vec![tenant_id.into(), subject_id.into()],
                ))
                .await
                .map_err(database_error)?;
            let sql = format!(
                "SELECT id FROM {table} WHERE tenant_id = ? AND id = ? AND deleted_at IS NULL"
            );
            transaction
                .query_one(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    sql,
                    vec![tenant_id.into(), subject_id.into()],
                ))
                .await
                .map_err(database_error)?
        }
        _ => {
            return Err(PortError::unavailable(
                "forum.moderation_database_backend_unsupported",
                "Forum moderation application database backend is unsupported",
            ));
        }
    };

    if row.is_none() {
        return Err(subject_unavailable());
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
        DatabaseBackend::Postgres => format!(
            "SELECT revision FROM {table} WHERE tenant_id = $1 AND {id_column} = $2"
        ),
        DatabaseBackend::Sqlite => format!(
            "SELECT revision FROM {table} WHERE tenant_id = ? AND {id_column} = ?"
        ),
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
        .ok_or_else(|| {
            PortError::invariant_violation(
                "forum.moderation_subject_revision_missing",
                "Forum moderation subject revision state is missing",
            )
        })?;
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

fn revision_table(
    kind: ModerationSubjectKind,
) -> Result<(&'static str, &'static str), PortError> {
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
    if command.decision_id.is_nil() || command.subject.id.is_nil() || command.subject.revision <= 0 {
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
            effect: ModerationDecisionEffect::v1(
                ModerationDecisionEffectAction::NoDomainMutation,
            )
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
