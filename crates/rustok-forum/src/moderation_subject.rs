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
    AccessMode, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, EntityTrait, IsolationLevel, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait, sea_query::Expr,
};
use uuid::Uuid;

use crate::entities::{forum_reply, forum_reply_revision, forum_topic, forum_topic_revision};
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
        validate_command_for_adapter(&self.key, command)?;

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
    lock_subject_row(transaction, tenant_id, key.kind(), command.subject.id).await?;
    let current_revision = current_subject_revision(
        transaction,
        tenant_id,
        key.kind(),
        command.subject.id,
    )
    .await?;
    if current_revision != command.subject.revision {
        return Err(PortError::conflict(
            "forum.moderation_subject_revision_conflict",
            "Forum subject changed after the moderation decision was reviewed",
        ));
    }

    match (key.kind(), &command.effect.action) {
        (_, ModerationDecisionEffectAction::NoDomainMutation) => {}
        (
            ModerationSubjectKind::ForumTopic,
            ModerationDecisionEffectAction::Lock {
                effective_until: None,
            },
        ) => {
            let topic = TopicService::find_topic_in_tx(transaction, tenant_id, command.subject.id)
                .await
                .map_err(forum_error)?;
            if topic.deleted_at.is_some() {
                return Err(subject_unavailable());
            }
            if !topic.is_locked {
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
    }

    Ok(ModerationDecisionApplication {
        decision_id: command.decision_id,
        subject: command.subject.clone(),
        applied_revision: current_revision,
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

async fn lock_subject_row(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    kind: ModerationSubjectKind,
    subject_id: Uuid,
) -> Result<(), PortError> {
    let exists = match kind {
        ModerationSubjectKind::ForumTopic => {
            if transaction.get_database_backend() == DatabaseBackend::Sqlite {
                // SQLite has no SELECT FOR UPDATE. Follow the established owner
                // lock protocol: reserve the writer with a no-op assignment and
                // verify existence separately because rows_affected may be zero.
                forum_topic::Entity::update_many()
                    .col_expr(
                        forum_topic::Column::UpdatedAt,
                        Expr::col(forum_topic::Column::UpdatedAt),
                    )
                    .filter(forum_topic::Column::TenantId.eq(tenant_id))
                    .filter(forum_topic::Column::Id.eq(subject_id))
                    .filter(forum_topic::Column::DeletedAt.is_null())
                    .exec(transaction)
                    .await
                    .map_err(database_error)?;
                forum_topic::Entity::find_by_id(subject_id)
                    .filter(forum_topic::Column::TenantId.eq(tenant_id))
                    .filter(forum_topic::Column::DeletedAt.is_null())
                    .one(transaction)
                    .await
                    .map_err(database_error)?
                    .is_some()
            } else {
                forum_topic::Entity::find_by_id(subject_id)
                    .filter(forum_topic::Column::TenantId.eq(tenant_id))
                    .filter(forum_topic::Column::DeletedAt.is_null())
                    .lock_exclusive()
                    .one(transaction)
                    .await
                    .map_err(database_error)?
                    .is_some()
            }
        }
        ModerationSubjectKind::ForumPost => {
            if transaction.get_database_backend() == DatabaseBackend::Sqlite {
                forum_reply::Entity::update_many()
                    .col_expr(
                        forum_reply::Column::UpdatedAt,
                        Expr::col(forum_reply::Column::UpdatedAt),
                    )
                    .filter(forum_reply::Column::TenantId.eq(tenant_id))
                    .filter(forum_reply::Column::Id.eq(subject_id))
                    .filter(forum_reply::Column::DeletedAt.is_null())
                    .exec(transaction)
                    .await
                    .map_err(database_error)?;
                forum_reply::Entity::find_by_id(subject_id)
                    .filter(forum_reply::Column::TenantId.eq(tenant_id))
                    .filter(forum_reply::Column::DeletedAt.is_null())
                    .one(transaction)
                    .await
                    .map_err(database_error)?
                    .is_some()
            } else {
                forum_reply::Entity::find_by_id(subject_id)
                    .filter(forum_reply::Column::TenantId.eq(tenant_id))
                    .filter(forum_reply::Column::DeletedAt.is_null())
                    .lock_exclusive()
                    .one(transaction)
                    .await
                    .map_err(database_error)?
                    .is_some()
            }
        }
        _ => return Err(subject_mismatch()),
    };
    if !exists {
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
    let latest = match kind {
        ModerationSubjectKind::ForumTopic => forum_topic_revision::Entity::find()
            .select_only()
            .column(forum_topic_revision::Column::Id)
            .filter(forum_topic_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_revision::Column::TopicId.eq(subject_id))
            .order_by_desc(forum_topic_revision::Column::Id)
            .into_tuple::<i64>()
            .one(transaction)
            .await
            .map_err(database_error)?,
        ModerationSubjectKind::ForumPost => forum_reply_revision::Entity::find()
            .select_only()
            .column(forum_reply_revision::Column::Id)
            .filter(forum_reply_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_revision::Column::ReplyId.eq(subject_id))
            .order_by_desc(forum_reply_revision::Column::Id)
            .into_tuple::<i64>()
            .one(transaction)
            .await
            .map_err(database_error)?,
        _ => return Err(subject_mismatch()),
    };
    latest
        .unwrap_or(0)
        .checked_add(1)
        .filter(|revision| *revision > 0)
        .ok_or_else(|| {
            PortError::invariant_violation(
                "forum.moderation_subject_revision_invalid",
                "Forum subject revision is unavailable",
            )
        })
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
