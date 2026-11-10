use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    DbBackend, EntityTrait, QueryFilter, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
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

impl AcceptTargetedGroupInvitationRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        if self.invitation_id.is_nil() {
            return Err(GroupsError::Validation(
                "invitation_id cannot be nil UUID".to_string(),
            ));
        }
        Ok(())
    }
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
}

#[async_trait]
impl GroupTargetedInvitationCommandPort for GroupTargetedInvitationService {
    async fn accept_targeted_group_invitation(
        &self,
        context: PortContext,
        request: AcceptTargetedGroupInvitationRequest,
    ) -> Result<AcceptGroupInvitationResult, PortError> {
        request.validate().map_err(PortError::from)?;
        self.accept_targeted_group_invitation_effective_owned(&context, request)
            .await
            .map_err(Into::into)
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
    let locator = query()
        .one(transaction)
        .await?
        .ok_or_else(|| GroupsError::NotFound)?;
    crate::membership_enforcement_transaction::reserve_group_write_for_update(
        transaction,
        tenant_id,
        locator.group_id,
    )
    .await?;
    match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
        _ => unreachable!("unsupported SeaORM database backend"),
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
        _ => unreachable!("unsupported SeaORM database backend"),
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
        crate::invitation_owner::validate_stored_invitation_uses(model)
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
    let group_version = crate::domain::next_revision(group_model.version)? as u64;
    let member_count = crate::domain::increment_member_count(group_model.member_count)?;
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
    let trimmed = context.tenant_id.trim();
    if trimmed.is_empty() {
        return Err(GroupsError::Validation(
            "tenant_id cannot be empty".to_string(),
        ));
    }
    let parsed = Uuid::parse_str(trimmed)
        .map_err(|_| GroupsError::Validation("tenant_id must be a UUID".to_string()))?;
    if parsed.is_nil() {
        return Err(GroupsError::Validation(
            "tenant_id cannot be nil UUID".to_string(),
        ));
    }
    Ok(parsed)
}

fn actor_user_id(context: &PortContext) -> GroupsResult<Uuid> {
    if context.actor.kind != PortActorKind::User {
        return Err(GroupsError::Forbidden(
            "a user actor is required for targeted group invitations".to_string(),
        ));
    }
    let trimmed = context.actor.id.trim();
    if trimmed.is_empty() {
        return Err(GroupsError::Validation(
            "actor.id cannot be empty".to_string(),
        ));
    }
    let parsed = Uuid::parse_str(trimmed)
        .map_err(|_| GroupsError::Validation("actor.id must be a UUID".to_string()))?;
    if parsed.is_nil() {
        return Err(GroupsError::Validation(
            "actor.id cannot be nil UUID".to_string(),
        ));
    }
    Ok(parsed)
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

#[path = "targeted_invitations_transactional.rs"]
mod transactional;
