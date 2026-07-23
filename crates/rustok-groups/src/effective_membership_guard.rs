use chrono::Utc;
use rustok_api::{PortActorKind, PortContext, PortError};
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::domain::{GroupMembershipEffectiveStatus, GroupRole};
use crate::error::{GroupsError, GroupsResult};
use crate::governance_entities::command_receipt;
use crate::membership_enforcement::resolve_group_membership_enforcement;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupManagerCapability {
    Moderate,
    ManageSettings,
}

pub(crate) fn tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
        PortError::validation("groups.invalid_tenant", "tenant_id must be a UUID")
    })
}

pub(crate) fn actor_user_id(context: &PortContext) -> Result<Uuid, PortError> {
    if context.actor.kind != PortActorKind::User {
        return Err(PortError::forbidden(
            "groups.user_actor_required",
            "a user actor is required",
        ));
    }
    Uuid::parse_str(context.actor.id.trim()).map_err(|_| {
        PortError::validation("groups.invalid_actor", "actor.id must be a UUID")
    })
}

pub(crate) fn has_platform_manage(context: &PortContext) -> bool {
    context
        .claims
        .iter()
        .any(|claim| matches!(claim.as_str(), "groups:manage" | "groups:*" | "*:*") )
}

/// Receipt-first replay compatibility guard used by public facades.
///
/// Existing receipts are delegated before current effective-state evaluation so identical replay
/// and changed-request conflict semantics remain owned by the command transaction.
pub(crate) async fn has_existing_receipt(
    db: &DatabaseConnection,
    context: &PortContext,
) -> Result<bool, PortError> {
    let tenant_id = tenant_id(context)?;
    let idempotency_key = context
        .idempotency_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            PortError::validation(
                "port.idempotency_key_required",
                "write port calls require a non-empty idempotency key",
            )
        })?;

    command_receipt::Entity::find()
        .filter(command_receipt::Column::TenantId.eq(tenant_id))
        .filter(command_receipt::Column::IdempotencyKey.eq(idempotency_key))
        .one(db)
        .await
        .map(|row| row.is_some())
        .map_err(|error| {
            PortError::unavailable("groups.receipt_lookup_unavailable", error.to_string())
        })
}

/// Canonical transaction-aware manager authorization.
///
/// Call this with the same owner transaction that will mutate invitation/application state. The
/// resolver reads the membership and enforcement projection through that transaction, eliminating
/// the facade-precheck-to-mutation authorization race.
pub(crate) async fn require_effective_manager_owned<C>(
    connection: &C,
    context: &PortContext,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
    capability: GroupManagerCapability,
) -> GroupsResult<()>
where
    C: ConnectionTrait,
{
    if has_platform_manage(context) {
        return Ok(());
    }

    let effective = resolve_group_membership_enforcement(
        connection,
        tenant_id,
        group_id,
        actor_user_id,
        Utc::now(),
    )
    .await?;

    if effective.effective_status == GroupMembershipEffectiveStatus::Suspended {
        return Err(GroupsError::Forbidden(
            "the actor's group membership is suspended".to_string(),
        ));
    }
    if effective.effective_status == GroupMembershipEffectiveStatus::LegacyBanned {
        return Err(GroupsError::Forbidden(
            "the actor's group membership is banned".to_string(),
        ));
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
        Err(GroupsError::Forbidden(message.to_string()))
    }
}

/// Canonical transaction-aware candidate/subject authorization.
pub(crate) async fn require_user_not_denied_owned<C>(
    connection: &C,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
    reject_active_member: bool,
) -> GroupsResult<()>
where
    C: ConnectionTrait,
{
    let effective =
        resolve_group_membership_enforcement(connection, tenant_id, group_id, user_id, Utc::now())
            .await?;

    match effective.effective_status {
        GroupMembershipEffectiveStatus::Suspended => Err(GroupsError::Forbidden(
            "group membership is suspended".to_string(),
        )),
        GroupMembershipEffectiveStatus::LegacyBanned => Err(GroupsError::Forbidden(
            "group membership is banned".to_string(),
        )),
        GroupMembershipEffectiveStatus::Active if reject_active_member => Err(
            GroupsError::Conflict("user is already an active group member".to_string()),
        ),
        _ => Ok(()),
    }
}

pub(crate) async fn require_effective_manager(
    db: &DatabaseConnection,
    context: &PortContext,
    group_id: Uuid,
    capability: GroupManagerCapability,
) -> Result<(), PortError> {
    require_effective_manager_owned(
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
    require_user_not_denied_owned(
        db,
        tenant_id,
        group_id,
        user_id,
        reject_active_member,
    )
    .await
    .map_err(Into::into)
}
