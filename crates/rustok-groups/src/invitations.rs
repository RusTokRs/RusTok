use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    DbBackend, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    TransactionTrait,
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

const CREATE_INVITATION_COMMAND: &str = "groups.create_invitation.v1";
const REVOKE_INVITATION_COMMAND: &str = "groups.revoke_invitation.v1";
const ACCEPT_INVITATION_COMMAND: &str = "groups.accept_invitation.v1";
const MIN_EXPIRY_SECONDS: u64 = 300;
const MAX_EXPIRY_SECONDS: u64 = 30 * 24 * 60 * 60;
const MAX_INVITATION_USES: u32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupInvitationStatus {
    Active,
    Exhausted,
    Revoked,
    Expired,
}

impl GroupInvitationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Exhausted => "exhausted",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupInvitation {
    pub id: Uuid,
    pub group_id: Uuid,
    pub invited_by_user_id: Uuid,
    pub target_user_id: Option<Uuid>,
    pub max_uses: u32,
    pub use_count: u32,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub status: GroupInvitationStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupInvitationConnection {
    pub items: Vec<GroupInvitation>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListGroupInvitationsRequest {
    pub group_id: Uuid,
    pub page: u64,
    pub per_page: u64,
    pub include_inactive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGroupInvitationRequest {
    pub group_id: Uuid,
    pub target_user_id: Option<Uuid>,
    pub expires_in_seconds: u64,
    pub max_uses: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGroupInvitationResult {
    pub invitation: GroupInvitation,
    pub token: Option<String>,
    pub group_version: u64,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevokeGroupInvitationRequest {
    pub invitation_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevokeGroupInvitationResult {
    pub invitation: GroupInvitation,
    pub group_version: u64,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptGroupInvitationRequest {
    pub token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptGroupInvitationResult {
    pub invitation_id: Uuid,
    pub group_id: Uuid,
    pub membership: GroupMembership,
    pub group_version: u64,
    pub replayed: bool,
}

#[async_trait]
pub trait GroupInvitationReadPort: Send + Sync {
    async fn list_group_invitations(
        &self,
        context: PortContext,
        request: ListGroupInvitationsRequest,
    ) -> Result<GroupInvitationConnection, PortError>;
}

#[async_trait]
pub trait GroupInvitationCommandPort: Send + Sync {
    async fn create_group_invitation(
        &self,
        context: PortContext,
        request: CreateGroupInvitationRequest,
    ) -> Result<CreateGroupInvitationResult, PortError>;

    async fn revoke_group_invitation(
        &self,
        context: PortContext,
        request: RevokeGroupInvitationRequest,
    ) -> Result<RevokeGroupInvitationResult, PortError>;

    async fn accept_group_invitation(
        &self,
        context: PortContext,
        request: AcceptGroupInvitationRequest,
    ) -> Result<AcceptGroupInvitationResult, PortError>;
}

#[derive(Clone)]
pub struct GroupInvitationService {
    db: DatabaseConnection,
}

impl GroupInvitationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn list_group_invitations_owned(
        &self,
        context: &PortContext,
        request: ListGroupInvitationsRequest,
    ) -> GroupsResult<GroupInvitationConnection> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let group_model = group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Id.eq(request.group_id))
            .one(&self.db)
            .await?
            .ok_or(GroupsError::NotFound)?;
        authorize_invitation_management(
            &self.db,
            context,
            tenant_id,
            group_model.id,
            actor_user_id,
        )
        .await?;

        let page = request.page.max(1);
        let per_page = request.per_page.clamp(1, 100);
        let now = Utc::now().fixed_offset();
        let mut query = invitation::Entity::find()
            .filter(invitation::Column::TenantId.eq(tenant_id))
            .filter(invitation::Column::GroupId.eq(request.group_id));
        if !request.include_inactive {
            query = query
                .filter(invitation::Column::RevokedAt.is_null())
                .filter(invitation::Column::ExpiresAt.gt(now))
                .filter(
                    Expr::col(invitation::Column::UseCount)
                        .lt(Expr::col(invitation::Column::MaxUses)),
                );
        }
        let paginator = query
            .order_by_desc(invitation::Column::CreatedAt)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let items = paginator
            .fetch_page(page.saturating_sub(1))
            .await?
            .into_iter()
            .map(map_invitation)
            .collect::<GroupsResult<Vec<_>>>()?;

        Ok(GroupInvitationConnection {
            items,
            total,
            page,
            per_page,
        })
    }

    async fn create_group_invitation_owned(
        &self,
        context: &PortContext,
        request: CreateGroupInvitationRequest,
    ) -> GroupsResult<CreateGroupInvitationResult> {
        require_write(context)?;
        validate_create_request(&request)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;
        let group_model = find_group_for_update(&transaction, tenant_id, request.group_id).await?;

        if let Some(mut replayed) = replay_receipt::<CreateGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            CREATE_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.token = None;
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        require_active_group(&group_model)?;
        authorize_invitation_management_in_transaction(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
        )
        .await?;

        let now = Utc::now();
        let expires_at = now + Duration::seconds(request.expires_in_seconds as i64);
        let token = generate_invitation_token();
        let token_hash = invitation_token_hash(&token);
        let invitation_model = invitation::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            group_id: Set(request.group_id),
            invited_by_user_id: Set(actor_user_id),
            target_user_id: Set(request.target_user_id),
            token_hash: Set(token_hash),
            max_uses: Set(request.max_uses as i32),
            use_count: Set(0),
            expires_at: Set(expires_at.fixed_offset()),
            revoked_at: Set(None),
            revoked_by_user_id: Set(None),
            created_at: Set(now.fixed_offset()),
            updated_at: Set(now.fixed_offset()),
        }
        .insert(&transaction)
        .await?;

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = CreateGroupInvitationResult {
            invitation: map_invitation(invitation_model)?,
            token: Some(token),
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
            "group.invitation_created",
            request.target_user_id,
            json!({
                "invitation_id": result.invitation.id,
                "target_user_id": request.target_user_id,
                "max_uses": request.max_uses,
                "expires_at": expires_at,
                "group_version": group_version
            }),
        )
        .await?;
        let mut stored_result = result.clone();
        stored_result.token = None;
        store_receipt(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            idempotency_key,
            CREATE_INVITATION_COMMAND,
            request_hash,
            &stored_result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn revoke_group_invitation_owned(
        &self,
        context: &PortContext,
        request: RevokeGroupInvitationRequest,
    ) -> GroupsResult<RevokeGroupInvitationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;
        let invitation_model = find_invitation_for_update(
            &transaction,
            tenant_id,
            request.invitation_id,
        )
        .await?;

        if let Some(mut replayed) = replay_receipt::<RevokeGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            REVOKE_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }
        if invitation_model.revoked_at.is_some() {
            return Err(GroupsError::Conflict(
                "group invitation is already revoked".to_string(),
            ));
        }

        let group_model =
            find_group_for_update(&transaction, tenant_id, invitation_model.group_id).await?;
        authorize_invitation_management_in_transaction(
            &transaction,
            context,
            tenant_id,
            invitation_model.group_id,
            actor_user_id,
        )
        .await?;
        let now = Utc::now();
        let mut active: invitation::ActiveModel = invitation_model.into();
        active.revoked_at = Set(Some(now.fixed_offset()));
        active.revoked_by_user_id = Set(Some(actor_user_id));
        active.updated_at = Set(now.fixed_offset());
        let revoked = active.update(&transaction).await?;
        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = RevokeGroupInvitationResult {
            invitation: map_invitation(revoked)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            result.invitation.group_id,
            actor_user_id,
            "group.invitation_revoked",
            result.invitation.target_user_id,
            json!({
                "invitation_id": result.invitation.id,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            result.invitation.group_id,
            actor_user_id,
            idempotency_key,
            REVOKE_INVITATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn accept_group_invitation_owned(
        &self,
        context: &PortContext,
        request: AcceptGroupInvitationRequest,
    ) -> GroupsResult<AcceptGroupInvitationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let token = request.token.trim();
        if token.len() < 32 || token.len() > 160 {
            return Err(invalid_invitation_token());
        }
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<AcceptGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            ACCEPT_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let token_hash = invitation_token_hash(token);
        let invitation_model =
            find_invitation_by_token_for_update(&transaction, tenant_id, &token_hash).await?;
        ensure_invitation_active(&invitation_model, actor_user_id)?;
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
            return Err(GroupsError::Conflict(
                "group invitation was already accepted by this user".to_string(),
            ));
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
        let next_use_count = invitation_model.use_count.saturating_add(1);
        let mut invitation_active: invitation::ActiveModel = invitation_model.into();
        invitation_active.use_count = Set(next_use_count);
        invitation_active.updated_at = Set(now.fixed_offset());
        invitation_active.update(&transaction).await?;

        let group_version = increment_group_membership_version(&transaction, group_model, now).await?;
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
            "group.invitation_accepted",
            Some(actor_user_id),
            json!({
                "invitation_id": invitation_id,
                "use_count": next_use_count,
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
            ACCEPT_INVITATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}

#[async_trait]
impl GroupInvitationReadPort for GroupInvitationService {
    async fn list_group_invitations(
        &self,
        context: PortContext,
        request: ListGroupInvitationsRequest,
    ) -> Result<GroupInvitationConnection, PortError> {
        self.list_group_invitations_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

#[async_trait]
impl GroupInvitationCommandPort for GroupInvitationService {
    async fn create_group_invitation(
        &self,
        context: PortContext,
        request: CreateGroupInvitationRequest,
    ) -> Result<CreateGroupInvitationResult, PortError> {
        self.create_group_invitation_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn revoke_group_invitation(
        &self,
        context: PortContext,
        request: RevokeGroupInvitationRequest,
    ) -> Result<RevokeGroupInvitationResult, PortError> {
        self.revoke_group_invitation_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn accept_group_invitation(
        &self,
        context: PortContext,
        request: AcceptGroupInvitationRequest,
    ) -> Result<AcceptGroupInvitationResult, PortError> {
        self.accept_group_invitation_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

fn validate_create_request(request: &CreateGroupInvitationRequest) -> GroupsResult<()> {
    if !(MIN_EXPIRY_SECONDS..=MAX_EXPIRY_SECONDS).contains(&request.expires_in_seconds) {
        return Err(GroupsError::Validation(format!(
            "invitation expiry must be between {MIN_EXPIRY_SECONDS} and {MAX_EXPIRY_SECONDS} seconds"
        )));
    }
    if !(1..=MAX_INVITATION_USES).contains(&request.max_uses) {
        return Err(GroupsError::Validation(format!(
            "invitation max_uses must be between 1 and {MAX_INVITATION_USES}"
        )));
    }
    if request.target_user_id.is_some() && request.max_uses != 1 {
        return Err(GroupsError::Validation(
            "a targeted invitation must have max_uses equal to 1".to_string(),
        ));
    }
    Ok(())
}

async fn authorize_invitation_management<C: sea_orm::ConnectionTrait>(
    connection: &C,
    context: &PortContext,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
) -> GroupsResult<()> {
    if has_platform_manage(context) {
        return Ok(());
    }
    let model = membership::Entity::find()
        .filter(membership::Column::TenantId.eq(tenant_id))
        .filter(membership::Column::GroupId.eq(group_id))
        .filter(membership::Column::UserId.eq(actor_user_id))
        .one(connection)
        .await?;
    let allowed = model
        .filter(|row| row.status == GroupMembershipStatus::Active.as_str())
        .and_then(|row| GroupRole::from_str(&row.role).ok())
        .is_some_and(GroupRole::can_moderate);
    if allowed {
        Ok(())
    } else {
        Err(GroupsError::Forbidden(
            "group owner, administrator, or moderator role is required".to_string(),
        ))
    }
}

async fn authorize_invitation_management_in_transaction(
    transaction: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
) -> GroupsResult<()> {
    authorize_invitation_management(
        transaction,
        context,
        tenant_id,
        group_id,
        actor_user_id,
    )
    .await
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
    .ok_or(GroupsError::NotFound)
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
    .ok_or(GroupsError::NotFound)
}

async fn find_invitation_by_token_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    token_hash: &str,
) -> GroupsResult<invitation::Model> {
    let query = || {
        invitation::Entity::find()
            .filter(invitation::Column::TenantId.eq(tenant_id))
            .filter(invitation::Column::TokenHash.eq(token_hash))
    };
    match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
    }
    .ok_or_else(invalid_invitation_token)
}

fn require_active_group(model: &group::Model) -> GroupsResult<()> {
    if model.status == GroupStatus::Active.as_str() {
        Ok(())
    } else {
        Err(GroupsError::Conflict("group is not active".to_string()))
    }
}

fn ensure_invitation_active(model: &invitation::Model, actor_user_id: Uuid) -> GroupsResult<()> {
    if model.revoked_at.is_some()
        || model.expires_at.with_timezone(&Utc) <= Utc::now()
        || model.use_count >= model.max_uses
        || model
            .target_user_id
            .is_some_and(|target_user_id| target_user_id != actor_user_id)
    {
        Err(invalid_invitation_token())
    } else {
        Ok(())
    }
}

async fn increment_group_version(
    transaction: &DatabaseTransaction,
    group_model: group::Model,
    now: DateTime<Utc>,
) -> GroupsResult<u64> {
    let group_version = group_model.version.saturating_add(1).max(1) as u64;
    let mut active: group::ActiveModel = group_model.into();
    active.version = Set(group_version as i64);
    active.updated_at = Set(now.fixed_offset());
    active.update(transaction).await?;
    Ok(group_version)
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

fn map_invitation(model: invitation::Model) -> GroupsResult<GroupInvitation> {
    let now = Utc::now();
    let status = if model.revoked_at.is_some() {
        GroupInvitationStatus::Revoked
    } else if model.expires_at.with_timezone(&Utc) <= now {
        GroupInvitationStatus::Expired
    } else if model.use_count >= model.max_uses {
        GroupInvitationStatus::Exhausted
    } else {
        GroupInvitationStatus::Active
    };
    Ok(GroupInvitation {
        id: model.id,
        group_id: model.group_id,
        invited_by_user_id: model.invited_by_user_id,
        target_user_id: model.target_user_id,
        max_uses: model.max_uses.max(0) as u32,
        use_count: model.use_count.max(0) as u32,
        expires_at: model.expires_at.with_timezone(&Utc),
        revoked_at: model.revoked_at.map(|value| value.with_timezone(&Utc)),
        revoked_by_user_id: model.revoked_by_user_id,
        created_at: model.created_at.with_timezone(&Utc),
        status,
    })
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

fn generate_invitation_token() -> String {
    format!(
        "gri_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

fn invitation_token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn invalid_invitation_token() -> GroupsError {
    GroupsError::Conflict("group invitation token is invalid or unavailable".to_string())
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

fn require_read(context: &PortContext) -> GroupsResult<()> {
    context
        .require_policy(PortCallPolicy::read())
        .map_err(|error| GroupsError::Validation(error.message))
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
            "a user actor is required for group invitations".to_string(),
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

fn has_platform_manage(context: &PortContext) -> bool {
    context
        .claims
        .iter()
        .any(|claim| matches!(claim.as_str(), "groups:manage" | "groups:*" | "*:*") )
}
