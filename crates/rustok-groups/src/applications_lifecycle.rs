const CANCEL_APPLICATION_COMMAND: &str = "groups.cancel_membership_application.v1";
const REOPEN_APPLICATION_COMMAND: &str = "groups.reopen_membership_application.v1";

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
        require_active_group(&group_model)?;
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

    async fn cancel_application_owned(
        &self,
        context: &PortContext,
        request: CancelGroupMembershipApplicationRequest,
    ) -> GroupsResult<GroupApplicationLifecycleResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<GroupApplicationLifecycleResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            CANCEL_APPLICATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let application_model = find_application_for_update(
            &transaction,
            tenant_id,
            request.application_id,
        )
        .await?;
        if application_model.user_id != actor_user_id {
            return Err(GroupsError::NotFound);
        }
        if application_model.status != GroupApplicationStatus::Pending.as_str() {
            return Err(GroupsError::Conflict(
                "only a pending membership application can be cancelled".to_string(),
            ));
        }

        let group_model =
            find_group_for_update(&transaction, tenant_id, application_model.group_id).await?;
        require_active_group(&group_model)?;
        let membership_model = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(application_model.group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| {
                GroupsError::Invariant(
                    "pending application membership is missing during cancellation".to_string(),
                )
            })?;
        if membership_model.status == GroupMembershipStatus::Banned.as_str() {
            return Err(GroupsError::Forbidden(
                "group membership is banned".to_string(),
            ));
        }
        if membership_model.status != GroupMembershipStatus::Pending.as_str() {
            return Err(GroupsError::Conflict(
                "membership is no longer pending".to_string(),
            ));
        }

        let now = Utc::now();
        let mut membership_active: membership::ActiveModel = membership_model.into();
        membership_active.status = Set(GroupMembershipStatus::Left.as_str().to_string());
        membership_active.joined_at = Set(None);
        membership_active.left_at = Set(Some(now.fixed_offset()));
        membership_active.updated_at = Set(now.fixed_offset());
        let membership_model = membership_active.update(&transaction).await?;

        let mut application_active: membership_application::ActiveModel = application_model.into();
        application_active.status = Set(GroupApplicationStatus::Cancelled.as_str().to_string());
        application_active.reviewed_at = Set(None);
        application_active.reviewed_by_user_id = Set(None);
        application_active.review_note = Set(None);
        application_active.updated_at = Set(now.fixed_offset());
        let application_model = application_active.update(&transaction).await?;

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = GroupApplicationLifecycleResult {
            application: map_application(application_model)?,
            membership: map_membership(membership_model)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            result.application.group_id,
            actor_user_id,
            "group.membership_application_cancelled",
            Some(actor_user_id),
            json!({
                "application_id": result.application.id,
                "previous_status": "pending",
                "membership_status": result.membership.status,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            result.application.group_id,
            actor_user_id,
            idempotency_key,
            CANCEL_APPLICATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn reopen_application_owned(
        &self,
        context: &PortContext,
        request: ReopenGroupMembershipApplicationRequest,
    ) -> GroupsResult<GroupApplicationLifecycleResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<GroupApplicationLifecycleResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            REOPEN_APPLICATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let application_model = find_application_for_update(
            &transaction,
            tenant_id,
            request.application_id,
        )
        .await?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, application_model.group_id).await?;
        require_application_group(&group_model)?;
        authorize_application_review(
            &transaction,
            context,
            tenant_id,
            application_model.group_id,
            actor_user_id,
        )
        .await?;

        let previous_status = GroupApplicationStatus::from_str(&application_model.status)
            .map_err(GroupsError::Invariant)?;
        if !matches!(
            previous_status,
            GroupApplicationStatus::Rejected | GroupApplicationStatus::Cancelled
        ) {
            return Err(GroupsError::Conflict(
                "only a rejected or cancelled membership application can be reopened"
                    .to_string(),
            ));
        }

        let membership_model = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(application_model.group_id))
            .filter(membership::Column::UserId.eq(application_model.user_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| {
                GroupsError::Invariant(
                    "application membership is missing during reopen".to_string(),
                )
            })?;
        if membership_model.status == GroupMembershipStatus::Banned.as_str() {
            return Err(GroupsError::Forbidden(
                "group membership is banned".to_string(),
            ));
        }
        if membership_model.status == GroupMembershipStatus::Active.as_str() {
            return Err(GroupsError::Conflict(
                "an active member application cannot be reopened".to_string(),
            ));
        }
        if membership_model.status != GroupMembershipStatus::Left.as_str() {
            return Err(GroupsError::Conflict(
                "membership must be left before the application can be reopened".to_string(),
            ));
        }

        let now = Utc::now();
        let target_user_id = application_model.user_id;
        let mut membership_active: membership::ActiveModel = membership_model.into();
        membership_active.role = Set(GroupRole::Member.as_str().to_string());
        membership_active.status = Set(GroupMembershipStatus::Pending.as_str().to_string());
        membership_active.joined_at = Set(None);
        membership_active.left_at = Set(None);
        membership_active.updated_at = Set(now.fixed_offset());
        let membership_model = membership_active.update(&transaction).await?;

        let mut application_active: membership_application::ActiveModel = application_model.into();
        application_active.status = Set(GroupApplicationStatus::Pending.as_str().to_string());
        application_active.reviewed_at = Set(None);
        application_active.reviewed_by_user_id = Set(None);
        application_active.review_note = Set(None);
        application_active.updated_at = Set(now.fixed_offset());
        let application_model = application_active.update(&transaction).await?;

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = GroupApplicationLifecycleResult {
            application: map_application(application_model)?,
            membership: map_membership(membership_model)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            result.application.group_id,
            actor_user_id,
            "group.membership_application_reopened",
            Some(target_user_id),
            json!({
                "application_id": result.application.id,
                "previous_status": previous_status.as_str(),
                "policy_id": result.application.policy_id,
                "policy_revision": result.application.policy_revision,
                "policy_locale": result.application.policy_locale,
                "snapshot_preserved": true,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            result.application.group_id,
            actor_user_id,
            idempotency_key,
            REOPEN_APPLICATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
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
        self.cancel_application_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn reopen_group_membership_application(
        &self,
        context: PortContext,
        request: ReopenGroupMembershipApplicationRequest,
    ) -> Result<GroupApplicationLifecycleResult, PortError> {
        self.reopen_application_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}
