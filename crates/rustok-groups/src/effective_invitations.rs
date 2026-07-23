use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::effective_membership_guard::{
    GroupManagerCapability, actor_user_id, has_existing_receipt, require_candidate_not_denied,
    require_effective_manager, tenant_id,
};
use crate::invitation_entities::invitation;
use crate::invitations_legacy::{
    AcceptGroupInvitationRequest, AcceptGroupInvitationResult, CreateGroupInvitationRequest,
    CreateGroupInvitationResult, GroupInvitationCommandPort, GroupInvitationConnection,
    GroupInvitationReadPort, ListGroupInvitationsRequest, RevokeGroupInvitationRequest,
    RevokeGroupInvitationResult,
};
use crate::targeted_invitations_legacy::{
    AcceptTargetedGroupInvitationRequest, GroupTargetedInvitationCommandPort,
};

#[derive(Clone)]
pub struct GroupInvitationService {
    db: DatabaseConnection,
    legacy: crate::invitations_legacy::GroupInvitationService,
}

impl GroupInvitationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            legacy: crate::invitations_legacy::GroupInvitationService::new(db.clone()),
            db,
        }
    }

    async fn invitation_group_id(
        &self,
        context: &PortContext,
        invitation_id: uuid::Uuid,
    ) -> Result<Option<uuid::Uuid>, PortError> {
        let tenant_id = tenant_id(context)?;
        invitation::Entity::find()
            .filter(invitation::Column::TenantId.eq(tenant_id))
            .filter(invitation::Column::Id.eq(invitation_id))
            .one(&self.db)
            .await
            .map(|row| row.map(|row| row.group_id))
            .map_err(|error| {
                PortError::unavailable("groups.invitation_lookup_unavailable", error.to_string())
            })
    }

    async fn token_group_id(
        &self,
        context: &PortContext,
        token: &str,
    ) -> Result<Option<uuid::Uuid>, PortError> {
        let tenant_id = tenant_id(context)?;
        let token_hash = crate::domain::sha256_hex(token.trim().as_bytes());
        invitation::Entity::find()
            .filter(invitation::Column::TenantId.eq(tenant_id))
            .filter(invitation::Column::TokenHash.eq(token_hash))
            .one(&self.db)
            .await
            .map(|row| row.map(|row| row.group_id))
            .map_err(|error| {
                PortError::unavailable("groups.invitation_lookup_unavailable", error.to_string())
            })
    }
}

#[async_trait]
impl GroupInvitationReadPort for GroupInvitationService {
    async fn list_group_invitations(
        &self,
        context: PortContext,
        request: ListGroupInvitationsRequest,
    ) -> Result<GroupInvitationConnection, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        require_effective_manager(
            &self.db,
            &context,
            request.group_id,
            GroupManagerCapability::Moderate,
        )
        .await?;
        GroupInvitationReadPort::list_group_invitations(&self.legacy, context, request).await
    }
}

#[async_trait]
impl GroupInvitationCommandPort for GroupInvitationService {
    async fn create_group_invitation(
        &self,
        context: PortContext,
        request: CreateGroupInvitationRequest,
    ) -> Result<CreateGroupInvitationResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if !has_existing_receipt(&self.db, &context).await? {
            require_effective_manager(
                &self.db,
                &context,
                request.group_id,
                GroupManagerCapability::Moderate,
            )
            .await?;
        }
        GroupInvitationCommandPort::create_group_invitation(&self.legacy, context, request).await
    }

    async fn revoke_group_invitation(
        &self,
        context: PortContext,
        request: RevokeGroupInvitationRequest,
    ) -> Result<RevokeGroupInvitationResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if !has_existing_receipt(&self.db, &context).await? {
            if let Some(group_id) = self
                .invitation_group_id(&context, request.invitation_id)
                .await?
            {
                require_effective_manager(
                    &self.db,
                    &context,
                    group_id,
                    GroupManagerCapability::Moderate,
                )
                .await?;
            }
        }
        GroupInvitationCommandPort::revoke_group_invitation(&self.legacy, context, request).await
    }

    async fn accept_group_invitation(
        &self,
        context: PortContext,
        request: AcceptGroupInvitationRequest,
    ) -> Result<AcceptGroupInvitationResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        actor_user_id(&context)?;
        if !has_existing_receipt(&self.db, &context).await? {
            if let Some(group_id) = self.token_group_id(&context, &request.token).await? {
                require_candidate_not_denied(&self.db, &context, group_id, true).await?;
            }
        }
        GroupInvitationCommandPort::accept_group_invitation(&self.legacy, context, request).await
    }
}

#[derive(Clone)]
pub struct GroupTargetedInvitationService {
    db: DatabaseConnection,
    legacy: crate::targeted_invitations_legacy::GroupTargetedInvitationService,
}

impl GroupTargetedInvitationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            legacy: crate::targeted_invitations_legacy::GroupTargetedInvitationService::new(
                db.clone(),
            ),
            db,
        }
    }

    async fn invitation_group_id(
        &self,
        context: &PortContext,
        invitation_id: uuid::Uuid,
    ) -> Result<Option<uuid::Uuid>, PortError> {
        let tenant_id = tenant_id(context)?;
        invitation::Entity::find()
            .filter(invitation::Column::TenantId.eq(tenant_id))
            .filter(invitation::Column::Id.eq(invitation_id))
            .one(&self.db)
            .await
            .map(|row| row.map(|row| row.group_id))
            .map_err(|error| {
                PortError::unavailable("groups.invitation_lookup_unavailable", error.to_string())
            })
    }
}

#[async_trait]
impl GroupTargetedInvitationCommandPort for GroupTargetedInvitationService {
    async fn accept_targeted_group_invitation(
        &self,
        context: PortContext,
        request: AcceptTargetedGroupInvitationRequest,
    ) -> Result<AcceptGroupInvitationResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        actor_user_id(&context)?;
        if !has_existing_receipt(&self.db, &context).await? {
            if let Some(group_id) = self
                .invitation_group_id(&context, request.invitation_id)
                .await?
            {
                require_candidate_not_denied(&self.db, &context, group_id, true).await?;
            }
        }
        GroupTargetedInvitationCommandPort::accept_targeted_group_invitation(
            &self.legacy,
            context,
            request,
        )
        .await
    }
}
