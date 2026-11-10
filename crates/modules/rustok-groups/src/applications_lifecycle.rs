use super::*;

pub(super) const CANCEL_APPLICATION_COMMAND: &str = "groups.cancel_membership_application.v1";
pub(super) const REOPEN_APPLICATION_COMMAND: &str = "groups.reopen_membership_application.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadMyGroupMembershipApplicationRequest {
    pub group_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelGroupMembershipApplicationRequest {
    pub application_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReopenGroupMembershipApplicationRequest {
    pub application_id: Uuid,
}

impl CancelGroupMembershipApplicationRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        if self.application_id.is_nil() {
            return Err(GroupsError::Validation(
                "application_id cannot be nil UUID".into(),
            ));
        }
        Ok(())
    }
}

impl ReopenGroupMembershipApplicationRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        if self.application_id.is_nil() {
            return Err(GroupsError::Validation(
                "application_id cannot be nil UUID".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationLifecycleResult {
    pub application: GroupMembershipApplication,
    pub membership: GroupMembership,
    pub group_version: u64,
    pub replayed: bool,
}

#[async_trait]
pub trait GroupApplicationLifecycleReadPort: Send + Sync {
    async fn read_my_group_membership_application(
        &self,
        context: PortContext,
        request: ReadMyGroupMembershipApplicationRequest,
    ) -> Result<Option<GroupMembershipApplication>, PortError>;
}

#[async_trait]
pub trait GroupApplicationLifecycleCommandPort: Send + Sync {
    async fn cancel_group_membership_application(
        &self,
        context: PortContext,
        request: CancelGroupMembershipApplicationRequest,
    ) -> Result<GroupApplicationLifecycleResult, PortError>;

    async fn reopen_group_membership_application(
        &self,
        context: PortContext,
        request: ReopenGroupMembershipApplicationRequest,
    ) -> Result<GroupApplicationLifecycleResult, PortError>;
}

impl GroupApplicationService {
    async fn read_my_application_owned(
        &self,
        context: &PortContext,
        request: ReadMyGroupMembershipApplicationRequest,
    ) -> GroupsResult<Option<GroupMembershipApplication>> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let group_model = find_group(&self.db, tenant_id, request.group_id).await?;
        require_candidate_group(&group_model)?;
        ensure_not_banned(&self.db, tenant_id, request.group_id, actor_user_id).await?;
        membership_application::Entity::find()
            .filter(membership_application::Column::TenantId.eq(tenant_id))
            .filter(membership_application::Column::GroupId.eq(request.group_id))
            .filter(membership_application::Column::UserId.eq(actor_user_id))
            .one(&self.db)
            .await?
            .map(map_application)
            .transpose()
    }
}

#[async_trait]
impl GroupApplicationLifecycleReadPort for GroupApplicationService {
    async fn read_my_group_membership_application(
        &self,
        context: PortContext,
        request: ReadMyGroupMembershipApplicationRequest,
    ) -> Result<Option<GroupMembershipApplication>, PortError> {
        self.read_my_application_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

#[async_trait]
impl GroupApplicationLifecycleCommandPort for GroupApplicationService {
    async fn cancel_group_membership_application(
        &self,
        context: PortContext,
        request: CancelGroupMembershipApplicationRequest,
    ) -> Result<GroupApplicationLifecycleResult, PortError> {
        self.cancel_application_effective_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn reopen_group_membership_application(
        &self,
        context: PortContext,
        request: ReopenGroupMembershipApplicationRequest,
    ) -> Result<GroupApplicationLifecycleResult, PortError> {
        self.reopen_application_effective_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}
