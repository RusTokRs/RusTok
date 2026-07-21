use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, DbBackend, EntityTrait,
    QueryFilter, QuerySelect, Set, TransactionTrait,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::{GroupMembershipStatus, GroupRole, GroupStatus};
use crate::dto::GroupMembership;
use crate::entities::{group, membership};
use crate::error::{GroupsError, GroupsResult};
use crate::governance_entities::{audit_entry, command_receipt};
use crate::invitation_entities::{invitation, redemption};
use crate::invitations::AcceptGroupInvitationResult;

const ACCEPT_TARGETED_INVITATION_COMMAND: &str = "groups.accept_targeted_invitation.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptTargetedGroupInvitationRequest {
    pub invitation_id: Uuid,
}

#[async_trait]
pub trait GroupTargetedInvitationCommandPort: Send + Sync {
    async fn accept_targeted_group_invitation(
        &self,
        context: PortContext,
        request: AcceptTargetedGroupInvitationRequest,
    ) -> Result<AcceptGroupInvitationResult, PortError>;
}

#[derive(Clone)]
pub struct GroupTargetedInvitationService {
    db: DatabaseConnection,
}

impl GroupTargetedInvitationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn accept_owned(
        &self,
        context: &PortContext,
        request: AcceptTargetedGroupInvitationRequest,
    ) -> GroupsResult<AcceptGroupInvitationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<AcceptGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            ACCEPT_TARGETED_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let invitation_model =
            find_invitation_for_update(&transaction, tenant_id, request.invitation_id).await?;
        ensure_targeted_invitation_active(&invitation_model, actor_user_id)?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, invitation_model.group_id).await?;
        require_active_group(&group_model)?;

        if redemption::Entity::find()
            .filter(redemption::Column::TenantId.eq(tenant_id))
            .filter(redemption::Column::InvitationId.eq(invitation_model.id))
            .filter(redemption::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?
            .is_some()
        {
            return Err(targeted_invitation_unavailable());
        }

        let existing_membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(invitation_model.group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?;
        if existing_membership
            .as_ref()
            .is_some_and(|row| row.status == GroupMembershipStatus::Banned.as_str())
        {
            return Err(GroupsError::Forbidden(
                "group membership is banned".to_string(),
            ));
        }
        if existing_membership
            .as_ref()
            .is_some_and(|row| row.status == GroupMembershipStatus::Active.as_str())
        {
            return Err(GroupsError::Conflict(
                "user is already an active group member".to_string(),
            ));
        }

        let now = Utc::now();
        let membership_model = if let Some(existing) = existing_membership {
            let mut active: membership::ActiveModel = existing.into();
            active.role = Set(GroupRole::Member.as_str().to_string());
            active.status = Set(GroupMembershipStatus::Active.as_str().to_string());
            active.invited_by_user_id = Set(Some(invitation_model.invited_by_user_id));
            active.joined_at = Set(Some(now.fixed_offset()));
            active.left_at = Set(None);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(invitation_model.group_id),
                user_id: Set(actor_user_id),
                role: Set(GroupRole::Member.as_str().to_string()),
                status: Set(GroupMembershipStatus::Active.as_str().to_string()),
                invited_by_user_id: Set(Some(invitation_model.invited_by_user_id)),
                joined_at: Set(Some(now.fixed_offset())),
                left_at: Set(None),
                metadata: Set(json!({})),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?
        };

        redemption::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            invitation_id: Set(invitation_model.id),
            group_id: Set(invitation_model.group_id),
            user_id: Set(actor_user_id),
            redeemed_at: Set(now.fixed_offset()),
        }
        .insert(&transaction)
        .await?;

        let invitation_id = invitation_model.id;
        let group_id = invitation_model.group_id;
        let mut invitation_active: invitation::ActiveModel = invitation_model.into();
        invitation_active.use_count = Set(1);
        invitation_active.updated_at = Set(now.fixed_offset());
        invitation_active.update(&transaction).await?;

        let group_version =
            increment_group_membership_version(&transaction, group_model, now).await?;
        let result = AcceptGroupInvitationResult {
            invitation_id,
            group_id,
            membership: map_membership(membership_model)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            group_id,
            actor_user_id,
            "group.targeted_invitation_accepted",
            Some(actor_user_id),
            json!({
                "invitation_id": invitation_id,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            group_id,
            actor_user_id,
            idempotency_key,
            ACCEPT_TARGETED_INVITATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}

#[async_trait]
impl GroupTargetedInvitationCommandPort for GroupTargetedInvitationService {
    async fn accept_targeted_group_invitation(
        &self,
        context: PortContext,
        request: AcceptTargetedGroupInvitationRequest,
    ) -> Result<AcceptGroupInvitationResult, PortError> {
        self.accept_owned(&context, request).await.map_err(Into::into)
    }
}

async fn find_invitation_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    invitation_id: Uuid,
) -> GroupsResult<invitation::Model> {
    let query = || {
        invitation::Entity::find()
            .filter(invitation::Column::TenantId.eq(tenant_id))
            .filter(invitation::Column::Id.eq(invitation_id))
    };
    match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
    }
    .ok_or_else(targeted_invitation_unavailable)
}

async fn find_group_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
) -> GroupsResult<group::Model> {
    let query = || {
        group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Id.eq(group_id))
    };
    match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
    }
    .ok_or_else(targeted_invitation_unavailable)
}

fn ensure_targeted_invitation_active(
    model: &invitation::Model,
    actor_user_id: Uuid,
) -> GroupsResult<()> {
    if model.target_user_id != Some(actor_user_id)
        || model.max_uses != 1
        || model.revoked_at.is_some()
        || model.expires_at.with_timezone(&Utc) <= Utc::now()
        || model.use_count >= model.max_uses
    {
        Err(targeted_invitation_unavailable())
    } else {
        Ok(())
    }
}

fn require_active_group(model: &group::Model) -> GroupsResult<()> {
    if model.status == GroupStatus::Active.as_str() {
        Ok(())
    } else {
        Err(targeted_invitation_unavailable())
    }
}

async fn increment_group_membership_version(
    transaction: &DatabaseTransaction,
    group_model: group::Model,
    now: DateTime<Utc>,
) -> GroupsResult<u64> {
    let group_version = group_model.version.saturating_add(1).max(1) as u64;
    let member_count = group_model.member_count.saturating_add(1);
    let mut active: group::ActiveModel = group_model.into();
    active.member_count = Set(member_count);
    active.version = Set(group_version as i64);
    active.updated_at = Set(now.fixed_offset());
    active.update(transaction).await?;
    Ok(group_version)
}

fn map_membership(model: membership::Model) -> GroupsResult<GroupMembership> {
    Ok(GroupMembership {
        id: model.id,
        group_id: model.group_id,
        user_id: model.user_id,
        role: GroupRole::from_str(&model.role).map_err(GroupsError::Invariant)?,
        status: GroupMembershipStatus::from_str(&model.status).map_err(GroupsError::Invariant)?,
    })
}

async fn replay_receipt<T: DeserializeOwned>(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
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
    if receipt.actor_user_id != actor_user_id
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
    actor_user_id: Uuid,
    action: &str,
    target_user_id: Option<Uuid>,
    details: Value,
) -> GroupsResult<()> {
    audit_entry::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
        actor_user_id: Set(Some(actor_user_id)),
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
            "a user actor is required for targeted group invitations".to_string(),
        ));
    }
    Uuid::parse_str(&context.actor.id)
        .map_err(|_| GroupsError::Validation("actor.id must be a UUID".to_string()))
}

fn idempotency_key(context: &PortContext) -> GroupsResult<String> {
    let key = context
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| GroupsError::Validation("idempotency key is required".to_string()))?;
    if key.len() > 160 {
        return Err(GroupsError::Validation(
            "idempotency key must not exceed 160 bytes".to_string(),
        ));
    }
    Ok(key.to_string())
}

fn targeted_invitation_unavailable() -> GroupsError {
    GroupsError::NotFound
}
