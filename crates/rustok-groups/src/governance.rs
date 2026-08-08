use std::str::FromStr;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, EntityTrait, QueryFilter, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::domain::{GroupMembershipEffectiveStatus, GroupMembershipStatus, GroupRole};
use crate::entities::{group, membership};
use crate::error::{GroupsError, GroupsResult};
use crate::governance_entities::{audit_entry, command_receipt};
use crate::membership_enforcement::resolve_group_membership_enforcement;
use crate::membership_enforcement_entities::membership_enforcement;
use crate::membership_enforcement_transaction::reserve_group_write_for_update;

const CHANGE_ROLE_COMMAND: &str = "groups.change_role.v1";
const TRANSFER_OWNERSHIP_COMMAND: &str = "groups.transfer_ownership.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeGroupRoleRequest {
    pub group_id: Uuid,
    pub target_user_id: Uuid,
    pub role: GroupRole,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferGroupOwnershipRequest {
    pub group_id: Uuid,
    pub new_owner_user_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupGovernanceResult {
    pub group_id: Uuid,
    pub actor_user_id: Uuid,
    pub target_user_id: Uuid,
    pub previous_role: GroupRole,
    pub current_role: GroupRole,
    pub group_version: u64,
    pub replayed: bool,
}

#[async_trait]
pub trait GroupGovernanceCommandPort: Send + Sync {
    async fn change_group_role(
        &self,
        context: PortContext,
        request: ChangeGroupRoleRequest,
    ) -> Result<GroupGovernanceResult, PortError>;

    async fn transfer_group_ownership(
        &self,
        context: PortContext,
        request: TransferGroupOwnershipRequest,
    ) -> Result<GroupGovernanceResult, PortError>;
}

#[derive(Clone)]
pub struct GroupGovernanceService {
    db: DatabaseConnection,
}

impl GroupGovernanceService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn change_role_owned(
        &self,
        context: &PortContext,
        request: ChangeGroupRoleRequest,
    ) -> GroupsResult<GroupGovernanceResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        if actor_user_id == request.target_user_id {
            return Err(GroupsError::Conflict(
                "a member cannot change their own local role".to_string(),
            ));
        }
        if request.role == GroupRole::Owner {
            return Err(GroupsError::Validation(
                "use ownership transfer to assign the owner role".to_string(),
            ));
        }

        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;
        let mut group_model = lock_group(&transaction, tenant_id, request.group_id).await?;
        if let Some(replayed) = replay_receipt::<GroupGovernanceResult>(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            &idempotency_key,
            CHANGE_ROLE_COMMAND,
            &request_hash,
        )
        .await?
        {
            transaction.commit().await?;
            return Ok(GroupGovernanceResult {
                replayed: true,
                ..replayed
            });
        }

        let platform_manage = has_platform_manage(context);
        let locked = lock_governance_memberships(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            request.target_user_id,
            !platform_manage,
        )
        .await?;
        let target_membership = locked
            .membership(request.target_user_id)
            .ok_or_else(|| GroupsError::Conflict("an active group membership is required".to_string()))?;
        validate_owner_identity(&group_model, &target_membership)?;
        if target_membership.user_id == group_model.owner_user_id {
            return Err(GroupsError::Invariant(
                "the owner role can only change through ownership transfer".to_string(),
            ));
        }

        let now = Utc::now();
        let target_role = effective_governance_role(
            &transaction,
            tenant_id,
            request.group_id,
            request.target_user_id,
            now,
        )
        .await?;
        let actor_role = if platform_manage {
            None
        } else {
            if locked.membership(actor_user_id).is_none() {
                return Err(GroupsError::ManagerRequired(
                    "an active group owner or administrator role is required".to_string(),
                ));
            }
            Some(
                effective_manager_role(
                    &transaction,
                    tenant_id,
                    request.group_id,
                    actor_user_id,
                    now,
                )
                .await?,
            )
        };
        authorize_role_change(actor_role, target_role, request.role, platform_manage)?;

        let fixed_now = now.fixed_offset();
        let mut target_active: membership::ActiveModel = target_membership.into();
        target_active.role = Set(request.role.as_str().to_string());
        target_active.updated_at = Set(fixed_now);
        target_active.update(&transaction).await?;

        group_model.version = group_model.version.saturating_add(1);
        group_model.updated_at = fixed_now;
        let group_version = group_model.version.max(1) as u64;
        let mut group_active: group::ActiveModel = group_model.into();
        group_active.version = Set(group_version as i64);
        group_active.updated_at = Set(fixed_now);
        group_active.update(&transaction).await?;

        let result = GroupGovernanceResult {
            group_id: request.group_id,
            actor_user_id,
            target_user_id: request.target_user_id,
            previous_role: target_role,
            current_role: request.role,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            Some(actor_user_id),
            "group.role_changed",
            Some(request.target_user_id),
            json!({
                "previous_role": target_role.as_str(),
                "current_role": request.role.as_str(),
                "group_version": group_version,
                "authorization": if platform_manage { "platform_manage" } else { "effective_local_manager" }
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            idempotency_key,
            CHANGE_ROLE_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn transfer_ownership_owned(
        &self,
        context: &PortContext,
        request: TransferGroupOwnershipRequest,
    ) -> GroupsResult<GroupGovernanceResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        if actor_user_id == request.new_owner_user_id {
            return Err(GroupsError::Conflict(
                "the selected user already acts as the requested owner".to_string(),
            ));
        }

        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;
        let group_model = lock_group(&transaction, tenant_id, request.group_id).await?;
        if let Some(replayed) = replay_receipt::<GroupGovernanceResult>(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            &idempotency_key,
            TRANSFER_OWNERSHIP_COMMAND,
            &request_hash,
        )
        .await?
        {
            transaction.commit().await?;
            return Ok(GroupGovernanceResult {
                replayed: true,
                ..replayed
            });
        }

        let platform_manage = has_platform_manage(context);
        if group_model.owner_user_id != actor_user_id && !platform_manage {
            return Err(GroupsError::Forbidden(
                "only the current owner or a platform group manager may transfer ownership"
                    .to_string(),
            ));
        }

        let previous_owner_id = group_model.owner_user_id;
        let locked = lock_ownership_memberships(
            &transaction,
            tenant_id,
            request.group_id,
            previous_owner_id,
            request.new_owner_user_id,
            actor_user_id,
            !platform_manage,
        )
        .await?;
        let previous_owner = locked.membership(previous_owner_id).ok_or_else(|| {
            GroupsError::Invariant(
                "group owner membership does not match the group owner reference".to_string(),
            )
        })?;
        validate_owner_identity(&group_model, &previous_owner)?;
        if stored_active_role(&previous_owner)? != GroupRole::Owner {
            return Err(GroupsError::Invariant(
                "group owner membership does not match the group owner reference".to_string(),
            ));
        }

        let new_owner = locked
            .membership(request.new_owner_user_id)
            .ok_or_else(|| GroupsError::Conflict("an active group membership is required".to_string()))?;
        validate_owner_identity(&group_model, &new_owner)?;
        if new_owner.user_id == previous_owner_id {
            return Err(GroupsError::Conflict(
                "the selected user already acts as the requested owner".to_string(),
            ));
        }

        let now = Utc::now();
        if platform_manage {
            validate_platform_recovery_owner_state(
                &transaction,
                tenant_id,
                request.group_id,
                previous_owner_id,
                now,
            )
            .await?;
        } else {
            let actor_role = effective_manager_role(
                &transaction,
                tenant_id,
                request.group_id,
                actor_user_id,
                now,
            )
            .await?;
            if actor_role != GroupRole::Owner {
                return Err(GroupsError::Forbidden(
                    "only the current owner or a platform group manager may transfer ownership"
                        .to_string(),
                ));
            }
        }
        let previous_target_role = effective_governance_role(
            &transaction,
            tenant_id,
            request.group_id,
            request.new_owner_user_id,
            now,
        )
        .await?;

        let fixed_now = now.fixed_offset();
        let mut previous_owner_active: membership::ActiveModel = previous_owner.into();
        previous_owner_active.role = Set(GroupRole::Admin.as_str().to_string());
        previous_owner_active.updated_at = Set(fixed_now);
        previous_owner_active.update(&transaction).await?;

        let mut new_owner_active: membership::ActiveModel = new_owner.into();
        new_owner_active.role = Set(GroupRole::Owner.as_str().to_string());
        new_owner_active.updated_at = Set(fixed_now);
        new_owner_active.update(&transaction).await?;

        let group_version = group_model.version.saturating_add(1).max(1) as u64;
        let mut group_active: group::ActiveModel = group_model.into();
        group_active.owner_user_id = Set(request.new_owner_user_id);
        group_active.version = Set(group_version as i64);
        group_active.updated_at = Set(fixed_now);
        group_active.update(&transaction).await?;

        let result = GroupGovernanceResult {
            group_id: request.group_id,
            actor_user_id,
            target_user_id: request.new_owner_user_id,
            previous_role: previous_target_role,
            current_role: GroupRole::Owner,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            Some(actor_user_id),
            "group.ownership_transferred",
            Some(request.new_owner_user_id),
            json!({
                "previous_owner_user_id": previous_owner_id,
                "new_owner_user_id": request.new_owner_user_id,
                "previous_target_role": previous_target_role.as_str(),
                "previous_owner_role": GroupRole::Admin.as_str(),
                "group_version": group_version,
                "authorization": if platform_manage { "platform_manage" } else { "effective_current_owner" }
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            idempotency_key,
            TRANSFER_OWNERSHIP_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}

#[async_trait]
impl GroupGovernanceCommandPort for GroupGovernanceService {
    async fn change_group_role(
        &self,
        context: PortContext,
        request: ChangeGroupRoleRequest,
    ) -> Result<GroupGovernanceResult, PortError> {
        self.change_role_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn transfer_group_ownership(
        &self,
        context: PortContext,
        request: TransferGroupOwnershipRequest,
    ) -> Result<GroupGovernanceResult, PortError> {
        self.transfer_ownership_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

struct LockedGovernanceState {
    memberships: Vec<membership::Model>,
}

impl LockedGovernanceState {
    fn membership(&self, user_id: Uuid) -> Option<membership::Model> {
        self.memberships
            .iter()
            .find(|row| row.user_id == user_id)
            .cloned()
    }
}

async fn lock_group(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
) -> GroupsResult<group::Model> {
    reserve_group_write_for_update(transaction, tenant_id, group_id).await?;
    group::Entity::find()
        .filter(group::Column::TenantId.eq(tenant_id))
        .filter(group::Column::Id.eq(group_id))
        .one(transaction)
        .await?
        .ok_or(GroupsError::NotFound)
}

async fn lock_governance_memberships(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
    target_user_id: Uuid,
    require_actor_membership: bool,
) -> GroupsResult<LockedGovernanceState> {
    let mut user_ids = vec![target_user_id];
    if require_actor_membership {
        user_ids.push(actor_user_id);
    }
    lock_membership_set(transaction, tenant_id, group_id, user_ids).await
}

async fn lock_ownership_memberships(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    previous_owner_user_id: Uuid,
    new_owner_user_id: Uuid,
    actor_user_id: Uuid,
    require_actor_membership: bool,
) -> GroupsResult<LockedGovernanceState> {
    let mut user_ids = vec![previous_owner_user_id, new_owner_user_id];
    if require_actor_membership {
        user_ids.push(actor_user_id);
    }
    lock_membership_set(transaction, tenant_id, group_id, user_ids).await
}

async fn lock_membership_set(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    mut user_ids: Vec<Uuid>,
) -> GroupsResult<LockedGovernanceState> {
    user_ids.sort_unstable();
    user_ids.dedup();

    let mut memberships = Vec::with_capacity(user_ids.len());
    for user_id in user_ids {
        let mut query = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(group_id))
            .filter(membership::Column::UserId.eq(user_id));
        if transaction.get_database_backend() != DatabaseBackend::Sqlite {
            query = query.lock_exclusive();
        }
        if let Some(row) = query.one(transaction).await? {
            memberships.push(row);
        }
    }

    let mut membership_ids = memberships.iter().map(|row| row.id).collect::<Vec<_>>();
    membership_ids.sort_unstable();
    for membership_id in membership_ids {
        let mut query = membership_enforcement::Entity::find_by_id(membership_id)
            .filter(membership_enforcement::Column::TenantId.eq(tenant_id));
        if transaction.get_database_backend() != DatabaseBackend::Sqlite {
            query = query.lock_exclusive();
        }
        query.one(transaction).await?;
    }

    Ok(LockedGovernanceState { memberships })
}

fn validate_owner_identity(
    group_model: &group::Model,
    membership_model: &membership::Model,
) -> GroupsResult<()> {
    let role = GroupRole::from_str(&membership_model.role).map_err(GroupsError::Invariant)?;
    let is_owner_reference = membership_model.user_id == group_model.owner_user_id;
    if is_owner_reference != (role == GroupRole::Owner) {
        return Err(GroupsError::Invariant(
            "group owner reference and owner membership role disagree".to_string(),
        ));
    }
    Ok(())
}

async fn effective_manager_role(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
    now: chrono::DateTime<Utc>,
) -> GroupsResult<GroupRole> {
    let state = resolve_group_membership_enforcement(transaction, tenant_id, group_id, user_id, now)
        .await?;
    match state.effective_status {
        GroupMembershipEffectiveStatus::Suspended => Err(GroupsError::MembershipSuspended),
        GroupMembershipEffectiveStatus::LegacyBanned => Err(GroupsError::MembershipBanned),
        GroupMembershipEffectiveStatus::Active => {
            let role = state.role.ok_or_else(|| {
                GroupsError::Invariant("active group membership is missing a local role".to_string())
            })?;
            if role.can_manage_settings() {
                Ok(role)
            } else {
                Err(GroupsError::ManagerRequired(
                    "an active group owner or administrator role is required".to_string(),
                ))
            }
        }
        _ => Err(GroupsError::ManagerRequired(
            "an active group owner or administrator role is required".to_string(),
        )),
    }
}

async fn effective_governance_role(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
    now: chrono::DateTime<Utc>,
) -> GroupsResult<GroupRole> {
    let state = resolve_group_membership_enforcement(transaction, tenant_id, group_id, user_id, now)
        .await?;
    match state.effective_status {
        GroupMembershipEffectiveStatus::Suspended => Err(GroupsError::MembershipSuspended),
        GroupMembershipEffectiveStatus::LegacyBanned => Err(GroupsError::MembershipBanned),
        GroupMembershipEffectiveStatus::Active => state.role.ok_or_else(|| {
            GroupsError::Invariant("active group membership is missing a local role".to_string())
        }),
        _ => Err(GroupsError::Conflict(
            "an active group membership is required".to_string(),
        )),
    }
}

async fn validate_platform_recovery_owner_state(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_user_id: Uuid,
    now: chrono::DateTime<Utc>,
) -> GroupsResult<()> {
    let state = resolve_group_membership_enforcement(
        transaction,
        tenant_id,
        group_id,
        owner_user_id,
        now,
    )
    .await?;
    match state.effective_status {
        GroupMembershipEffectiveStatus::Active | GroupMembershipEffectiveStatus::Suspended => Ok(()),
        GroupMembershipEffectiveStatus::LegacyBanned => Err(GroupsError::MembershipBanned),
        _ => Err(GroupsError::Invariant(
            "current group owner effective membership is not recoverable".to_string(),
        )),
    }
}

fn stored_active_role(model: &membership::Model) -> GroupsResult<GroupRole> {
    let status = GroupMembershipStatus::from_str(&model.status).map_err(GroupsError::Invariant)?;
    if status != GroupMembershipStatus::Active {
        return Err(GroupsError::Conflict(
            "an active group membership is required".to_string(),
        ));
    }
    GroupRole::from_str(&model.role).map_err(GroupsError::Invariant)
}

fn authorize_role_change(
    actor_role: Option<GroupRole>,
    target_role: GroupRole,
    requested_role: GroupRole,
    platform_manage: bool,
) -> GroupsResult<()> {
    if platform_manage {
        return Ok(());
    }
    let actor_role = actor_role.ok_or_else(|| {
        GroupsError::ManagerRequired(
            "an active group owner or administrator role is required".to_string(),
        )
    })?;
    let allowed = match actor_role {
        GroupRole::Owner => true,
        GroupRole::Admin => {
            target_role != GroupRole::Admin
                && matches!(requested_role, GroupRole::Moderator | GroupRole::Member)
        }
        GroupRole::Moderator | GroupRole::Member => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(GroupsError::Forbidden(
            "the local role cannot perform this role transition".to_string(),
        ))
    }
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
            "idempotency key was already used for another group command".to_string(),
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
                "group command response is not serializable: {error}"
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

fn request_hash<T: Serialize>(request: &T) -> GroupsResult<String> {
    let bytes = serde_json::to_vec(request).map_err(|error| {
        GroupsError::Validation(format!(
            "group command request is not serializable: {error}"
        ))
    })?;
    Ok(crate::domain::sha256_hex(&bytes))
}

fn require_write(context: &PortContext) -> GroupsResult<()> {
    context
        .require_policy(PortCallPolicy::write())
        .map_err(|error| GroupsError::Validation(error.message))
}

fn context_tenant_id(context: &PortContext) -> GroupsResult<Uuid> {
    Uuid::parse_str(&context.tenant_id)
        .map_err(|_| GroupsError::Validation("tenant_id must be a UUID".to_string()))
}

fn actor_user_id(context: &PortContext) -> GroupsResult<Uuid> {
    if context.actor.kind != PortActorKind::User {
        return Err(GroupsError::Forbidden(
            "a user actor is required for group governance".to_string(),
        ));
    }
    Uuid::parse_str(&context.actor.id)
        .map_err(|_| GroupsError::Validation("actor.id must be a UUID".to_string()))
}

fn idempotency_key(context: &PortContext) -> GroupsResult<String> {
    context
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| GroupsError::Validation("idempotency key is required".to_string()))
}

fn has_platform_manage(context: &PortContext) -> bool {
    context
        .claims
        .iter()
        .any(|claim| matches!(claim.as_str(), "groups:manage" | "groups:*" | "*:*"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admins_cannot_promote_another_admin() {
        assert!(
            authorize_role_change(
                Some(GroupRole::Admin),
                GroupRole::Admin,
                GroupRole::Moderator,
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn admins_can_manage_moderators_and_members() {
        assert!(
            authorize_role_change(
                Some(GroupRole::Admin),
                GroupRole::Member,
                GroupRole::Moderator,
                false,
            )
            .is_ok()
        );
    }

    #[test]
    fn owners_can_delegate_all_non_owner_roles() {
        assert!(
            authorize_role_change(
                Some(GroupRole::Owner),
                GroupRole::Admin,
                GroupRole::Member,
                false,
            )
            .is_ok()
        );
    }
}
