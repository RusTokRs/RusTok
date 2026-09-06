use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::DatabaseConnection;

use crate::effective_membership_guard::{
    GroupManagerCapability, actor_user_id, require_effective_manager, tenant_id,
};
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
        tenant_id(&context)?;
        actor_user_id(&context)?;
        self.legacy
            .create_group_invitation_effective_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn revoke_group_invitation(
        &self,
        context: PortContext,
        request: RevokeGroupInvitationRequest,
    ) -> Result<RevokeGroupInvitationResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        tenant_id(&context)?;
        actor_user_id(&context)?;
        self.legacy
            .revoke_group_invitation_effective_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn accept_group_invitation(
        &self,
        context: PortContext,
        request: AcceptGroupInvitationRequest,
    ) -> Result<AcceptGroupInvitationResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        tenant_id(&context)?;
        actor_user_id(&context)?;
        self.legacy
            .accept_group_invitation_effective_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

#[derive(Clone)]
pub struct GroupTargetedInvitationService {
    legacy: crate::targeted_invitations_legacy::GroupTargetedInvitationService,
}

impl GroupTargetedInvitationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            legacy: crate::targeted_invitations_legacy::GroupTargetedInvitationService::new(db),
        }
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
        tenant_id(&context)?;
        actor_user_id(&context)?;
        self.legacy
            .accept_targeted_group_invitation_effective_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}
