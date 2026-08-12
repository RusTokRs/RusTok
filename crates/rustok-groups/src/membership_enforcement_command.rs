use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QuerySelect, Set, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::domain::{
    GroupMembershipEffectiveStatus, GroupMembershipEnforcementSourceKind, GroupMembershipStatus,
    GroupRole,
};
use crate::entities::group;
use crate::error::{GroupsError, GroupsResult};
use crate::governance_entities::{audit_entry, command_receipt};
use crate::group_event_entities;
use crate::membership_enforcement::resolve_group_membership_enforcement;
use crate::membership_enforcement_entities::{membership_enforcement, membership_state};
use crate::ports::GroupMembershipEnforcementCommandPort;

const SUSPEND_COMMAND: &str = "groups.membership.suspend.v1";
const REVOKE_SUSPENSION_COMMAND: &str = "groups.membership.suspension_revoke.v1";
const SUSPENDED_EVENT: &str = "groups.membership.suspended";
const REVOKED_EVENT: &str = "groups.membership.suspension_revoked";
const MAX_REASON_CODE_BYTES: usize = 80;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuspendGroupMembershipRequest {
    pub group_id: Uuid,
    pub target_user_id: Uuid,
    pub expected_membership_revision: i64,
    pub reason_code: String,
    pub effective_until: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevokeGroupMembershipSuspensionRequest {
    pub group_id: Uuid,
    pub target_user_id: Uuid,
    pub expected_membership_revision: i64,
    pub reason_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMembershipEnforcementMutationResult {
    pub group_id: Uuid,
    pub membership_id: Uuid,
    pub user_id: Uuid,
    pub membership_revision: i64,
    pub group_version: i64,
    /// Stored-lifecycle active count. Temporary enforcement deliberately does not change it.
    pub member_count: i64,
    pub effective_status: GroupMembershipEffectiveStatus,
    pub enforcement_revision: i64,
    pub effective_until: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub replayed: bool,
}

/// Provenance passed to the shared Groups-owned enforcement mutation.
///
/// Direct commands use `DirectLocal`. The later neutral Moderation adapter can reuse these owner
/// mutations with `ModerationDecision` provenance after its own receipt, subject/scope and revision
/// validation. Moderation cases, queue state and policy snapshots never belong here.
#[derive(Clone, Debug)]
pub(crate) struct MembershipEnforcementProvenance {
    pub(crate) source_kind: GroupMembershipEnforcementSourceKind,
    pub(crate) moderation_decision_id: Option<Uuid>,
    pub(crate) moderation_decision_hash: Option<String>,
    pub(crate) actor_kind: String,
    pub(crate) actor_id: String,
    pub(crate) audit_actor_user_id: Option<Uuid>,
}

impl MembershipEnforcementProvenance {
    fn direct_local(actor_user_id: Uuid) -> Self {
        Self {
            source_kind: GroupMembershipEnforcementSourceKind::DirectLocal,
            moderation_decision_id: None,
            moderation_decision_hash: None,
            actor_kind: "user".to_string(),
            actor_id: actor_user_id.to_string(),
            audit_actor_user_id: Some(actor_user_id),
        }
    }
}

#[derive(Clone)]
pub struct GroupMembershipEnforcementCommandService {
    db: DatabaseConnection,
}

impl GroupMembershipEnforcementCommandService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn suspend_owned(
        &self,
        context: &PortContext,
        request: SuspendGroupMembershipRequest,
    ) -> GroupsResult<GroupMembershipEnforcementMutationResult> {
        validate_identity(
            request.group_id,
            request.target_user_id,
            request.expected_membership_revision,
        )?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        if actor_user_id == request.target_user_id {
            return Err(GroupsError::MembershipEnforcementSelfTarget);
        }
        let idempotency_key = idempotency_key(context)?;
        let reason_code = normalize_reason_code(&request.reason_code)?;
        let request = SuspendGroupMembershipRequest {
            reason_code,
            ..request
        };
        let request_hash = request_hash(&request)?;

        let transaction = self.db.begin().await?;
        let locked_group = lock_group(&transaction, tenant_id, request.group_id).await?;
        if let Some(replayed) = replay_receipt::<GroupMembershipEnforcementMutationResult>(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            &idempotency_key,
            SUSPEND_COMMAND,
            &request_hash,
        )
        .await?
        {
            transaction.commit().await?;
            return Ok(GroupMembershipEnforcementMutationResult {
                replayed: true,
                ..replayed
            });
        }

        let now = Utc::now();
        if request.effective_until.is_some_and(|until| until <= now) {
            return Err(GroupsError::Validation(
                "membership suspension expiry must be in the future".to_string(),
            ));
        }

        let platform_moderate = has_platform_moderate(context);
        let locked = lock_command_memberships(
            &transaction,
            tenant_id,
            locked_group.id,
            actor_user_id,
            request.target_user_id,
            !platform_moderate,
        )
        .await?;
        authorize_direct_enforcement(
            &transaction,
            tenant_id,
            &locked_group,
            &locked,
            actor_user_id,
            request.target_user_id,
            platform_moderate,
            now,
        )
        .await?;

        let target = locked.membership(request.target_user_id).ok_or_else(|| {
            GroupsError::Conflict("target group membership is required".to_string())
        })?;
        let result = apply_membership_suspension_in_tx(
            &transaction,
            context,
            locked_group,
            target.clone(),
            locked.enforcement(target.id),
            request.expected_membership_revision,
            request.reason_code.clone(),
            request.effective_until,
            MembershipEnforcementProvenance::direct_local(actor_user_id),
            now,
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            idempotency_key,
            SUSPEND_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn revoke_owned(
        &self,
        context: &PortContext,
        request: RevokeGroupMembershipSuspensionRequest,
    ) -> GroupsResult<GroupMembershipEnforcementMutationResult> {
        validate_identity(
            request.group_id,
            request.target_user_id,
            request.expected_membership_revision,
        )?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        if actor_user_id == request.target_user_id {
            return Err(GroupsError::MembershipEnforcementSelfTarget);
        }
        let idempotency_key = idempotency_key(context)?;
        let reason_code = normalize_reason_code(&request.reason_code)?;
        let request = RevokeGroupMembershipSuspensionRequest {
            reason_code,
            ..request
        };
        let request_hash = request_hash(&request)?;

        let transaction = self.db.begin().await?;
        let locked_group = lock_group(&transaction, tenant_id, request.group_id).await?;
        if let Some(replayed) = replay_receipt::<GroupMembershipEnforcementMutationResult>(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            &idempotency_key,
            REVOKE_SUSPENSION_COMMAND,
            &request_hash,
        )
        .await?
        {
            transaction.commit().await?;
            return Ok(GroupMembershipEnforcementMutationResult {
                replayed: true,
                ..replayed
            });
        }

        let now = Utc::now();
        let platform_moderate = has_platform_moderate(context);
        let locked = lock_command_memberships(
            &transaction,
            tenant_id,
            locked_group.id,
            actor_user_id,
            request.target_user_id,
            !platform_moderate,
        )
        .await?;
        authorize_direct_enforcement(
            &transaction,
            tenant_id,
            &locked_group,
            &locked,
            actor_user_id,
            request.target_user_id,
            platform_moderate,
            now,
        )
        .await?;

        let target = locked.membership(request.target_user_id).ok_or_else(|| {
            GroupsError::Conflict("target group membership is required".to_string())
        })?;
        let result = revoke_membership_suspension_in_tx(
            &transaction,
            context,
            locked_group,
            target.clone(),
            locked.enforcement(target.id),
            request.expected_membership_revision,
            request.reason_code.clone(),
            MembershipEnforcementProvenance::direct_local(actor_user_id),
            now,
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            idempotency_key,
            REVOKE_SUSPENSION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}

#[async_trait]
impl GroupMembershipEnforcementCommandPort for GroupMembershipEnforcementCommandService {
    async fn suspend_membership(
        &self,
        context: PortContext,
        request: SuspendGroupMembershipRequest,
    ) -> Result<GroupMembershipEnforcementMutationResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.suspend_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn revoke_membership_suspension(
        &self,
        context: PortContext,
        request: RevokeGroupMembershipSuspensionRequest,
    ) -> Result<GroupMembershipEnforcementMutationResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.revoke_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

struct LockedCommandState {
    memberships: Vec<membership_state::Model>,
    enforcements: Vec<membership_enforcement::Model>,
}

impl LockedCommandState {
    fn membership(&self, user_id: Uuid) -> Option<membership_state::Model> {
        self.memberships
            .iter()
            .find(|row| row.user_id == user_id)
            .cloned()
    }

    fn enforcement(&self, membership_id: Uuid) -> Option<membership_enforcement::Model> {
        self.enforcements
            .iter()
            .find(|row| row.membership_id == membership_id)
            .cloned()
    }
}

async fn lock_group(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
) -> GroupsResult<group::Model> {
    match transaction.get_database_backend() {
        DatabaseBackend::Sqlite => {
            transaction
                .execute(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "UPDATE groups SET version = version WHERE tenant_id = ? AND id = ?",
                    vec![tenant_id.into(), group_id.into()],
                ))
                .await?;
            group::Entity::find()
                .filter(group::Column::TenantId.eq(tenant_id))
                .filter(group::Column::Id.eq(group_id))
                .one(transaction)
                .await?
                .ok_or(GroupsError::NotFound)
        }
        DatabaseBackend::Postgres | DatabaseBackend::MySql => group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Id.eq(group_id))
            .lock_exclusive()
            .one(transaction)
            .await?
            .ok_or(GroupsError::NotFound),
    }
}

async fn lock_command_memberships(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
    target_user_id: Uuid,
    require_actor_membership: bool,
) -> GroupsResult<LockedCommandState> {
    let mut user_ids = vec![target_user_id];
    if require_actor_membership {
        user_ids.push(actor_user_id);
    }
    user_ids.sort_unstable();
    user_ids.dedup();

    let mut memberships = Vec::with_capacity(user_ids.len());
    for user_id in user_ids {
        let mut query = membership_state::Entity::find()
            .filter(membership_state::Column::TenantId.eq(tenant_id))
            .filter(membership_state::Column::GroupId.eq(group_id))
            .filter(membership_state::Column::UserId.eq(user_id));
        if transaction.get_database_backend() != DatabaseBackend::Sqlite {
            query = query.lock_exclusive();
        }
        if let Some(row) = query.one(transaction).await? {
            memberships.push(row);
        }
    }

    let mut membership_ids = memberships.iter().map(|row| row.id).collect::<Vec<_>>();
    membership_ids.sort_unstable();
    let mut enforcements = Vec::with_capacity(membership_ids.len());
    for membership_id in membership_ids {
        let mut query = membership_enforcement::Entity::find_by_id(membership_id)
            .filter(membership_enforcement::Column::TenantId.eq(tenant_id));
        if transaction.get_database_backend() != DatabaseBackend::Sqlite {
            query = query.lock_exclusive();
        }
        if let Some(row) = query.one(transaction).await? {
            enforcements.push(row);
        }
    }

    Ok(LockedCommandState {
        memberships,
        enforcements,
    })
}

async fn authorize_direct_enforcement(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_model: &group::Model,
    locked: &LockedCommandState,
    actor_user_id: Uuid,
    target_user_id: Uuid,
    platform_moderate: bool,
    now: DateTime<Utc>,
) -> GroupsResult<()> {
    let target = locked
        .membership(target_user_id)
        .ok_or_else(|| GroupsError::Conflict("target group membership is required".to_string()))?;
    let target_role = GroupRole::from_str(&target.role).map_err(GroupsError::Invariant)?;
    let target_is_owner = target.user_id == group_model.owner_user_id;
    if target_is_owner != (target_role == GroupRole::Owner) {
        return Err(GroupsError::Invariant(
            "group owner reference and owner membership role disagree".to_string(),
        ));
    }
    if target_is_owner {
        return Err(GroupsError::MembershipEnforcementOwnerProtected);
    }

    let target_status =
        GroupMembershipStatus::from_str(&target.status).map_err(GroupsError::Invariant)?;
    if target_status == GroupMembershipStatus::Banned {
        return Err(GroupsError::MembershipBanned);
    }
    if platform_moderate {
        return Ok(());
    }

    let actor = locked.membership(actor_user_id).ok_or_else(|| {
        GroupsError::ManagerRequired("active local moderator authority is required".to_string())
    })?;
    let actor_state = resolve_group_membership_enforcement(
        transaction,
        tenant_id,
        group_model.id,
        actor_user_id,
        now,
    )
    .await?;
    match actor_state.effective_status {
        GroupMembershipEffectiveStatus::Suspended => return Err(GroupsError::MembershipSuspended),
        GroupMembershipEffectiveStatus::LegacyBanned => return Err(GroupsError::MembershipBanned),
        GroupMembershipEffectiveStatus::Active => {}
        _ => {
            return Err(GroupsError::ManagerRequired(
                "active local moderator authority is required".to_string(),
            ));
        }
    }

    let actor_role = GroupRole::from_str(&actor.role).map_err(GroupsError::Invariant)?;
    let allowed = match actor_role {
        GroupRole::Owner => true,
        GroupRole::Admin => matches!(target_role, GroupRole::Moderator | GroupRole::Member),
        GroupRole::Moderator => target_role == GroupRole::Member,
        GroupRole::Member => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(GroupsError::ManagerRequired(
            "local moderation hierarchy does not permit this membership enforcement mutation"
                .to_string(),
        ))
    }
}

/// Shared Groups-owned suspension mutation used by the direct command and later by the neutral
/// Moderation adapter. The caller must hold the group, target membership and enforcement locks.
pub(crate) async fn apply_membership_suspension_in_tx(
    transaction: &DatabaseTransaction,
    context: &PortContext,
    group_model: group::Model,
    target: membership_state::Model,
    current_enforcement: Option<membership_enforcement::Model>,
    expected_membership_revision: i64,
    reason_code: String,
    effective_until: Option<DateTime<Utc>>,
    provenance: MembershipEnforcementProvenance,
    now: DateTime<Utc>,
) -> GroupsResult<GroupMembershipEnforcementMutationResult> {
    validate_mutation_identity(&group_model, &target, expected_membership_revision)?;
    validate_provenance(&provenance)?;
    let target_status =
        GroupMembershipStatus::from_str(&target.status).map_err(GroupsError::Invariant)?;
    if target_status == GroupMembershipStatus::Banned {
        return Err(GroupsError::MembershipBanned);
    }
    if effective_until.is_some_and(|until| until <= now) {
        return Err(GroupsError::Validation(
            "membership suspension expiry must be in the future".to_string(),
        ));
    }

    if current_enforcement
        .as_ref()
        .is_some_and(|row| enforcement_is_effective(row, now))
    {
        return Err(GroupsError::MembershipEnforcementAlreadySuspended);
    }

    let fixed_now = now.fixed_offset();
    let fixed_until = effective_until.map(|value| value.fixed_offset());
    if let Some(existing) = current_enforcement {
        let mut active: membership_enforcement::ActiveModel = existing.into();
        active.state = Set("suspended".to_string());
        active.reason_code = Set(reason_code.clone());
        active.source_kind = Set(provenance.source_kind.as_str().to_string());
        active.effective_from = Set(fixed_now);
        active.effective_until = Set(fixed_until);
        active.restore_status = Set(target_status.as_str().to_string());
        active.moderation_decision_id = Set(provenance.moderation_decision_id);
        active.moderation_decision_hash = Set(provenance.moderation_decision_hash.clone());
        active.actor_kind = Set(provenance.actor_kind.clone());
        active.actor_id = Set(provenance.actor_id.clone());
        active.revoked_at = Set(None);
        active.updated_at = Set(fixed_now);
        active.update(transaction).await?;
    } else {
        membership_enforcement::ActiveModel {
            membership_id: Set(target.id),
            tenant_id: Set(target.tenant_id),
            group_id: Set(target.group_id),
            user_id: Set(target.user_id),
            state: Set("suspended".to_string()),
            reason_code: Set(reason_code.clone()),
            source_kind: Set(provenance.source_kind.as_str().to_string()),
            effective_from: Set(fixed_now),
            effective_until: Set(fixed_until),
            restore_status: Set(target_status.as_str().to_string()),
            moderation_decision_id: Set(provenance.moderation_decision_id),
            moderation_decision_hash: Set(provenance.moderation_decision_hash.clone()),
            actor_kind: Set(provenance.actor_kind.clone()),
            actor_id: Set(provenance.actor_id.clone()),
            revision: Set(1),
            revoked_at: Set(None),
            created_at: Set(fixed_now),
            updated_at: Set(fixed_now),
        }
        .insert(transaction)
        .await?;
    }

    let group_after =
        bump_group_version_without_member_count_change(transaction, group_model, fixed_now).await?;
    let state = resolve_group_membership_enforcement(
        transaction,
        target.tenant_id,
        target.group_id,
        target.user_id,
        now,
    )
    .await?;
    let result = mutation_result(&group_after, &state, false)?;

    append_audit(
        transaction,
        context,
        target.tenant_id,
        target.group_id,
        provenance.audit_actor_user_id,
        "group.membership_suspended",
        Some(target.user_id),
        json!({
            "membership_id": target.id,
            "reason_code": reason_code,
            "source_kind": provenance.source_kind.as_str(),
            "moderation_decision_id": provenance.moderation_decision_id,
            "moderation_decision_hash": provenance.moderation_decision_hash.clone(),
            "mutation_actor_kind": provenance.actor_kind,
            "mutation_actor_id": provenance.actor_id,
            "effective_until": effective_until,
            "previous_membership_revision": expected_membership_revision,
            "membership_revision": result.membership_revision,
            "group_version": result.group_version,
            "member_count": result.member_count,
            "member_count_semantics": "stored_lifecycle_active"
        }),
    )
    .await?;
    append_domain_event(
        transaction,
        target.tenant_id,
        target.id,
        SUSPENDED_EVENT,
        provenance.audit_actor_user_id,
        json!({
            "group_id": target.group_id,
            "membership_id": target.id,
            "user_id": target.user_id,
            "reason_code": reason_code,
            "source_kind": provenance.source_kind.as_str(),
            "moderation_decision_id": provenance.moderation_decision_id,
            "moderation_decision_hash": provenance.moderation_decision_hash,
            "mutation_actor_kind": provenance.actor_kind,
            "mutation_actor_id": provenance.actor_id,
            "effective_until": effective_until,
            "membership_revision": result.membership_revision,
            "group_version": result.group_version
        }),
        fixed_now,
    )
    .await?;
    Ok(result)
}

/// Shared Groups-owned revocation mutation. A direct-local caller may only revoke direct-local
/// enforcement, so local moderation cannot erase moderation-decision provenance.
pub(crate) async fn revoke_membership_suspension_in_tx(
    transaction: &DatabaseTransaction,
    context: &PortContext,
    group_model: group::Model,
    target: membership_state::Model,
    current_enforcement: Option<membership_enforcement::Model>,
    expected_membership_revision: i64,
    revocation_reason_code: String,
    provenance: MembershipEnforcementProvenance,
    now: DateTime<Utc>,
) -> GroupsResult<GroupMembershipEnforcementMutationResult> {
    validate_mutation_identity(&group_model, &target, expected_membership_revision)?;
    validate_provenance(&provenance)?;
    let existing = current_enforcement.ok_or(GroupsError::MembershipEnforcementNotActive)?;
    let existing_source = GroupMembershipEnforcementSourceKind::from_str(&existing.source_kind)
        .map_err(GroupsError::Invariant)?;
    if provenance.source_kind == GroupMembershipEnforcementSourceKind::DirectLocal
        && existing_source != GroupMembershipEnforcementSourceKind::DirectLocal
    {
        return Err(GroupsError::MembershipEnforcementSourceConflict);
    }
    if !enforcement_is_effective(&existing, now) {
        return Err(GroupsError::MembershipEnforcementNotActive);
    }

    let previous_reason_code = existing.reason_code.clone();
    let previous_effective_until = existing
        .effective_until
        .map(|value| value.with_timezone(&Utc));
    let previous_moderation_decision_id = existing.moderation_decision_id;
    let previous_moderation_decision_hash = existing.moderation_decision_hash.clone();
    let fixed_now = now.fixed_offset();
    let mut active: membership_enforcement::ActiveModel = existing.into();
    // Preserve the original suspension actor/source provenance. The revoking actor is retained in
    // immutable audit/event facts instead of overwriting who established the enforcement row.
    active.revoked_at = Set(Some(fixed_now));
    active.updated_at = Set(fixed_now);
    active.update(transaction).await?;

    let group_after =
        bump_group_version_without_member_count_change(transaction, group_model, fixed_now).await?;
    let state = resolve_group_membership_enforcement(
        transaction,
        target.tenant_id,
        target.group_id,
        target.user_id,
        now,
    )
    .await?;
    let result = mutation_result(&group_after, &state, false)?;

    append_audit(
        transaction,
        context,
        target.tenant_id,
        target.group_id,
        provenance.audit_actor_user_id,
        "group.membership_suspension_revoked",
        Some(target.user_id),
        json!({
            "membership_id": target.id,
            "revocation_reason_code": revocation_reason_code,
            "previous_reason_code": previous_reason_code,
            "previous_effective_until": previous_effective_until,
            "source_kind": existing_source.as_str(),
            "previous_moderation_decision_id": previous_moderation_decision_id,
            "previous_moderation_decision_hash": previous_moderation_decision_hash.clone(),
            "mutation_actor_kind": provenance.actor_kind,
            "mutation_actor_id": provenance.actor_id,
            "previous_membership_revision": expected_membership_revision,
            "membership_revision": result.membership_revision,
            "group_version": result.group_version,
            "member_count": result.member_count,
            "member_count_semantics": "stored_lifecycle_active"
        }),
    )
    .await?;
    append_domain_event(
        transaction,
        target.tenant_id,
        target.id,
        REVOKED_EVENT,
        provenance.audit_actor_user_id,
        json!({
            "group_id": target.group_id,
            "membership_id": target.id,
            "user_id": target.user_id,
            "revocation_reason_code": revocation_reason_code,
            "previous_reason_code": previous_reason_code,
            "source_kind": existing_source.as_str(),
            "previous_moderation_decision_id": previous_moderation_decision_id,
            "previous_moderation_decision_hash": previous_moderation_decision_hash,
            "mutation_actor_kind": provenance.actor_kind,
            "mutation_actor_id": provenance.actor_id,
            "membership_revision": result.membership_revision,
            "group_version": result.group_version
        }),
        fixed_now,
    )
    .await?;
    Ok(result)
}

fn enforcement_is_effective(row: &membership_enforcement::Model, now: DateTime<Utc>) -> bool {
    row.revoked_at.is_none()
        && row.effective_from.with_timezone(&Utc) <= now
        && row
            .effective_until
            .as_ref()
            .is_none_or(|until| now < until.with_timezone(&Utc))
}

fn validate_mutation_identity(
    group_model: &group::Model,
    target: &membership_state::Model,
    expected_membership_revision: i64,
) -> GroupsResult<()> {
    if target.tenant_id != group_model.tenant_id || target.group_id != group_model.id {
        return Err(GroupsError::Invariant(
            "membership enforcement target does not belong to the locked group".to_string(),
        ));
    }
    let target_role = GroupRole::from_str(&target.role).map_err(GroupsError::Invariant)?;
    if target.user_id == group_model.owner_user_id || target_role == GroupRole::Owner {
        return Err(GroupsError::MembershipEnforcementOwnerProtected);
    }
    if target.revision != expected_membership_revision {
        return Err(GroupsError::MembershipEnforcementRevisionConflict);
    }
    Ok(())
}

fn validate_provenance(provenance: &MembershipEnforcementProvenance) -> GroupsResult<()> {
    if !matches!(
        provenance.actor_kind.as_str(),
        "user" | "service" | "system"
    ) || provenance.actor_id.trim().is_empty()
    {
        return Err(GroupsError::Invariant(
            "membership enforcement provenance actor is invalid".to_string(),
        ));
    }
    match provenance.source_kind {
        GroupMembershipEnforcementSourceKind::DirectLocal => {
            if provenance.moderation_decision_id.is_some()
                || provenance.moderation_decision_hash.is_some()
            {
                return Err(GroupsError::Invariant(
                    "direct local membership enforcement cannot carry moderation decision identity"
                        .to_string(),
                ));
            }
        }
        GroupMembershipEnforcementSourceKind::ModerationDecision => {
            let Some(decision_id) = provenance.moderation_decision_id else {
                return Err(GroupsError::Invariant(
                    "moderation-driven membership enforcement requires decision identity"
                        .to_string(),
                ));
            };
            let Some(hash) = provenance.moderation_decision_hash.as_deref() else {
                return Err(GroupsError::Invariant(
                    "moderation-driven membership enforcement requires decision hash".to_string(),
                ));
            };
            if decision_id.is_nil()
                || hash.len() != 64
                || !hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err(GroupsError::Invariant(
                    "moderation-driven membership enforcement decision identity is invalid"
                        .to_string(),
                ));
            }
        }
    }
    Ok(())
}

async fn bump_group_version_without_member_count_change(
    transaction: &DatabaseTransaction,
    group_model: group::Model,
    now: chrono::DateTime<chrono::FixedOffset>,
) -> GroupsResult<group::Model> {
    if group_model.member_count < 0 || group_model.version < 1 {
        return Err(GroupsError::Invariant(
            "group member count and version must be valid before enforcement mutation".to_string(),
        ));
    }
    let next_version = group_model.version.checked_add(1).ok_or_else(|| {
        GroupsError::Invariant("group version overflow during enforcement mutation".to_string())
    })?;
    let mut active: group::ActiveModel = group_model.into();
    active.version = Set(next_version);
    active.updated_at = Set(now);
    Ok(active.update(transaction).await?)
}

fn mutation_result(
    group_model: &group::Model,
    state: &crate::dto::GroupMembershipEffectiveState,
    replayed: bool,
) -> GroupsResult<GroupMembershipEnforcementMutationResult> {
    let membership_id = state.membership_id.ok_or_else(|| {
        GroupsError::Invariant("membership disappeared during enforcement mutation".to_string())
    })?;
    let membership_revision = state.membership_revision.ok_or_else(|| {
        GroupsError::Invariant(
            "membership revision disappeared during enforcement mutation".to_string(),
        )
    })?;
    if membership_revision < 1 || group_model.version < 1 || group_model.member_count < 0 {
        return Err(GroupsError::Invariant(
            "invalid owner revisions after membership enforcement mutation".to_string(),
        ));
    }
    let enforcement = state.enforcement.as_ref().ok_or_else(|| {
        GroupsError::Invariant(
            "enforcement row disappeared during enforcement mutation".to_string(),
        )
    })?;
    Ok(GroupMembershipEnforcementMutationResult {
        group_id: state.group_id,
        membership_id,
        user_id: state.user_id,
        membership_revision,
        group_version: group_model.version,
        member_count: group_model.member_count,
        effective_status: state.effective_status,
        enforcement_revision: enforcement.revision,
        effective_until: enforcement.effective_until,
        revoked_at: enforcement.revoked_at,
        replayed,
    })
}

async fn replay_receipt<T: DeserializeOwned>(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
    idempotency_key: &str,
    command_type: &str,
    request_hash: &str,
) -> GroupsResult<Option<T>> {
    let Some(receipt) = command_receipt::Entity::find()
        .filter(command_receipt::Column::TenantId.eq(tenant_id))
        .filter(command_receipt::Column::IdempotencyKey.eq(idempotency_key))
        .one(transaction)
        .await?
    else {
        return Ok(None);
    };
    if receipt.group_id != group_id
        || receipt.actor_user_id != actor_user_id
        || receipt.command_type != command_type
        || receipt.request_hash != request_hash
    {
        return Err(GroupsError::Conflict(
            "idempotency key was already used for another group enforcement command".to_string(),
        ));
    }
    serde_json::from_value(receipt.response)
        .map(Some)
        .map_err(|error| GroupsError::Invariant(format!("invalid group command receipt: {error}")))
}

async fn store_receipt<T: Serialize>(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
    idempotency_key: String,
    command_type: &str,
    request_hash: String,
    response: &T,
) -> GroupsResult<()> {
    command_receipt::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
        actor_user_id: Set(actor_user_id),
        idempotency_key: Set(idempotency_key),
        command_type: Set(command_type.to_string()),
        request_hash: Set(request_hash),
        response: Set(serde_json::to_value(response).map_err(|error| {
            GroupsError::Invariant(format!(
                "group enforcement command response is not serializable: {error}"
            ))
        })?),
        created_at: Set(Utc::now().fixed_offset()),
    }
    .insert(transaction)
    .await?;
    Ok(())
}

async fn append_audit(
    transaction: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Option<Uuid>,
    action: &str,
    target_user_id: Option<Uuid>,
    details: Value,
) -> GroupsResult<()> {
    audit_entry::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
        actor_user_id: Set(actor_user_id),
        action: Set(action.to_string()),
        target_user_id: Set(target_user_id),
        details: Set(details),
        correlation_id: Set(context.correlation_id.clone()),
        created_at: Set(Utc::now().fixed_offset()),
    }
    .insert(transaction)
    .await?;
    Ok(())
}

async fn append_domain_event(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    membership_id: Uuid,
    event_type: &str,
    actor_id: Option<Uuid>,
    payload: Value,
    created_at: chrono::DateTime<chrono::FixedOffset>,
) -> GroupsResult<()> {
    group_event_entities::ActiveModel {
        sequence_no: NotSet,
        event_id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        aggregate_type: Set("membership".to_string()),
        aggregate_id: Set(membership_id),
        event_type: Set(event_type.to_string()),
        schema_version: Set(1),
        actor_id: Set(actor_id),
        payload: Set(payload),
        created_at: Set(created_at),
    }
    .insert(transaction)
    .await?;
    Ok(())
}

fn validate_identity(
    group_id: Uuid,
    target_user_id: Uuid,
    expected_revision: i64,
) -> GroupsResult<()> {
    if group_id.is_nil() || target_user_id.is_nil() || expected_revision < 1 {
        return Err(GroupsError::Validation(
            "group, target membership identity and positive expected revision are required"
                .to_string(),
        ));
    }
    Ok(())
}

fn normalize_reason_code(value: &str) -> GroupsResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_REASON_CODE_BYTES {
        return Err(GroupsError::Validation(
            "membership enforcement reason code must contain 1..80 bytes".to_string(),
        ));
    }
    if !value.bytes().all(|byte| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(byte, b'.' | b':' | b'_' | b'-')
    }) {
        return Err(GroupsError::Validation(
            "membership enforcement reason code must be canonical lowercase ASCII".to_string(),
        ));
    }
    Ok(value.to_string())
}

fn request_hash<T: Serialize>(request: &T) -> GroupsResult<String> {
    let bytes = serde_json::to_vec(request).map_err(|error| {
        GroupsError::Validation(format!(
            "group enforcement command request is not serializable: {error}"
        ))
    })?;
    Ok(crate::domain::sha256_hex(&bytes))
}

fn context_tenant_id(context: &PortContext) -> GroupsResult<Uuid> {
    Uuid::parse_str(context.tenant_id.trim())
        .map_err(|_| GroupsError::Validation("tenant_id must be a UUID".to_string()))
}

fn actor_user_id(context: &PortContext) -> GroupsResult<Uuid> {
    if context.actor.kind != PortActorKind::User {
        return Err(GroupsError::Forbidden(
            "a user actor is required for direct group membership enforcement".to_string(),
        ));
    }
    Uuid::parse_str(context.actor.id.trim())
        .map_err(|_| GroupsError::Validation("actor.id must be a UUID".to_string()))
}

fn idempotency_key(context: &PortContext) -> GroupsResult<String> {
    context
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 160)
        .map(str::to_string)
        .ok_or_else(|| GroupsError::Validation("bounded idempotency key is required".to_string()))
}

fn has_platform_moderate(context: &PortContext) -> bool {
    context.claims.iter().any(|claim| {
        matches!(
            claim.as_str(),
            "groups:moderate" | "groups:manage" | "groups:*" | "*:*"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_codes_are_bounded_and_canonical() {
        assert_eq!(normalize_reason_code(" harassment ").unwrap(), "harassment");
        assert!(normalize_reason_code("Human readable reason").is_err());
        assert!(normalize_reason_code(&"x".repeat(81)).is_err());
    }

    #[test]
    fn moderation_provenance_requires_canonical_decision_identity() {
        let valid = MembershipEnforcementProvenance {
            source_kind: GroupMembershipEnforcementSourceKind::ModerationDecision,
            moderation_decision_id: Some(Uuid::new_v4()),
            moderation_decision_hash: Some("a".repeat(64)),
            actor_kind: "service".to_string(),
            actor_id: "rustok-moderation".to_string(),
            audit_actor_user_id: None,
        };
        assert!(validate_provenance(&valid).is_ok());

        let invalid = MembershipEnforcementProvenance {
            moderation_decision_hash: Some("A".repeat(64)),
            ..valid
        };
        assert!(validate_provenance(&invalid).is_err());
    }
}
