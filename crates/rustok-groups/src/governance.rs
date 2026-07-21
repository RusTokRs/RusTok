use std::str::FromStr;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::{GroupMembershipStatus, GroupRole};
use crate::entities::{group, membership};
use crate::error::{GroupsError, GroupsResult};
use crate::governance_entities::{audit_entry, command_receipt};

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
        if let Some(replayed) = replay_receipt::<GroupGovernanceResult>(
            &transaction,
            tenant_id,
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

        let mut group_model = find_group(&transaction, tenant_id, request.group_id).await?;
        let actor_membership = find_membership(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
        )
        .await?;
        let target_membership = find_membership(
            &transaction,
            tenant_id,
            request.group_id,
            request.target_user_id,
        )
        .await?;

        let actor_role = active_role(&actor_membership)?;
        let target_role = active_role(&target_membership)?;
        if target_role == GroupRole::Owner {
            return Err(GroupsError::Invariant(
                "the owner role can only change through ownership transfer".to_string(),
            ));
        }
        authorize_role_change(actor_role, target_role, request.role, has_platform_manage(context))?;

        let now = Utc::now().fixed_offset();
        let mut target_active: membership::ActiveModel = target_membership.into();
        target_active.role = Set(request.role.as_str().to_string());
        target_active.updated_at = Set(now);
        target_active.update(&transaction).await?;

        group_model.version = group_model.version.saturating_add(1);
        group_model.updated_at = now;
        let group_version = group_model.version.max(1) as u64;
        let mut group_active: group::ActiveModel = group_model.into();
        group_active.version = Set(group_version as i64);
        group_active.updated_at = Set(now);
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
                "group_version": group_version
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
        if let Some(replayed) = replay_receipt::<GroupGovernanceResult>(
            &transaction,
            tenant_id,
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

        let group_model = find_group(&transaction, tenant_id, request.group_id).await?;
        if group_model.owner_user_id != actor_user_id && !has_platform_manage(context) {
            return Err(GroupsError::Forbidden(
                "only the current owner or a platform group manager may transfer ownership"
                    .to_string(),
            ));
        }
        let previous_owner_id = group_model.owner_user_id;
        let previous_owner = find_membership(
            &transaction,
            tenant_id,
            request.group_id,
            previous_owner_id,
        )
        .await?;
        let new_owner = find_membership(
            &transaction,
            tenant_id,
            request.group_id,
            request.new_owner_user_id,
        )
        .await?;
        if active_role(&previous_owner)? != GroupRole::Owner {
            return Err(GroupsError::Invariant(
                "group owner membership does not match the group owner reference".to_string(),
            ));
        }
        let previous_target_role = active_role(&new_owner)?;

        let now = Utc::now().fixed_offset();
        let mut previous_owner_active: membership::ActiveModel = previous_owner.into();
        previous_owner_active.role = Set(GroupRole::Admin.as_str().to_string());
        previous_owner_active.updated_at = Set(now);
        previous_owner_active.update(&transaction).await?;

        let mut new_owner_active: membership::ActiveModel = new_owner.into();
        new_owner_active.role = Set(GroupRole::Owner.as_str().to_string());
        new_owner_active.updated_at = Set(now);
        new_owner_active.update(&transaction).await?;

        let group_version = group_model.version.saturating_add(1).max(1) as u64;
        let mut group_active: group::ActiveModel = group_model.into();
        group_active.owner_user_id = Set(request.new_owner_user_id);
        group_active.version = Set(group_version as i64);
        group_active.updated_at = Set(now);
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
                "group_version": group_version
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
        self.change_role_owned(&context, request).await.map_err(Into::into)
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

async fn find_group(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
) -> GroupsResult<group::Model> {
    group::Entity::find()
        .filter(group::Column::TenantId.eq(tenant_id))
        .filter(group::Column::Id.eq(group_id))
        .one(transaction)
        .await?
        .ok_or(GroupsError::NotFound)
}

async fn find_membership(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> GroupsResult<membership::Model> {
    membership::Entity::find()
        .filter(membership::Column::TenantId.eq(tenant_id))
        .filter(membership::Column::GroupId.eq(group_id))
        .filter(membership::Column::UserId.eq(user_id))
        .one(transaction)
        .await?
        .ok_or_else(|| GroupsError::Conflict("an active group membership is required".to_string()))
}

fn active_role(model: &membership::Model) -> GroupsResult<GroupRole> {
    let status = GroupMembershipStatus::from_str(&model.status).map_err(GroupsError::Invariant)?;
    if status != GroupMembershipStatus::Active {
        return Err(GroupsError::Conflict(
            "an active group membership is required".to_string(),
        ));
    }
    GroupRole::from_str(&model.role).map_err(GroupsError::Invariant)
}

fn authorize_role_change(
    actor_role: GroupRole,
    target_role: GroupRole,
    requested_role: GroupRole,
    platform_manage: bool,
) -> GroupsResult<()> {
    if platform_manage {
        return Ok(());
    }
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
    if receipt.command_type != command_type || receipt.request_hash != request_hash {
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
            GroupsError::Invariant(format!("group command response is not serializable: {error}"))
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
        GroupsError::Validation(format!("group command request is not serializable: {error}"))
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
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
        .any(|claim| matches!(claim.as_str(), "groups:manage" | "groups:*" | "*:*") )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admins_cannot_promote_another_admin() {
        assert!(authorize_role_change(
            GroupRole::Admin,
            GroupRole::Admin,
            GroupRole::Moderator,
            false,
        )
        .is_err());
    }

    #[test]
    fn admins_can_manage_moderators_and_members() {
        assert!(authorize_role_change(
            GroupRole::Admin,
            GroupRole::Member,
            GroupRole::Moderator,
            false,
        )
        .is_ok());
    }

    #[test]
    fn owners_can_delegate_all_non_owner_roles() {
        assert!(authorize_role_change(
            GroupRole::Owner,
            GroupRole::Admin,
            GroupRole::Member,
            false,
        )
        .is_ok());
    }
}
