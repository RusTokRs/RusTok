use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::domain::{
    GroupMembershipEffectiveStatus, GroupMembershipEnforcementSourceKind,
    GroupMembershipEnforcementState, GroupMembershipStatus, GroupRole,
};
use crate::dto::{
    GroupMembershipEffectiveState, GroupMembershipEnforcementSummary,
    ReadGroupMembershipEnforcementRequest,
};
use crate::error::{GroupsError, GroupsResult};
use crate::membership_enforcement_entities::{membership_enforcement, membership_state};
use crate::ports::GroupMembershipEnforcementReadPort;

#[derive(Clone)]
pub struct GroupMembershipEnforcementService {
    db: DatabaseConnection,
}

impl GroupMembershipEnforcementService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn read_effective_state_owned(
        &self,
        tenant_id: Uuid,
        request: ReadGroupMembershipEnforcementRequest,
        evaluated_at: DateTime<Utc>,
    ) -> GroupsResult<GroupMembershipEffectiveState> {
        resolve_group_membership_enforcement(
            &self.db,
            tenant_id,
            request.group_id,
            request.user_id,
            evaluated_at,
        )
        .await
    }
}

/// Canonical owner-clock resolver for a membership subject.
///
/// Existing lifecycle commands still write the legacy membership table, while database
/// revision guards keep the subject revision monotonic. Access consumers must use this
/// resolver rather than interpreting `group_memberships.status` directly.
pub(crate) async fn resolve_group_membership_enforcement<C>(
    connection: &C,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
    evaluated_at: DateTime<Utc>,
) -> GroupsResult<GroupMembershipEffectiveState>
where
    C: ConnectionTrait,
{
    let Some(membership) = membership_state::Entity::find()
        .filter(membership_state::Column::TenantId.eq(tenant_id))
        .filter(membership_state::Column::GroupId.eq(group_id))
        .filter(membership_state::Column::UserId.eq(user_id))
        .one(connection)
        .await?
    else {
        return Ok(GroupMembershipEffectiveState {
            tenant_id,
            group_id,
            user_id,
            membership_id: None,
            role: None,
            stored_status: None,
            membership_revision: None,
            effective_status: GroupMembershipEffectiveStatus::Missing,
            active_member: false,
            denied_reentry: false,
            enforcement: None,
            evaluated_at,
        });
    };

    if membership.revision < 1 {
        return Err(GroupsError::Invariant(
            "stored group membership revision must be positive".to_string(),
        ));
    }
    let role = GroupRole::from_str(&membership.role).map_err(GroupsError::Invariant)?;
    let stored_status =
        GroupMembershipStatus::from_str(&membership.status).map_err(GroupsError::Invariant)?;

    let enforcement = membership_enforcement::Entity::find_by_id(membership.id)
        .filter(membership_enforcement::Column::TenantId.eq(tenant_id))
        .one(connection)
        .await?
        .map(|row| map_enforcement(row, &membership, evaluated_at))
        .transpose()?;

    let effective_status = resolve_effective_status(
        stored_status,
        enforcement.as_ref().is_some_and(|row| row.is_effective),
    );

    Ok(GroupMembershipEffectiveState {
        tenant_id,
        group_id,
        user_id,
        membership_id: Some(membership.id),
        role: Some(role),
        stored_status: Some(stored_status),
        membership_revision: Some(membership.revision),
        effective_status,
        active_member: effective_status.is_active_member(),
        denied_reentry: effective_status.denies_reentry(),
        enforcement,
        evaluated_at,
    })
}

fn map_enforcement(
    row: membership_enforcement::Model,
    membership: &membership_state::Model,
    evaluated_at: DateTime<Utc>,
) -> GroupsResult<GroupMembershipEnforcementSummary> {
    if row.tenant_id != membership.tenant_id
        || row.membership_id != membership.id
        || row.group_id != membership.group_id
        || row.user_id != membership.user_id
    {
        return Err(GroupsError::Invariant(
            "membership enforcement identity does not match its membership".to_string(),
        ));
    }
    if row.revision < 1 {
        return Err(GroupsError::Invariant(
            "stored membership enforcement revision must be positive".to_string(),
        ));
    }
    if row.reason_code.trim().is_empty() || row.actor_id.trim().is_empty() {
        return Err(GroupsError::Invariant(
            "membership enforcement reason and actor must be present".to_string(),
        ));
    }
    if !matches!(row.actor_kind.as_str(), "user" | "service" | "system") {
        return Err(GroupsError::Invariant(
            "stored membership enforcement actor kind is unsupported".to_string(),
        ));
    }

    let state = GroupMembershipEnforcementState::from_str(&row.state)
        .map_err(GroupsError::Invariant)?;
    let source_kind = GroupMembershipEnforcementSourceKind::from_str(&row.source_kind)
        .map_err(GroupsError::Invariant)?;
    let restore_status = GroupMembershipStatus::from_str(&row.restore_status)
        .map_err(GroupsError::Invariant)?;
    if restore_status == GroupMembershipStatus::Banned {
        return Err(GroupsError::Invariant(
            "membership enforcement cannot restore legacy banned state".to_string(),
        ));
    }

    match source_kind {
        GroupMembershipEnforcementSourceKind::DirectLocal => {
            if row.moderation_decision_id.is_some() || row.moderation_decision_hash.is_some() {
                return Err(GroupsError::Invariant(
                    "direct local enforcement must not carry moderation decision identity"
                        .to_string(),
                ));
            }
        }
        GroupMembershipEnforcementSourceKind::ModerationDecision => {
            let hash = row.moderation_decision_hash.as_deref().ok_or_else(|| {
                GroupsError::Invariant(
                    "moderation-driven enforcement requires a decision hash".to_string(),
                )
            })?;
            if row.moderation_decision_id.is_none()
                || hash.len() != 64
                || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(GroupsError::Invariant(
                    "moderation-driven enforcement decision identity is invalid".to_string(),
                ));
            }
        }
    }

    let effective_from = row.effective_from.with_timezone(&Utc);
    let effective_until = row
        .effective_until
        .map(|value| value.with_timezone(&Utc));
    if effective_until.is_some_and(|until| until <= effective_from) {
        return Err(GroupsError::Invariant(
            "membership enforcement expiry must follow its effective start".to_string(),
        ));
    }
    let revoked_at = row.revoked_at.map(|value| value.with_timezone(&Utc));
    let is_effective = revoked_at.is_none()
        && effective_from <= evaluated_at
        && effective_until.is_none_or(|until| evaluated_at < until);

    Ok(GroupMembershipEnforcementSummary {
        membership_id: row.membership_id,
        state,
        reason_code: row.reason_code,
        source_kind,
        effective_from,
        effective_until,
        restore_status,
        moderation_decision_id: row.moderation_decision_id,
        moderation_decision_hash: row.moderation_decision_hash,
        actor_kind: row.actor_kind,
        actor_id: row.actor_id,
        revision: row.revision,
        revoked_at,
        is_effective,
    })
}

fn resolve_effective_status(
    stored_status: GroupMembershipStatus,
    has_effective_suspension: bool,
) -> GroupMembershipEffectiveStatus {
    if has_effective_suspension {
        GroupMembershipEffectiveStatus::Suspended
    } else if stored_status == GroupMembershipStatus::Banned {
        GroupMembershipEffectiveStatus::LegacyBanned
    } else if stored_status == GroupMembershipStatus::Active {
        GroupMembershipEffectiveStatus::Active
    } else {
        GroupMembershipEffectiveStatus::Inactive
    }
}

#[async_trait]
impl GroupMembershipEnforcementReadPort for GroupMembershipEnforcementService {
    async fn read_membership_enforcement(
        &self,
        context: PortContext,
        request: ReadGroupMembershipEnforcementRequest,
    ) -> Result<GroupMembershipEffectiveState, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        if !can_read_effective_membership(&context, request.user_id) {
            return Err(PortError::forbidden(
                "groups.membership_enforcement_forbidden",
                "membership enforcement state is not visible to this actor",
            ));
        }
        let tenant_id = Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
            PortError::validation("groups.tenant_id_invalid", "tenant_id must be a UUID")
        })?;
        self.read_effective_state_owned(tenant_id, request, Utc::now())
            .await
            .map_err(Into::into)
    }
}

fn can_read_effective_membership(context: &PortContext, target_user_id: Uuid) -> bool {
    let exact_user = context.actor.kind == PortActorKind::User
        && Uuid::parse_str(context.actor.id.trim()).ok() == Some(target_user_id);
    exact_user
        || context.claims.iter().any(|claim| {
            matches!(
                claim.as_str(),
                "groups:access:read"
                    | "groups:read"
                    | "groups:manage"
                    | "groups:*"
                    | "*: *"
                    | "*:*"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_suspension_overrides_stored_lifecycle_state() {
        assert_eq!(
            resolve_effective_status(GroupMembershipStatus::Active, true),
            GroupMembershipEffectiveStatus::Suspended
        );
        assert_eq!(
            resolve_effective_status(GroupMembershipStatus::Pending, true),
            GroupMembershipEffectiveStatus::Suspended
        );
    }

    #[test]
    fn expired_or_revoked_enforcement_falls_back_to_stored_state() {
        assert_eq!(
            resolve_effective_status(GroupMembershipStatus::Active, false),
            GroupMembershipEffectiveStatus::Active
        );
        assert_eq!(
            resolve_effective_status(GroupMembershipStatus::Banned, false),
            GroupMembershipEffectiveStatus::LegacyBanned
        );
        assert_eq!(
            resolve_effective_status(GroupMembershipStatus::Left, false),
            GroupMembershipEffectiveStatus::Inactive
        );
    }
}
