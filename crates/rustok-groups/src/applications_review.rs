#[async_trait]
pub trait GroupApplicationReviewCommandPort: Send + Sync {
    async fn review_group_membership_application(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError>;
}

#[async_trait]
impl GroupApplicationReviewCommandPort for GroupApplicationService {
    async fn review_group_membership_application(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError> {
        self.review_application_authorized_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

impl GroupApplicationService {
    async fn review_application_authorized_owned(
        &self,
        context: &PortContext,
        mut request: ReviewGroupMembershipApplicationRequest,
    ) -> GroupsResult<ReviewGroupMembershipApplicationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        request.note = normalize_optional_note(request.note)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<ReviewGroupMembershipApplicationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            REVIEW_APPLICATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let application_model =
            find_application_for_update(&transaction, tenant_id, request.application_id).await?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, application_model.group_id).await?;
        require_active_group(&group_model)?;
        authorize_application_review(
            &transaction,
            context,
            tenant_id,
            application_model.group_id,
            actor_user_id,
        )
        .await?;
        if application_model.status != GroupApplicationStatus::Pending.as_str() {
            return Err(GroupsError::Conflict(
                "membership application is no longer pending".to_string(),
            ));
        }

        let membership_model = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(application_model.group_id))
            .filter(membership::Column::UserId.eq(application_model.user_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| {
                GroupsError::Invariant("pending application membership is missing".to_string())
            })?;
        if membership_model.status == GroupMembershipStatus::Banned.as_str() {
            return Err(GroupsError::Forbidden(
                "group membership is banned".to_string(),
            ));
        }

        let now = Utc::now();
        let approved = request.decision == GroupApplicationReviewDecision::Approve;
        let mut membership_active: membership::ActiveModel = membership_model.into();
        membership_active.role = Set(GroupRole::Member.as_str().to_string());
        membership_active.status = Set(if approved {
            GroupMembershipStatus::Active.as_str().to_string()
        } else {
            GroupMembershipStatus::Left.as_str().to_string()
        });
        membership_active.joined_at = Set(approved.then_some(now.fixed_offset()));
        membership_active.left_at = Set((!approved).then_some(now.fixed_offset()));
        membership_active.updated_at = Set(now.fixed_offset());
        let membership_model = membership_active.update(&transaction).await?;

        let mut application_active: membership_application::ActiveModel = application_model.into();
        application_active.status = Set(if approved {
            GroupApplicationStatus::Approved.as_str().to_string()
        } else {
            GroupApplicationStatus::Rejected.as_str().to_string()
        });
        application_active.reviewed_at = Set(Some(now.fixed_offset()));
        application_active.reviewed_by_user_id = Set(Some(actor_user_id));
        application_active.review_note = Set(request.note.clone());
        application_active.updated_at = Set(now.fixed_offset());
        let application_model = application_active.update(&transaction).await?;

        let group_version = if approved {
            increment_group_membership_version(&transaction, group_model, now).await?
        } else {
            increment_group_version(&transaction, group_model, now).await?
        };
        let result = ReviewGroupMembershipApplicationResult {
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
            if approved {
                "group.membership_application_approved"
            } else {
                "group.membership_application_rejected"
            },
            Some(result.application.user_id),
            json!({
                "application_id": result.application.id,
                "decision": if approved { "approve" } else { "reject" },
                "review_note_present": result.application.review_note.is_some(),
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
            REVIEW_APPLICATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}
