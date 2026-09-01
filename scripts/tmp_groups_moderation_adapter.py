from pathlib import Path
import json


def replace_once(path: str, old: str, new: str) -> None:
    p = Path(path)
    text = p.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}")
    p.write_text(text.replace(old, new, 1))


def require_absent(path: str) -> None:
    if Path(path).exists():
        raise SystemExit(f"{path}: expected path to be absent")


model = "crates/rustok-moderation-api/src/model.rs"
dispatch = "crates/rustok-moderation/src/application_dispatch.rs"
groups_cargo = "crates/rustok-groups/Cargo.toml"
groups_lib = "crates/rustok-groups/src/lib.rs"
groups_readme = "crates/rustok-groups/README.md"
groups_plan = "crates/rustok-groups/docs/implementation-plan.md"
moderation_plan = "crates/rustok-moderation/docs/implementation-plan.md"
groups_contract = "crates/rustok-groups/contracts/groups-effective-membership-access.json"
adapter_path = "crates/rustok-groups/src/moderation_subject.rs"
verifier_path = "scripts/verify/verify-groups-moderation-subject-adapter.mjs"

# Neutral, versioned scope fact carried in PortContext without changing the serialized
# ApplyModerationDecisionCommand shape used by historical Forum owner receipts.
scope_anchor = '''impl ModerationScopeRef {
    pub const fn platform() -> Self {
        Self {
            kind: ModerationScopeKind::Platform,
            id: None,
        }
    }
}
'''
scope_block = scope_anchor + '''

/// Reserved trusted-context claim prefix for the immutable moderation case scope.
///
/// The scope deliberately stays outside `ApplyModerationDecisionCommand`: existing domain
/// receipts bind that command's serialized shape, so extending it would change historical replay
/// request digests. Domain adapters that need scope must bind this canonical claim alongside the
/// command in their own owner receipt.
pub const MODERATION_SCOPE_CLAIM_PREFIX: &str = "moderation.scope.v1:";

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum ModerationScopeClaimError {
    #[error("moderation scope is structurally invalid")]
    InvalidScope,
    #[error("moderation scope claim is missing")]
    MissingClaim,
    #[error("moderation scope claim is duplicated")]
    DuplicateClaim,
    #[error("moderation scope claim is invalid")]
    InvalidClaim,
}

pub fn moderation_scope_claim(
    scope: &ModerationScopeRef,
) -> Result<String, ModerationScopeClaimError> {
    match (scope.kind, scope.id) {
        (ModerationScopeKind::Platform, None) => {
            Ok(format!("{MODERATION_SCOPE_CLAIM_PREFIX}platform"))
        }
        (ModerationScopeKind::Platform, Some(_)) => Err(ModerationScopeClaimError::InvalidScope),
        (kind, Some(id)) if !id.is_nil() => Ok(format!(
            "{MODERATION_SCOPE_CLAIM_PREFIX}{}:{id}",
            kind.as_str()
        )),
        _ => Err(ModerationScopeClaimError::InvalidScope),
    }
}

pub fn moderation_scope_from_claims(
    claims: &[String],
) -> Result<ModerationScopeRef, ModerationScopeClaimError> {
    let mut matching = claims
        .iter()
        .filter_map(|claim| claim.strip_prefix(MODERATION_SCOPE_CLAIM_PREFIX));
    let encoded = matching
        .next()
        .ok_or(ModerationScopeClaimError::MissingClaim)?;
    if matching.next().is_some() {
        return Err(ModerationScopeClaimError::DuplicateClaim);
    }
    if encoded == "platform" {
        return Ok(ModerationScopeRef::platform());
    }

    let (kind, id) = encoded
        .split_once(':')
        .ok_or(ModerationScopeClaimError::InvalidClaim)?;
    if id.contains(':') {
        return Err(ModerationScopeClaimError::InvalidClaim);
    }
    let kind = ModerationScopeKind::parse(kind).ok_or(ModerationScopeClaimError::InvalidClaim)?;
    if kind == ModerationScopeKind::Platform {
        return Err(ModerationScopeClaimError::InvalidClaim);
    }
    let id = Uuid::parse_str(id).map_err(|_| ModerationScopeClaimError::InvalidClaim)?;
    if id.is_nil() {
        return Err(ModerationScopeClaimError::InvalidClaim);
    }
    Ok(ModerationScopeRef { kind, id: Some(id) })
}
'''
replace_once(model, scope_anchor, scope_block)

replace_once(
    model,
    '''    #[test]\n    fn suspension_requires_matching_decision_kind() {''',
    '''    #[test]
    fn scope_claim_round_trips_group_identity() {
        let id = Uuid::new_v4();
        let scope = ModerationScopeRef {
            kind: ModerationScopeKind::Group,
            id: Some(id),
        };
        let claim = moderation_scope_claim(&scope).expect("scope claim");
        assert_eq!(
            moderation_scope_from_claims(&[claim]).expect("decoded scope"),
            scope
        );
    }

    #[test]
    fn scope_claim_fails_closed_on_missing_duplicate_or_invalid_identity() {
        assert_eq!(
            moderation_scope_from_claims(&[]),
            Err(ModerationScopeClaimError::MissingClaim)
        );
        let scope = ModerationScopeRef {
            kind: ModerationScopeKind::Group,
            id: Some(Uuid::new_v4()),
        };
        let claim = moderation_scope_claim(&scope).expect("scope claim");
        assert_eq!(
            moderation_scope_from_claims(&[claim.clone(), claim]),
            Err(ModerationScopeClaimError::DuplicateClaim)
        );
        assert_eq!(
            moderation_scope_from_claims(&[format!(
                "{MODERATION_SCOPE_CLAIM_PREFIX}group:{}",
                Uuid::nil()
            )]),
            Err(ModerationScopeClaimError::InvalidClaim)
        );
        assert_eq!(
            moderation_scope_claim(&ModerationScopeRef {
                kind: ModerationScopeKind::Platform,
                id: Some(Uuid::new_v4()),
            }),
            Err(ModerationScopeClaimError::InvalidScope)
        );
    }

    #[test]
    fn suspension_requires_matching_decision_kind() {''',
)

# Moderation dispatcher propagates the already-validated immutable case scope through the
# trusted context. The existing command stays byte-shape compatible for domain receipts.
replace_once(
    dispatch,
    '''use rustok_moderation_api::{
    ApplyModerationDecisionCommand, ModerationDecisionApplication, ModerationSubjectAdapterRegistry,
};''',
    '''use rustok_moderation_api::{
    ApplyModerationDecisionCommand, ModerationDecisionApplication, ModerationSubjectAdapterRegistry,
    moderation_scope_claim,
};''',
)
replace_once(
    dispatch,
    '''        let command = match self
            .reconstruct_application_command(tenant_id, &operation)
            .await
        {
            Ok(command) => command,''',
    '''        let (command, scope_claim) = match self
            .reconstruct_application_command(tenant_id, &operation)
            .await
        {
            Ok(reconstructed) => reconstructed,''',
)
replace_once(
    dispatch,
    '''        let context = application_port_context(tenant_id, decision_id, lease_token);''',
    '''        let context = application_port_context(tenant_id, decision_id, lease_token, scope_claim);''',
)
replace_once(
    dispatch,
    '''    ) -> ModerationResult<ApplyModerationDecisionCommand> {''',
    '''    ) -> ModerationResult<(ApplyModerationDecisionCommand, String)> {''',
)
replace_once(
    dispatch,
    '''        if case.subject != operation.subject {
            return Err(ModerationError::Invariant(
                "application operation subject does not match its moderation case".to_string(),
            ));
        }

        let effect = decision.effect.ok_or_else(|| {''',
    '''        if case.subject != operation.subject {
            return Err(ModerationError::Invariant(
                "application operation subject does not match its moderation case".to_string(),
            ));
        }
        let scope_claim = moderation_scope_claim(&case.scope)
            .map_err(|error| ModerationError::Invariant(error.to_string()))?;

        let effect = decision.effect.ok_or_else(|| {''',
)
replace_once(
    dispatch,
    '''        Ok(ApplyModerationDecisionCommand {
            decision_id: decision.id,
            subject: operation.subject.clone(),
            decision_kind: decision.decision_kind,
            reason_code: decision.reason_code,
            effect,
            decision_hash: decision.decision_hash,
        })''',
    '''        Ok((
            ApplyModerationDecisionCommand {
                decision_id: decision.id,
                subject: operation.subject.clone(),
                decision_kind: decision.decision_kind,
                reason_code: decision.reason_code,
                effect,
                decision_hash: decision.decision_hash,
            },
            scope_claim,
        ))''',
)
replace_once(
    dispatch,
    '''fn application_port_context(tenant_id: Uuid, decision_id: Uuid, lease_token: Uuid) -> PortContext {''',
    '''fn application_port_context(
    tenant_id: Uuid,
    decision_id: Uuid,
    lease_token: Uuid,
    scope_claim: String,
) -> PortContext {''',
)
replace_once(
    dispatch,
    '''    .with_causation_id(decision_id.to_string())
    .with_idempotency_key(decision_id.to_string())''',
    '''    .with_causation_id(decision_id.to_string())
    .with_claim(scope_claim)
    .with_idempotency_key(decision_id.to_string())''',
)
replace_once(
    dispatch,
    '''    use rustok_api::PortActorKind;

    #[test]
    fn retry_delay_is_bounded_exponential() {''',
    '''    use rustok_api::PortActorKind;
    use rustok_moderation_api::ModerationScopeRef;

    #[test]
    fn retry_delay_is_bounded_exponential() {''',
)
replace_once(
    dispatch,
    '''        let first = application_port_context(tenant_id, decision_id, Uuid::new_v4());
        let second = application_port_context(tenant_id, decision_id, Uuid::new_v4());''',
    '''        let scope_claim = moderation_scope_claim(&ModerationScopeRef::platform())
            .expect("platform scope claim");
        let first = application_port_context(
            tenant_id,
            decision_id,
            Uuid::new_v4(),
            scope_claim.clone(),
        );
        let second = application_port_context(
            tenant_id,
            decision_id,
            Uuid::new_v4(),
            scope_claim.clone(),
        );''',
)
replace_once(
    dispatch,
    '''        assert_eq!(second.idempotency_key, first.idempotency_key);
        assert_ne!(second.correlation_id, first.correlation_id);''',
    '''        assert_eq!(second.idempotency_key, first.idempotency_key);
        assert_eq!(first.claims, vec![scope_claim]);
        assert_ne!(second.correlation_id, first.correlation_id);''',
)

# Groups acquires only neutral Moderation API + shared owner receipt support.
replace_once(
    groups_cargo,
    '''rustok-core.workspace = true
rustok-notifications-api.workspace = true''',
    '''rustok-core.workspace = true
rustok-moderation-api = { path = "../rustok-moderation-api" }
rustok-notifications-api.workspace = true
rustok-outbox.workspace = true''',
)

# Register one GroupMembership factory through the same host-owned registry as Forum.
replace_once(
    groups_lib,
    '''use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use rustok_notifications_api::register_notification_source_provider_factory;''',
    '''use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use rustok_moderation_api::register_moderation_subject_adapter_factory;
use rustok_notifications_api::register_notification_source_provider_factory;''',
)
replace_once(
    groups_lib,
    '''pub mod membership_enforcement;
mod membership_enforcement_command;
pub mod membership_enforcement_entities;''',
    '''pub mod membership_enforcement;
mod membership_enforcement_command;
pub mod membership_enforcement_entities;
mod moderation_subject;''',
)
replace_once(
    groups_lib,
    '''pub use membership_enforcement_command::{
    GroupMembershipEnforcementCommandService, GroupMembershipEnforcementMutationResult,
    RevokeGroupMembershipSuspensionRequest, SuspendGroupMembershipRequest,
};
pub use policy_history::*;''',
    '''pub use membership_enforcement_command::{
    GroupMembershipEnforcementCommandService, GroupMembershipEnforcementMutationResult,
    RevokeGroupMembershipSuspensionRequest, SuspendGroupMembershipRequest,
};
pub use moderation_subject::GroupsModerationSubjectAdapterFactory;
pub use policy_history::*;''',
)
replace_once(
    groups_lib,
    '''        register_notification_source_provider_factory(
            extensions,
            notification_source::GroupsNotificationSourceProviderFactory,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "groups notification source factory registration failed: {error}"
            ))
        })?;
        Ok(())''',
    '''        register_notification_source_provider_factory(
            extensions,
            notification_source::GroupsNotificationSourceProviderFactory,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "groups notification source factory registration failed: {error}"
            ))
        })?;
        register_moderation_subject_adapter_factory(
            extensions,
            moderation_subject::GroupsModerationSubjectAdapterFactory,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "groups moderation subject factory registration failed: {error}"
            ))
        })?;
        Ok(())''',
)

require_absent(adapter_path)
Path(adapter_path).write_text(r'''use std::sync::Arc;

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
use crate::error::{GroupsError, GroupsResult};
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
        ModerationDecisionEffectAction::SuspendSubject { effective_until } => effective_until.clone(),
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
    if command.decision_id.is_nil() || command.subject.id.is_nil() || command.subject.revision <= 0 {
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
        PortActorKind::System if context.actor.id == "system" => ("system", context.actor.id.clone()),
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
            effect: ModerationDecisionEffect::v1(
                ModerationDecisionEffectAction::SuspendSubject {
                    effective_until: None,
                },
            )
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
        assert!(validate_command_for_adapter(&key, &command(ModerationSubjectKind::Group)).is_err());
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
''')

# Bind immutable Moderation decision identity only after command validation; the helper receives the
# fully populated provenance that the existing owner mutation validates and persists.
replace_once(
    adapter_path,
    '''        let provenance = trusted_moderation_provenance(&context)?;
        validate_command_for_adapter(&self.key, &command)?;''',
    '''        let provenance = trusted_moderation_provenance(&context)?;
        validate_command_for_adapter(&self.key, &command)?;
        let provenance = bind_decision_provenance(provenance, &command);''',
)

# Contract/docs: source-complete adapter, runtime evidence deliberately still open.
contract = json.loads(Path(groups_contract).read_text())
remaining = contract.get("remaining_paths")
if not isinstance(remaining, list) or "moderation_subject_adapter" not in remaining:
    raise SystemExit("groups effective contract: moderation_subject_adapter remaining path missing")
contract["remaining_paths"] = [item for item in remaining if item != "moderation_subject_adapter"]
converted = contract.setdefault("converted_source_paths", {})
if not isinstance(converted, dict):
    raise SystemExit("groups effective contract: converted_source_paths must be an object")
converted["moderation_subject_adapter"] = [
    "crates/rustok-groups/src/moderation_subject.rs",
    "crates/rustok-moderation-api/src/model.rs",
    "crates/rustok-moderation/src/application_dispatch.rs",
]
evidence = contract.setdefault("evidence", {})
if not isinstance(evidence, dict):
    raise SystemExit("groups effective contract: evidence must be an object")
evidence["moderation_subject_adapter_static_boundary"] = verifier_path
evidence["moderation_subject_adapter_runtime"] = None
Path(groups_contract).write_text(json.dumps(contract, indent=2) + "\n")

replace_once(
    groups_readme,
    '''  receipt-first replay, local hierarchy, owner protection, group-version advance, audit/events and
  shared owner mutation functions reserved for the later neutral Moderation adapter;''',
    '''  receipt-first replay, local hierarchy, owner protection, group-version advance, audit/events and
  shared owner mutation functions also used by the neutral Moderation adapter;
- neutral `GroupsModerationSubjectAdapterFactory` for `groups/group_membership`, with trusted
  Moderation scope propagation, producer receipt replay, exact membership revision/group scope
  fencing and `SuspendSubject` -> Groups-owned expiry-aware enforcement;''',
)
replace_once(
    groups_readme,
    '''Localization/governance transaction-aware conversion, provider ACL integration, the neutral
moderation adapter/application orchestration, native/GraphQL direct-enforcement transport, and
runtime evidence remain open. `GROUPS-07` remains `in_progress`.''',
    '''Provider ACL integration, broader native/GraphQL parity, and moderation/direct-enforcement
runtime/replay/concurrency evidence remain open. The neutral membership Moderation adapter is now
source-complete, while `GROUPS-07` remains `in_progress` until its declared runtime gates close.''',
)
replace_once(
    groups_readme,
    '''- Moderation owns reports, cases, decisions, retries, appeals, and application orchestration.
  The neutral adapter will call the shared Groups enforcement owner mutation after producer receipt,
  scope, subject revision and effect validation; moderation never writes Groups tables directly.''',
    '''- Moderation owns reports, cases, decisions, retries, appeals, and application orchestration.
  The neutral adapter calls the shared Groups enforcement owner mutation after producer receipt,
  exact trusted scope, subject revision and effect validation; moderation never writes Groups tables
  directly.''',
)
replace_once(
    groups_readme,
    '''invitation/application authorization and the direct enforcement command are source-complete, but
runtime evidence and the remaining owner/adapter paths are open.''',
    '''invitation/application authorization, direct enforcement, and the neutral membership Moderation
adapter are source-complete, but runtime evidence and remaining provider/owner paths are open.''',
)
replace_once(
    groups_readme,
    '''- [Membership enforcement command guard](../../scripts/verify/verify-groups-membership-enforcement-command.mjs)
- [Effective membership access guard]''',
    '''- [Membership enforcement command guard](../../scripts/verify/verify-groups-membership-enforcement-command.mjs)
- [Moderation membership adapter guard](../../scripts/verify/verify-groups-moderation-subject-adapter.mjs)
- [Effective membership access guard]''',
)

replace_once(
    groups_plan,
    '''- direct `GroupMembershipEnforcementCommandPort` suspend/revoke with expected-revision CAS,
  receipt-first replay, hierarchy/owner protection, shared owner mutation, audit/events and bounded
  direct-local provenance;
- direct GraphQL suspend/revoke mutations''',
    '''- direct `GroupMembershipEnforcementCommandPort` suspend/revoke with expected-revision CAS,
  receipt-first replay, hierarchy/owner protection, shared owner mutation, audit/events and bounded
  direct-local provenance;
- neutral `groups/group_membership` Moderation subject adapter with trusted canonical group-scope
  propagation, decision-ID/hash producer receipt binding before subject reads, exact membership
  revision fencing and `SuspendSubject` reuse of the shared expiry-aware Groups owner mutation;
- direct GraphQL suspend/revoke mutations''',
)
replace_once(
    groups_plan,
    '''- neutral moderation adapter and durable moderation application orchestration.''',
    '''- moderation adapter runtime/replay/concurrency evidence and remaining durable producer/provider integration evidence.''',
)
replace_once(
    groups_plan,
    '''| GROUPS-07 | in_progress | revision, enforcement read/direct command/GraphQL, effective core/join/leave/feature/localization/governance access, transactional invitation/application authorization | moderation adapter, provider cutover, runtime/concurrency/parity evidence |''',
    '''| GROUPS-07 | in_progress | revision, enforcement read/direct command/GraphQL, neutral membership Moderation adapter, effective core/join/leave/feature/localization/governance access, transactional invitation/application authorization | provider cutover plus moderation/direct runtime/concurrency/replay/parity evidence |''',
)
replace_once(groups_plan, "### Planned moderation adapter", "### Source-complete moderation adapter")
replace_once(groups_plan, "Initial mapping remains:", "Implemented bounded mapping:")
replace_once(
    groups_plan,
    '''The adapter is the next moderation-specific source slice and requires the neutral
`rustok-moderation-api` dependency plus producer receipt integration. It must reuse the owner mutation
above rather than introduce a second Groups enforcement state path.''',
    '''The adapter now depends only on neutral `rustok-moderation-api` plus shared `rustok-outbox`
producer receipts. Moderation carries the already-validated immutable case scope as a versioned
trusted `PortContext` claim without changing the historical `ApplyModerationDecisionCommand` receipt
shape. Groups binds that exact scope together with the command in its own receipt, then reuses the
owner mutation above rather than introducing a second enforcement state path. Runtime/replay/race
proof remains an explicit GROUPS-07 evidence gate.''',
)
replace_once(
    groups_plan,
    '''node scripts/verify/verify-groups-membership-enforcement-command.mjs
node scripts/verify/verify-groups-membership-enforcement-graphql.mjs''',
    '''node scripts/verify/verify-groups-membership-enforcement-command.mjs
node scripts/verify/verify-groups-moderation-subject-adapter.mjs
node scripts/verify/verify-groups-membership-enforcement-graphql.mjs''',
)

replace_once(
    moderation_plan,
    '''For Groups compatibility:
''',
    '''For Groups compatibility now present in source:
''',
)
replace_once(
    moderation_plan,
    '''- the Groups adapter verifies tenant, scope, subject ID/revision, decision hash, effect
  compatibility, and local invariants inside the owner transaction.''',
    '''- the Groups adapter verifies tenant, canonical trusted group scope, subject ID/revision,
  decision hash, effect compatibility, and local invariants inside the owner transaction;
- `SuspendSubject { effective_until }` is the bounded first mapping and reuses Groups-owned
  expiry/enforcement state; unsupported effects remain non-successful.''',
)
replace_once(
    moderation_plan,
    '''- moderation admin FFA owns queue/case/decision/application surfaces; Groups FFA owns current
  local enforcement state and authorized direct domain actions.

For Forum's bounded adapter source:''',
    '''- moderation admin FFA owns queue/case/decision/application surfaces; Groups FFA owns current
  local enforcement state and authorized direct domain actions;
- Groups registers only the neutral `groups/group_membership` adapter factory and depends on
  `rustok-moderation-api`, never the Moderation persistence owner.

For Forum's bounded adapter source:''',
)
replace_once(
    moderation_plan,
    '''`dispatch_application_operation_once` is source-ready as the bounded dispatcher. It claims
at most one exact due operation, reconstructs `ApplyModerationDecisionCommand` from immutable
decision/effect/case facts, verifies decision hash and exact reviewed subject, looks up only
the exact materialized `(subject_module, subject_kind)` adapter and invokes it with a trusted
service `PortContext`.''',
    '''`dispatch_application_operation_once` is source-ready as the bounded dispatcher. It claims
at most one exact due operation, reconstructs `ApplyModerationDecisionCommand` from immutable
decision/effect/case facts, verifies decision hash and exact reviewed subject, canonicalizes the
immutable case scope into a versioned trusted `PortContext` claim, looks up only the exact materialized
`(subject_module, subject_kind)` adapter and invokes it with a trusted service context. Keeping scope
outside the serialized command preserves historical domain receipt request digests; scope-aware
adapters bind the canonical claim alongside the command in their own receipt.''',
)
replace_once(
    moderation_plan,
    '''2. Integrate Groups as the membership-scoped expiry reference adapter, then continue the accepted producer sequence (Blog, Comments, Pages, Reviews, Marketplace, Media, Messaging and Profiles) through `rustok-moderation-api` without cross-owner persistence reads.''',
    '''2. Retain runtime/replay/concurrency evidence for the Groups membership-scoped expiry reference adapter, then continue the accepted producer sequence with Blog, Comments, Pages, Reviews, Marketplace, Media, Messaging and Profiles through `rustok-moderation-api` without cross-owner persistence reads.''',
)

require_absent(verifier_path)
Path(verifier_path).write_text(r'''import fs from "node:fs";

const read = (path) => fs.readFileSync(path, "utf8");
const requireText = (source, needle, message) => {
  if (!source.includes(needle)) throw new Error(message);
};
const forbidText = (source, needle, message) => {
  if (source.includes(needle)) throw new Error(message);
};

const cargo = read("crates/rustok-groups/Cargo.toml");
const lib = read("crates/rustok-groups/src/lib.rs");
const adapter = read("crates/rustok-groups/src/moderation_subject.rs");
const ownerMutation = read("crates/rustok-groups/src/membership_enforcement_command.rs");
const ownerLock = read("crates/rustok-groups/src/membership_enforcement_transaction.rs");
const neutralModel = read("crates/rustok-moderation-api/src/model.rs");
const dispatcher = read("crates/rustok-moderation/src/application_dispatch.rs");
const plan = read("crates/rustok-groups/docs/implementation-plan.md");
const moderationPlan = read("crates/rustok-moderation/docs/implementation-plan.md");
const contract = JSON.parse(
  read("crates/rustok-groups/contracts/groups-effective-membership-access.json"),
);

for (const marker of [
  'rustok-moderation-api = { path = "../rustok-moderation-api" }',
  "rustok-outbox.workspace = true",
]) {
  requireText(cargo, marker, `Groups moderation adapter dependency missing: ${marker}`);
}
forbidText(
  cargo,
  "rustok-moderation =",
  "Groups must not depend on the Moderation persistence owner",
);

for (const marker of [
  "mod moderation_subject;",
  "pub use moderation_subject::GroupsModerationSubjectAdapterFactory;",
  "register_moderation_subject_adapter_factory",
  "moderation_subject::GroupsModerationSubjectAdapterFactory",
]) {
  requireText(lib, marker, `Groups module registration missing ${marker}`);
}

for (const marker of [
  'pub const GROUPS_MODERATION_MODULE: &str = "groups"',
  "ModerationSubjectKind::GroupMembership",
  "ModerationSubjectCommandPort for GroupsModerationSubjectAdapter",
  "context.require_policy(PortCallPolicy::write())?",
  'const MODERATION_DISPATCH_ACTOR: &str = "rustok-moderation"',
  "moderation_scope_from_claims(&context.claims)",
  "GroupsModerationReceiptRequest",
  "scope: &scope",
  "command: &command",
  "idempotency::admit",
  "idempotency::OwnerOperationScope::Tenant(tenant_id)",
  "idempotency::complete(&transaction, lease, &application)",
  "lock_membership_enforcement_target_by_id_for_update",
  "target.group.id != group_id",
  "target.membership.revision != command.subject.revision",
  "ModerationDecisionEffectAction::SuspendSubject",
  "GroupMembershipEnforcementSourceKind::ModerationDecision",
  "moderation_decision_id: Some(command.decision_id)",
  "moderation_decision_hash: Some(command.decision_hash.clone())",
  "apply_membership_suspension_in_tx",
  "result.membership_revision <= command.subject.revision",
  '"groups.moderation_effect_unsupported"',
  '"groups.moderation_scope_mismatch"',
]) {
  requireText(adapter, marker, `Groups moderation adapter is missing ${marker}`);
}

const admitIndex = adapter.indexOf("idempotency::admit");
const lockIndex = adapter.indexOf("lock_membership_enforcement_target_by_id_for_update");
if (admitIndex < 0 || lockIndex < 0 || admitIndex >= lockIndex) {
  throw new Error("Groups moderation producer receipt admission must precede membership subject reads");
}
for (const forbidden of [
  "rustok_moderation::",
  "moderation_cases",
  "moderation_decisions",
  "moderation_reports",
]) {
  forbidText(adapter, forbidden, `Groups adapter crosses Moderation owner persistence: ${forbidden}`);
}

for (const marker of [
  "pub(crate) async fn apply_membership_suspension_in_tx",
  "validate_mutation_identity",
  "validate_provenance",
  "moderation-driven membership enforcement requires decision identity",
]) {
  requireText(ownerMutation, marker, `Groups owner mutation seam missing ${marker}`);
}
requireText(
  ownerLock,
  "lock_membership_enforcement_target_by_id_for_update",
  "Groups adapter must retain the receipt-first membership-ID owner lock primitive",
);

for (const marker of [
  'pub const MODERATION_SCOPE_CLAIM_PREFIX: &str = "moderation.scope.v1:"',
  "pub fn moderation_scope_claim",
  "pub fn moderation_scope_from_claims",
  "DuplicateClaim",
  "InvalidClaim",
]) {
  requireText(neutralModel, marker, `Neutral scope claim contract missing ${marker}`);
}
const commandStart = neutralModel.indexOf("pub struct ApplyModerationDecisionCommand");
const commandEnd = neutralModel.indexOf("pub struct ModerationDecisionApplication", commandStart);
if (commandStart < 0 || commandEnd < 0) throw new Error("neutral command boundary missing");
const commandBlock = neutralModel.slice(commandStart, commandEnd);
forbidText(
  commandBlock,
  "pub scope:",
  "Historical ApplyModerationDecisionCommand receipt shape must not be extended with scope",
);

for (const marker of [
  "moderation_scope_claim(&case.scope)",
  ".with_claim(scope_claim)",
  "application_port_context(tenant_id, decision_id, lease_token, scope_claim)",
]) {
  requireText(dispatcher, marker, `Moderation dispatcher scope propagation missing ${marker}`);
}

if (contract.remaining_paths?.includes("moderation_subject_adapter")) {
  throw new Error("Groups contract still lists the source-complete moderation adapter as remaining");
}
if (!contract.converted_source_paths?.moderation_subject_adapter?.includes(
  "crates/rustok-groups/src/moderation_subject.rs",
)) {
  throw new Error("Groups contract does not retain the moderation adapter source path");
}
if (
  contract.evidence?.moderation_subject_adapter_static_boundary !==
  "scripts/verify/verify-groups-moderation-subject-adapter.mjs"
) {
  throw new Error("Groups contract is missing the adapter static boundary");
}
if (contract.evidence?.moderation_subject_adapter_runtime !== null) {
  throw new Error("Groups contract must keep runtime adapter evidence explicitly open");
}
for (const marker of [
  "### Source-complete moderation adapter",
  "Runtime/replay/race",
  "GROUPS-07 | in_progress",
  "verify-groups-moderation-subject-adapter.mjs",
]) {
  requireText(plan, marker, `Groups canonical plan is missing ${marker}`);
}
requireText(
  moderationPlan,
  "For Groups compatibility now present in source:",
  "Moderation plan must retain the Groups reference-adapter handoff",
);
requireText(
  moderationPlan,
  "historical domain receipt request digests",
  "Moderation plan must document scope propagation compatibility",
);

console.log("Groups moderation membership adapter source guard passed");
''')
