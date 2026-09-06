use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{HostRuntimeContext, PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_moderation_api::{
    ApplyModerationDecisionCommand, ModerationDecisionApplication, ModerationDecisionEffectAction,
    ModerationScopeKind, ModerationScopeRef, ModerationSubjectAdapterBuildError,
    ModerationSubjectAdapterFactory, ModerationSubjectAdapterKey, ModerationSubjectCommandPort,
    ModerationSubjectKind, moderation_scope_from_claims,
};
use rustok_outbox::idempotency;
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
use serde::Serialize;
use uuid::Uuid;

use crate::domain::GroupMembershipEnforcementSourceKind;
use crate::error::GroupsError;
use crate::membership_enforcement_command::{
    MembershipEnforcementProvenance, apply_membership_suspension_in_tx,
};
use crate::membership_enforcement_transaction::lock_membership_enforcement_target_by_id_for_update;

pub const GROUPS_MODERATION_MODULE: &str = "groups";
const GROUPS_OWNER_SLUG: &str = "groups";
const MODERATION_DISPATCH_ACTOR: &str = "rustok-moderation";
const APPLY_MODERATION_DECISION_OPERATION: &str = "apply_moderation_decision";

#[derive(Clone, Copy, Debug, Default)]
pub struct GroupsModerationSubjectAdapterFactory;

impl GroupsModerationSubjectAdapterFactory {
    fn adapter_key(self) -> ModerationSubjectAdapterKey {
        ModerationSubjectAdapterKey::new(
            GROUPS_MODERATION_MODULE,
            ModerationSubjectKind::GroupMembership,
        )
        .expect("static Groups moderation adapter key is valid")
    }
}

impl ModerationSubjectAdapterFactory for GroupsModerationSubjectAdapterFactory {
    fn key(&self) -> ModerationSubjectAdapterKey {
        self.adapter_key()
    }

    fn build(
        &self,
        host: &HostRuntimeContext,
    ) -> Result<Arc<dyn ModerationSubjectCommandPort>, ModerationSubjectAdapterBuildError> {
        Ok(Arc::new(GroupsModerationSubjectAdapter {
            db: host.db_clone(),
            key: self.adapter_key(),
        }))
    }
}

#[derive(Clone)]
struct GroupsModerationSubjectAdapter {
    db: DatabaseConnection,
    key: ModerationSubjectAdapterKey,
}

#[derive(Serialize)]
struct GroupsModerationReceiptRequest<'a> {
    scope: &'a ModerationScopeRef,
    command: &'a ApplyModerationDecisionCommand,
}

#[async_trait]
impl ModerationSubjectCommandPort for GroupsModerationSubjectAdapter {
    fn key(&self) -> ModerationSubjectAdapterKey {
        self.key.clone()
    }

    async fn apply_moderation_decision(
        &self,
        context: PortContext,
        command: ApplyModerationDecisionCommand,
    ) -> Result<ModerationDecisionApplication, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let provenance = trusted_moderation_provenance(&context)?;
        validate_command_for_adapter(&self.key, &command)?;
        let provenance = bind_decision_provenance(provenance, &command);
        let tenant_id = parse_tenant_id(&context)?;
        let scope = moderation_scope_from_claims(&context.claims).map_err(|_| {
            PortError::validation(
                "groups.moderation_scope_claim_invalid",
                "trusted moderation application context must carry exactly one canonical scope claim",
            )
        })?;
        let group_id = require_group_scope(&scope)?;

        let expected_idempotency_key = command.decision_id.to_string();
        if context.idempotency_key.as_deref() != Some(expected_idempotency_key.as_str()) {
            return Err(PortError::validation(
                "groups.moderation_decision_idempotency_mismatch",
                "moderation decision UUID must equal the port idempotency key",
            ));
        }

        let receipt_request = GroupsModerationReceiptRequest {
            scope: &scope,
            command: &command,
        };
        let lease = match idempotency::admit(
            &self.db,
            idempotency::OwnerOperationScope::Tenant(tenant_id),
            GROUPS_OWNER_SLUG,
            expected_idempotency_key.as_str(),
            APPLY_MODERATION_DECISION_OPERATION,
            &receipt_request,
        )
        .await?
        {
            idempotency::Admission::Run(lease) => lease,
            idempotency::Admission::Replay(value) => return decode_replay(value),
            idempotency::Admission::ReplayError(error) => return Err(error),
        };

        let result = self
            .execute_apply(tenant_id, group_id, provenance, lease, &context, &command)
            .await;
        if let Err(error) = &result {
            if !error.retryable {
                let _ = idempotency::fail(&self.db, lease, error).await;
            }
        }
        result
    }
}

impl GroupsModerationSubjectAdapter {
    async fn execute_apply(
        &self,
        tenant_id: Uuid,
        group_id: Uuid,
        provenance: MembershipEnforcementProvenance,
        lease: idempotency::Lease,
        context: &PortContext,
        command: &ApplyModerationDecisionCommand,
    ) -> Result<ModerationDecisionApplication, PortError> {
        let transaction = self.db.begin().await.map_err(database_error)?;
        let result = apply_inside_transaction(
            &transaction,
            tenant_id,
            group_id,
            provenance,
            context,
            command,
        )
        .await;
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
    group_id: Uuid,
    provenance: MembershipEnforcementProvenance,
    context: &PortContext,
    command: &ApplyModerationDecisionCommand,
) -> Result<ModerationDecisionApplication, PortError> {
    let effective_until = match &command.effect.action {
        ModerationDecisionEffectAction::SuspendSubject { effective_until } => {
            effective_until.clone()
        }
        _ => {
            return Err(PortError::validation(
                "groups.moderation_effect_unsupported",
                "Groups membership moderation currently supports only SuspendSubject",
            ));
        }
    };

    let target = lock_membership_enforcement_target_by_id_for_update(
        transaction,
        tenant_id,
        command.subject.id,
    )
    .await
    .map_err(groups_error)?
    .ok_or_else(subject_unavailable)?;

    if target.group.id != group_id || target.membership.group_id != group_id {
        return Err(PortError::conflict(
            "groups.moderation_scope_mismatch",
            "moderation decision group scope does not own the target membership",
        ));
    }
    if target.membership.revision != command.subject.revision {
        return Err(PortError::conflict(
            "groups.moderation_subject_revision_conflict",
            "group membership changed after the moderation decision was reviewed",
        ));
    }

    let now = Utc::now();
    let result = apply_membership_suspension_in_tx(
        transaction,
        context,
        target.group,
        target.membership,
        target.enforcement,
        command.subject.revision,
        command.reason_code.as_str().to_string(),
        effective_until,
        provenance,
        now,
    )
    .await
    .map_err(groups_error)?;

    if result.membership_id != command.subject.id {
        return Err(PortError::invariant_violation(
            "groups.moderation_subject_identity_changed",
            "Groups moderation mutation returned another membership identity",
        ));
    }
    if result.membership_revision <= command.subject.revision {
        return Err(PortError::invariant_violation(
            "groups.moderation_subject_revision_not_advanced",
            "Groups moderation mutation did not advance the membership revision",
        ));
    }

    Ok(ModerationDecisionApplication {
        decision_id: command.decision_id,
        subject: command.subject.clone(),
        applied_revision: result.membership_revision,
        applied_at: now,
    })
}

fn validate_command_for_adapter(
    key: &ModerationSubjectAdapterKey,
    command: &ApplyModerationDecisionCommand,
) -> Result<(), PortError> {
    if command.decision_id.is_nil() || command.subject.id.is_nil() || command.subject.revision <= 0
    {
        return Err(PortError::validation(
            "groups.moderation_identity_invalid",
            "moderation decision and Groups membership identities must be non-nil and revisioned",
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
            "groups.moderation_decision_hash_invalid",
            "moderation decision hash must be canonical lowercase SHA-256 hex",
        ));
    }
    command
        .effect
        .validate_for_decision_kind(command.decision_kind)
        .map_err(|_| {
            PortError::validation(
                "groups.moderation_effect_invalid",
                "moderation decision kind and effect are incompatible",
            )
        })?;
    Ok(())
}

fn trusted_moderation_provenance(
    context: &PortContext,
) -> Result<MembershipEnforcementProvenance, PortError> {
    let (actor_kind, actor_id) = match &context.actor.kind {
        PortActorKind::Service if context.actor.id == MODERATION_DISPATCH_ACTOR => {
            ("service", context.actor.id.clone())
        }
        PortActorKind::System if context.actor.id == "system" => {
            ("system", context.actor.id.clone())
        }
        _ => {
            return Err(PortError::forbidden(
                "groups.moderation_application_caller_forbidden",
                "Groups moderation decision application is restricted to trusted Moderation orchestration callers",
            ));
        }
    };
    Ok(MembershipEnforcementProvenance {
        source_kind: GroupMembershipEnforcementSourceKind::ModerationDecision,
        moderation_decision_id: None,
        moderation_decision_hash: None,
        actor_kind: actor_kind.to_string(),
        actor_id,
        audit_actor_user_id: None,
    })
}

fn bind_decision_provenance(
    mut provenance: MembershipEnforcementProvenance,
    command: &ApplyModerationDecisionCommand,
) -> MembershipEnforcementProvenance {
    provenance.moderation_decision_id = Some(command.decision_id);
    provenance.moderation_decision_hash = Some(command.decision_hash.clone());
    provenance
}

fn require_group_scope(scope: &ModerationScopeRef) -> Result<Uuid, PortError> {
    match (scope.kind, scope.id) {
        (ModerationScopeKind::Group, Some(group_id)) if !group_id.is_nil() => Ok(group_id),
        _ => Err(PortError::validation(
            "groups.moderation_scope_mismatch",
            "Groups membership moderation requires an exact non-nil Group scope",
        )),
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    let tenant_id = Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
        PortError::validation(
            "groups.moderation_tenant_invalid",
            "moderation application tenant must be a UUID",
        )
    })?;
    if tenant_id.is_nil() {
        return Err(PortError::validation(
            "groups.moderation_tenant_invalid",
            "moderation application tenant must be non-nil",
        ));
    }
    Ok(tenant_id)
}

fn decode_replay(value: serde_json::Value) -> Result<ModerationDecisionApplication, PortError> {
    serde_json::from_value(value).map_err(|_| {
        PortError::invariant_violation(
            "groups.moderation_application_receipt_corrupt",
            "stored Groups moderation application receipt is invalid",
        )
    })
}

fn groups_error(error: GroupsError) -> PortError {
    error.into()
}

fn database_error(error: sea_orm::DbErr) -> PortError {
    GroupsError::Persistence(error.to_string()).into()
}

fn subject_unavailable() -> PortError {
    PortError::not_found(
        "groups.moderation_subject_unavailable",
        "Groups membership moderation subject is unavailable",
    )
}

fn subject_mismatch() -> PortError {
    PortError::validation(
        "groups.moderation_subject_mismatch",
        "moderation decision does not target the Groups membership adapter",
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
                module: GROUPS_MODERATION_MODULE.to_string(),
                kind,
                id: Uuid::new_v4(),
                revision: 3,
            },
            decision_kind: ModerationDecisionKind::SuspendSubject,
            reason_code: ModerationReasonCode::Harassment,
            effect: ModerationDecisionEffect::v1(ModerationDecisionEffectAction::SuspendSubject {
                effective_until: None,
            })
            .expect("valid effect"),
            decision_hash: "a".repeat(64),
        }
    }

    #[test]
    fn adapter_key_is_exact_group_membership() {
        let key = GroupsModerationSubjectAdapterFactory.adapter_key();
        assert_eq!(key.module(), GROUPS_MODERATION_MODULE);
        assert_eq!(key.kind(), ModerationSubjectKind::GroupMembership);
    }

    #[test]
    fn adapter_rejects_other_subject_kinds() {
        let key = GroupsModerationSubjectAdapterFactory.adapter_key();
        assert!(
            validate_command_for_adapter(&key, &command(ModerationSubjectKind::GroupMembership))
                .is_ok()
        );
        assert!(
            validate_command_for_adapter(&key, &command(ModerationSubjectKind::Group)).is_err()
        );
    }

    #[test]
    fn direct_users_and_unrelated_services_are_not_trusted_dispatchers() {
        let tenant_id = Uuid::new_v4();
        for actor in [
            PortActor::user(Uuid::new_v4().to_string()),
            PortActor::service("another-service"),
        ] {
            let context = PortContext::new(tenant_id.to_string(), actor, "und", "test");
            assert!(trusted_moderation_provenance(&context).is_err());
        }
        let trusted = PortContext::new(
            tenant_id.to_string(),
            PortActor::service(MODERATION_DISPATCH_ACTOR),
            "und",
            "test",
        );
        let command = command(ModerationSubjectKind::GroupMembership);
        let provenance = bind_decision_provenance(
            trusted_moderation_provenance(&trusted).expect("trusted provenance"),
            &command,
        );
        assert_eq!(
            provenance.source_kind,
            GroupMembershipEnforcementSourceKind::ModerationDecision
        );
        assert_eq!(provenance.moderation_decision_id, Some(command.decision_id));
        assert_eq!(
            provenance.moderation_decision_hash.as_deref(),
            Some(command.decision_hash.as_str())
        );
    }
}
