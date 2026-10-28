use chrono::Utc;
use rustok_api::{PortActorKind, PortContext, PortError};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use crate::domain::{GroupMembershipEffectiveStatus, GroupRole};
use crate::error::{GroupsError, GroupsResult};
use crate::membership_enforcement::resolve_group_membership_enforcement;
use crate::membership_enforcement_transaction::resolve_group_membership_enforcement_now_for_update;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupManagerCapability {
    Moderate,
    ManageSettings,
}

pub(crate) fn tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    let trimmed = context.tenant_id.trim();
    if trimmed.is_empty() {
        return Err(PortError::validation(
            "groups.invalid_tenant",
            "tenant_id cannot be empty",
        ));
    }
    let parsed = Uuid::parse_str(trimmed)
        .map_err(|_| PortError::validation("groups.invalid_tenant", "tenant_id must be a UUID"))?;
    if parsed.is_nil() {
        return Err(PortError::validation(
            "groups.invalid_tenant",
            "tenant_id cannot be nil UUID",
        ));
    }
    Ok(parsed)
}

pub(crate) fn actor_user_id(context: &PortContext) -> Result<Uuid, PortError> {
    if context.actor.kind != PortActorKind::User {
        return Err(PortError::forbidden(
            "groups.user_actor_required",
            "a user actor is required",
        ));
    }
    let trimmed = context.actor.id.trim();
    if trimmed.is_empty() {
        return Err(PortError::validation(
            "groups.invalid_actor",
            "actor.id cannot be empty",
        ));
    }
    let parsed = Uuid::parse_str(trimmed)
        .map_err(|_| PortError::validation("groups.invalid_actor", "actor.id must be a UUID"))?;
    if parsed.is_nil() {
        return Err(PortError::validation(
            "groups.invalid_actor",
            "actor.id cannot be nil UUID",
        ));
    }
    Ok(parsed)
}

pub(crate) fn has_platform_manage(context: &PortContext) -> bool {
    context
        .claims
        .iter()
        .any(|claim| matches!(claim.as_str(), "groups:manage" | "groups:*" | "*:*"))
}

/// Canonical transaction-aware manager authorization.
///
/// The resolver acquires the Groups owner lock order `Group -> GroupMembership ->
/// GroupMembershipEnforcement` before evaluating authority, so a concurrent enforcement mutation
/// cannot commit between this check and the command's first domain write.
pub(crate) async fn require_effective_manager_owned(
    transaction: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
    capability: GroupManagerCapability,
) -> GroupsResult<()> {
    if tenant_id.is_nil() {
        return Err(GroupsError::Validation(
            "tenant_id cannot be nil UUID".to_string(),
        ));
    }
    if group_id.is_nil() {
        return Err(GroupsError::Validation(
            "group_id cannot be nil UUID".to_string(),
        ));
    }
    if actor_user_id.is_nil() {
        return Err(GroupsError::Validation(
            "actor_user_id cannot be nil UUID".to_string(),
        ));
    }
    if has_platform_manage(context) {
        return Ok(());
    }

    let effective = resolve_group_membership_enforcement_now_for_update(
        transaction,
        tenant_id,
        group_id,
        actor_user_id,
    )
    .await?;
    require_manager_state(effective, capability)
}

/// Canonical owner-clock manager authorization for read-only owner surfaces.
///
/// This shares the exact effective-state semantics with the transactional guard without acquiring
/// write locks. Callers that mutate owner state must use `require_effective_manager_owned` instead.
pub(crate) async fn require_effective_manager_direct_owned(
    db: &DatabaseConnection,
    context: &PortContext,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
    capability: GroupManagerCapability,
) -> GroupsResult<()> {
    if tenant_id.is_nil() {
        return Err(GroupsError::Validation(
            "tenant_id cannot be nil UUID".to_string(),
        ));
    }
    if group_id.is_nil() {
        return Err(GroupsError::Validation(
            "group_id cannot be nil UUID".to_string(),
        ));
    }
    if actor_user_id.is_nil() {
        return Err(GroupsError::Validation(
            "actor_user_id cannot be nil UUID".to_string(),
        ));
    }
    if has_platform_manage(context) {
        return Ok(());
    }

    let effective =
        resolve_group_membership_enforcement(db, tenant_id, group_id, actor_user_id, Utc::now())
            .await?;
    require_manager_state(effective, capability)
}

/// Canonical transaction-aware candidate/subject authorization under the same owner lock order.
pub(crate) async fn require_user_not_denied_owned(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
    reject_active_member: bool,
) -> GroupsResult<()> {
    if tenant_id.is_nil() {
        return Err(GroupsError::Validation(
            "tenant_id cannot be nil UUID".to_string(),
        ));
    }
    if group_id.is_nil() {
        return Err(GroupsError::Validation(
            "group_id cannot be nil UUID".to_string(),
        ));
    }
    if user_id.is_nil() {
        return Err(GroupsError::Validation(
            "user_id cannot be nil UUID".to_string(),
        ));
    }
    let effective = resolve_group_membership_enforcement_now_for_update(
        transaction,
        tenant_id,
        group_id,
        user_id,
    )
    .await?;
    require_candidate_state(effective.effective_status, reject_active_member)
}

pub(crate) async fn require_effective_manager(
    db: &DatabaseConnection,
    context: &PortContext,
    group_id: Uuid,
    capability: GroupManagerCapability,
) -> Result<(), PortError> {
    if group_id.is_nil() {
        return Err(PortError::validation(
            "groups.invalid_group_id",
            "group_id cannot be nil UUID",
        ));
    }
    if has_platform_manage(context) {
        return Ok(());
    }
    require_effective_manager_direct_owned(
        db,
        context,
        tenant_id(context)?,
        group_id,
        actor_user_id(context)?,
        capability,
    )
    .await
    .map_err(Into::into)
}

pub(crate) async fn require_candidate_not_denied(
    db: &DatabaseConnection,
    context: &PortContext,
    group_id: Uuid,
    reject_active_member: bool,
) -> Result<(), PortError> {
    if group_id.is_nil() {
        return Err(PortError::validation(
            "groups.invalid_group_id",
            "group_id cannot be nil UUID",
        ));
    }
    require_user_not_denied(
        db,
        tenant_id(context)?,
        group_id,
        actor_user_id(context)?,
        reject_active_member,
    )
    .await
}

pub(crate) async fn require_user_not_denied(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
    reject_active_member: bool,
) -> Result<(), PortError> {
    if tenant_id.is_nil() {
        return Err(PortError::validation(
            "groups.invalid_tenant",
            "tenant_id cannot be nil UUID",
        ));
    }
    if group_id.is_nil() {
        return Err(PortError::validation(
            "groups.invalid_group_id",
            "group_id cannot be nil UUID",
        ));
    }
    if user_id.is_nil() {
        return Err(PortError::validation(
            "groups.invalid_user_id",
            "user_id cannot be nil UUID",
        ));
    }
    let effective =
        resolve_group_membership_enforcement(db, tenant_id, group_id, user_id, Utc::now())
            .await
            .map_err(PortError::from)?;
    require_candidate_state(effective.effective_status, reject_active_member).map_err(Into::into)
}

fn require_manager_state(
    effective: crate::dto::GroupMembershipEffectiveState,
    capability: GroupManagerCapability,
) -> GroupsResult<()> {
    if effective.effective_status == GroupMembershipEffectiveStatus::Suspended {
        return Err(GroupsError::MembershipSuspended);
    }
    if effective.effective_status == GroupMembershipEffectiveStatus::LegacyBanned {
        return Err(GroupsError::MembershipBanned);
    }

    let role_allowed = match capability {
        GroupManagerCapability::Moderate => effective.role.is_some_and(GroupRole::can_moderate),
        GroupManagerCapability::ManageSettings => {
            effective.role.is_some_and(GroupRole::can_manage_settings)
        }
    };
    if effective.active_member && role_allowed {
        Ok(())
    } else {
        let message = match capability {
            GroupManagerCapability::Moderate => {
                "an active group owner, administrator, or moderator role is required"
            }
            GroupManagerCapability::ManageSettings => {
                "an active group owner or administrator role is required"
            }
        };
        Err(GroupsError::ManagerRequired(message.to_string()))
    }
}

fn require_candidate_state(
    effective_status: GroupMembershipEffectiveStatus,
    reject_active_member: bool,
) -> GroupsResult<()> {
    match effective_status {
        GroupMembershipEffectiveStatus::Suspended => Err(GroupsError::MembershipSuspended),
        GroupMembershipEffectiveStatus::LegacyBanned => Err(GroupsError::MembershipBanned),
        GroupMembershipEffectiveStatus::Active if reject_active_member => {
            Err(GroupsError::MembershipAlreadyActive)
        }
        _ => Ok(()),
    }
}
